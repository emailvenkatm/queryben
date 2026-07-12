//! Azure Resource Manager REST client. Bearer in, typed structs out.
//! Shares a single reqwest::Client across calls so ARM keep-alives can pool.

use std::sync::OnceLock;

use reqwest::{Client, StatusCode};
use serde_json::Value;

use crate::core::azure::{AzureSqlDatabase, AzureSqlServer, AzureSubscription};
use crate::error::AppError;

const SUBSCRIPTIONS_API_VERSION: &str = "2022-12-01";
const SQL_RP_API_VERSION: &str = "2023-08-01-preview";
const FIREWALL_API_VERSION: &str = "2023-08-01-preview";
const ARM_ROOT: &str = "https://management.azure.com";
const USER_AGENT: &str = "QueryBen/0.1.0";

fn http() -> &'static Client {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        // Falls back to Client::new() so we stay compatible with the crate-wide
        // `deny(unwrap_used, expect_used)` lint.
        Client::builder()
            .user_agent(USER_AGENT)
            .build()
            .unwrap_or_else(|_| Client::new())
    })
}

async fn arm_get(bearer: &str, url: &str) -> Result<Value, AppError> {
    let resp = http()
        .get(url)
        .bearer_auth(bearer)
        .header("Accept", "application/json")
        .send()
        .await?;

    let status = resp.status();
    if status.is_success() {
        return Ok(resp.json::<Value>().await?);
    }

    if status == StatusCode::UNAUTHORIZED {
        return Err(AppError::AuthFailed(format!(
            "Azure REST 401 (bearer expired or wrong audience): {url}"
        )));
    }
    if status == StatusCode::NOT_FOUND {
        return Err(AppError::NotFound(format!("Azure REST 404: {url}")));
    }
    if status == StatusCode::TOO_MANY_REQUESTS {
        let retry_after_seconds = resp
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u32>().ok());
        return Err(AppError::RateLimited {
            retry_after_seconds,
        });
    }

    let body = resp.text().await.unwrap_or_else(|_| "<no body>".into());
    Err(AppError::Internal(format!(
        "Azure REST {status}: {body}"
    )))
}

async fn arm_put(bearer: &str, url: &str, body: &Value) -> Result<Value, AppError> {
    let resp = http()
        .put(url)
        .bearer_auth(bearer)
        .header("Accept", "application/json")
        .json(body)
        .send()
        .await?;

    let status = resp.status();
    if status.is_success() {
        return Ok(resp.json::<Value>().await.unwrap_or(Value::Null));
    }

    if status == StatusCode::UNAUTHORIZED {
        return Err(AppError::AuthFailed(format!(
            "Azure REST 401 (bearer expired or wrong audience): {url}"
        )));
    }
    if status == StatusCode::FORBIDDEN {
        return Err(AppError::AuthFailed(format!(
            "Azure REST 403 (signed-in user lacks permission to modify this resource): {url}"
        )));
    }
    // ARM aggressively throttles rapid firewallRules PUTs against the same
    // server. Surface as `RateLimited` so the UI can show a "try again in a
    // moment" pill instead of an angry red banner — and so we never auto-retry.
    if status == StatusCode::TOO_MANY_REQUESTS {
        let retry_after_seconds = resp
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u32>().ok());
        return Err(AppError::RateLimited {
            retry_after_seconds,
        });
    }

    let text = resp.text().await.unwrap_or_else(|_| "<no body>".into());
    Err(AppError::Internal(format!("Azure REST {status}: {text}")))
}

/// Pin `start_ip`..=`end_ip` in the Azure SQL server firewall. This is what
/// SSMS and Azure Data Studio do on error 40615: one ARM PUT with the mgmt
/// bearer. Pass equal start/end for a single-IP rule; pass a /24 range
/// (e.g. `70.185.81.0`..`70.185.81.255`) to cover the caller's whole ISP
/// block so IP drift within the subnet doesn't re-trigger 40615.
pub async fn add_firewall_rule(
    bearer: &str,
    server_arm_id: &str,
    rule_name: &str,
    start_ip: &str,
    end_ip: &str,
) -> Result<(), AppError> {
    let url = format!(
        "{ARM_ROOT}{server_arm_id}/firewallRules/{rule_name}?api-version={FIREWALL_API_VERSION}"
    );
    let body = serde_json::json!({
        "properties": {
            "startIpAddress": start_ip,
            "endIpAddress": end_ip,
        }
    });
    arm_put(bearer, &url, &body).await.map(|_| ())
}

/// Located Azure SQL server: enough to construct an ARM ID.
#[derive(Debug, Clone)]
pub struct DiscoveredSqlServer {
    pub subscription_id: String,
    pub resource_group: String,
    pub server_name: String,
    pub server_arm_id: String,
}

/// Fan out across every subscription the signed-in user can see, list its SQL
/// servers, and return the first one whose FQDN matches `server_fqdn` (case
/// insensitive). Used by the auto-firewall path when the caller only has the
/// tiberius target hostname — e.g. `foo.database.windows.net` — and not the
/// full ARM ID. Azure SQL server names are globally unique, so a match is
/// unambiguous even across subscriptions.
pub async fn discover_sql_server(
    bearer: &str,
    server_fqdn: &str,
) -> Result<DiscoveredSqlServer, AppError> {
    let target = server_fqdn.to_ascii_lowercase();
    let subs = list_subscriptions(bearer).await?;
    for sub in &subs {
        // A subscription the user can see but not enumerate SQL servers on
        // (RBAC) is a normal failure mode; log and keep going.
        let servers = match list_sql_servers(bearer, &sub.subscription_id).await {
            Ok(s) => s,
            Err(err) => {
                tracing::info!(
                    target: "queryben::azure_rest::discover",
                    subscription = %sub.subscription_id,
                    %err,
                    "skipping subscription during firewall discovery"
                );
                continue;
            }
        };
        for s in servers {
            if s.fully_qualified_domain_name.to_ascii_lowercase() == target {
                return Ok(DiscoveredSqlServer {
                    subscription_id: sub.subscription_id.clone(),
                    resource_group: s.resource_group.clone(),
                    server_name: s.name.clone(),
                    server_arm_id: s.id,
                });
            }
        }
    }
    Err(AppError::NotFound(format!(
        "Azure SQL server {server_fqdn} not found in any subscription the signed-in account can access"
    )))
}

fn s(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

fn prop(v: &Value, key: &str) -> Option<String> {
    v.get("properties").and_then(|p| p.get(key)).and_then(Value::as_str).map(str::to_string)
}

fn sku(v: &Value, key: &str) -> Option<String> {
    v.get("sku").and_then(|p| p.get(key)).and_then(Value::as_str).map(str::to_string)
}

// Pulls resource-group / server names out of an ARM ID path.
fn segment_after(id: &str, needle: &str) -> String {
    id.split_once(needle)
        .and_then(|(_, rest)| rest.split('/').next())
        .unwrap_or("")
        .to_string()
}

pub async fn list_subscriptions(bearer: &str) -> Result<Vec<AzureSubscription>, AppError> {
    let url = format!("{ARM_ROOT}/subscriptions?api-version={SUBSCRIPTIONS_API_VERSION}");
    let body = arm_get(bearer, &url).await?;
    let values = body
        .get("value")
        .and_then(Value::as_array)
        .ok_or_else(|| AppError::Internal("subscriptions: missing 'value' array".into()))?;

    let mut out = Vec::with_capacity(values.len());
    for v in values {
        out.push(AzureSubscription {
            id: s(v, "id").unwrap_or_default(),
            subscription_id: s(v, "subscriptionId").unwrap_or_default(),
            display_name: s(v, "displayName").unwrap_or_default(),
            tenant_id: s(v, "tenantId").unwrap_or_default(),
            state: s(v, "state").unwrap_or_else(|| "Unknown".into()),
        });
    }
    Ok(out)
}

pub async fn list_sql_servers(
    bearer: &str,
    subscription_id: &str,
) -> Result<Vec<AzureSqlServer>, AppError> {
    let url = format!(
        "{ARM_ROOT}/subscriptions/{subscription_id}/providers/Microsoft.Sql/servers?api-version={SQL_RP_API_VERSION}"
    );
    let body = arm_get(bearer, &url).await?;
    let values = body
        .get("value")
        .and_then(Value::as_array)
        .ok_or_else(|| AppError::Internal("sql servers: missing 'value' array".into()))?;

    let mut out = Vec::with_capacity(values.len());
    for v in values {
        let id = s(v, "id").unwrap_or_default();
        out.push(AzureSqlServer {
            resource_group: segment_after(&id, "/resourceGroups/"),
            name: s(v, "name").unwrap_or_default(),
            location: s(v, "location").unwrap_or_default(),
            kind: s(v, "kind"),
            fully_qualified_domain_name: prop(v, "fullyQualifiedDomainName").unwrap_or_default(),
            administrator_login: prop(v, "administratorLogin"),
            version: prop(v, "version"),
            id,
        });
    }
    Ok(out)
}

pub async fn list_databases(
    bearer: &str,
    server_id: &str,
) -> Result<Vec<AzureSqlDatabase>, AppError> {
    // Guard against a stray trailing slash producing `//databases`.
    let server_id = server_id.trim_end_matches('/');
    let url =
        format!("{ARM_ROOT}{server_id}/databases?api-version={SQL_RP_API_VERSION}");
    let body = arm_get(bearer, &url).await?;
    let values = body
        .get("value")
        .and_then(Value::as_array)
        .ok_or_else(|| AppError::Internal("databases: missing 'value' array".into()))?;

    let mut out = Vec::with_capacity(values.len());
    for v in values {
        let name = s(v, "name").unwrap_or_default();
        // `master` is never a useful connect target.
        if name.eq_ignore_ascii_case("master") { continue; }
        let id = s(v, "id").unwrap_or_default();
        out.push(AzureSqlDatabase {
            server_name: segment_after(&id, "/servers/"),
            location: s(v, "location").unwrap_or_default(),
            sku_tier: sku(v, "tier"),
            sku_name: sku(v, "name"),
            status: prop(v, "status"),
            collation: prop(v, "collation"),
            creation_date: prop(v, "creationDate")
                .and_then(|d| chrono::DateTime::parse_from_rfc3339(&d).ok())
                .map(|dt| dt.with_timezone(&chrono::Utc)),
            id,
            name,
        });
    }
    Ok(out)
}
