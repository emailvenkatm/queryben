//! Import ADS connections and snippets into QueryBen's registry.

use std::path::PathBuf;

use chrono::Utc;
use serde::Serialize;
use uuid::Uuid;

use crate::core::connection::{
    AuthMode, Connection, ConnectionEntry, ConnectionRegistry,
};
use crate::error::AppError;

use super::detection::ads_user_dir;
use super::msal_cache::{cache_files_present, try_borrow_ads_token};

/// Env override so tests can point snippet import at a tempdir. Rust-side
/// mirror of the same pattern used by azure_accounts + token_file_cache.
pub const ENV_QB_SNIPPETS_PATH_OVERRIDE: &str = "QUERYBEN_SNIPPETS_PATH";

/// One-shot summary of what `import_from_ads` actually did. Fields are
/// deliberately count-only — the UI just renders "3 connections imported,
/// 1 account signed in" and doesn't need the actual entries here.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AdsImportSummary {
    pub connections_imported: u32,
    /// Number of AAD accounts primed in the token cache. 0 or 1 in practice
    /// today — ADS's MSAL cache is per-account and we import all of them.
    pub accounts_imported: u32,
    /// Snippets we copied into QueryBen's snippets.json. Distinct from
    /// `snippet_count` on detection: this is the number that actually made
    /// it across (existing snippets are deduped by name).
    pub snippets_imported: u32,
}

/// Read every ADS connection into QueryBen's registry. Idempotent by
/// (server, database, auth_mode) — running twice does not create duplicates.
/// Primes the Azure token cache for any AAD accounts referenced by the
/// imported connections (best-effort; failures are logged, not fatal).
pub async fn import_from_ads(registry: &ConnectionRegistry) -> Result<AdsImportSummary, AppError> {
    let Some(user_dir) = ads_user_dir() else {
        return Ok(AdsImportSummary {
            connections_imported: 0,
            accounts_imported: 0,
            snippets_imported: 0,
        });
    };
    let settings_path = user_dir.join("settings.json");
    let raw = match std::fs::read_to_string(&settings_path) {
        Ok(s) => s,
        Err(_) => {
            return Ok(AdsImportSummary {
                connections_imported: 0,
                accounts_imported: 0,
                snippets_imported: 0,
            });
        }
    };
    let parsed: serde_json::Value = serde_json::from_str(&raw).map_err(|e| {
        AppError::internal(format!("parse ADS settings.json: {e}"))
    })?;

    let ads_connections = parsed
        .get("datasource.connections")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let existing = registry.list()?;
    let mut connections_imported = 0u32;
    let mut seen_accounts: std::collections::HashSet<String> = std::collections::HashSet::new();

    for entry in ads_connections {
        let Some(options) = entry.get("options") else { continue };
        let server = options.get("server").and_then(|s| s.as_str()).unwrap_or_default();
        let database = options.get("database").and_then(|s| s.as_str()).unwrap_or("master");
        let auth_str = options
            .get("authenticationType")
            .and_then(|s| s.as_str())
            .unwrap_or("SqlLogin");
        let auth_mode = ads_auth_to_qb(auth_str);

        if server.is_empty() {
            continue;
        }

        let is_duplicate = existing.iter().any(|c| {
            c.server.eq_ignore_ascii_case(server)
                && c.database.eq_ignore_ascii_case(database)
                && discriminant_matches(&c.auth_mode, &auth_mode)
        });
        if is_duplicate {
            continue;
        }

        let display_name = options
            .get("connectionName")
            .and_then(|s| s.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(server)
            .to_string();
        let username = options
            .get("user")
            .and_then(|s| s.as_str())
            .filter(|s| !s.is_empty())
            .map(String::from);
        let tenant_id = options
            .get("azureTenantId")
            .and_then(|s| s.as_str())
            .filter(|s| !s.is_empty())
            .map(String::from);
        let account_id = options
            .get("azureAccount")
            .and_then(|s| s.as_str())
            .filter(|s| !s.is_empty())
            .map(String::from);
        let arm_id = options
            .get("azureResourceId")
            .and_then(|s| s.as_str())
            .filter(|s| !s.is_empty())
            .map(String::from);

        if let Some(id) = account_id.as_deref() {
            seen_accounts.insert(id.to_string());
        }

        let conn = Connection {
            id: Uuid::new_v4(),
            name: display_name,
            server: server.to_string(),
            database: database.to_string(),
            port: None,
            username,
            auth_mode,
            created_at: Utc::now(),
            last_used: None,
            account_id,
            nickname: None,
            color: None,
        };
        let entry = ConnectionEntry {
            connection: conn,
            password: None,
            trust_server_certificate: options
                .get("trustServerCertificate")
                .and_then(|v| v.as_str().map(|s| s.eq_ignore_ascii_case("true")).or_else(|| v.as_bool()))
                .unwrap_or(false),
            tenant_id,
            client_id: None,
            server_arm_id: arm_id,
        };
        if let Err(err) = registry.insert(entry) {
            tracing::warn!(
                target: "queryben::ads_bridge::import",
                %err,
                "insert failed; skipping"
            );
            continue;
        }
        connections_imported += 1;
    }

    // Prime the token cache. Best-effort — a keychain deny leaves the account
    // count at 0 but the connections still land. The refresh trade actually
    // happens lazily on first use via the existing acquire_token path.
    let accounts_imported = prime_token_cache_for_accounts(&seen_accounts).await;

    let snippets_imported = import_ads_snippets(&user_dir).unwrap_or(0);

    Ok(AdsImportSummary {
        connections_imported,
        accounts_imported,
        snippets_imported,
    })
}

/// Copy ADS snippet files' contents (name + prefix + body) into QueryBen's
/// `snippets.json`. Best-effort: any error returns None. Deduplicates by
/// snippet name so re-import doesn't append copies.
fn import_ads_snippets(user_dir: &std::path::Path) -> Option<u32> {
    let snippet_dir = user_dir.join("snippets");
    let entries = std::fs::read_dir(&snippet_dir).ok()?;
    let target_path = qb_snippets_path()?;

    let mut collected: Vec<serde_json::Value> = if target_path.exists() {
        std::fs::read_to_string(&target_path)
            .ok()
            .and_then(|s| serde_json::from_str::<Vec<serde_json::Value>>(&s).ok())
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let existing_names: std::collections::HashSet<String> = collected
        .iter()
        .filter_map(|v| v.get("name").and_then(|n| n.as_str()).map(String::from))
        .collect();

    let mut added = 0u32;
    for entry in entries.flatten() {
        let path = entry.path();
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        if ext != "json" && ext != "code-snippets" {
            continue;
        }
        let Ok(raw) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(parsed) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&raw)
        else {
            continue;
        };
        for (name, body) in parsed {
            if existing_names.contains(&name) {
                continue;
            }
            let prefix = body.get("prefix").and_then(|s| s.as_str()).unwrap_or("").to_string();
            let sql = match body.get("body") {
                Some(serde_json::Value::Array(lines)) => lines
                    .iter()
                    .filter_map(|l| l.as_str())
                    .collect::<Vec<_>>()
                    .join("\n"),
                Some(serde_json::Value::String(s)) => s.clone(),
                _ => continue,
            };
            let description = body
                .get("description")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string();
            collected.push(serde_json::json!({
                "name": name,
                "prefix": prefix,
                "body": sql,
                "description": description,
            }));
            added += 1;
        }
    }

    if added > 0 {
        if let Some(parent) = target_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let json = serde_json::to_vec_pretty(&collected).ok()?;
        std::fs::write(&target_path, json).ok()?;
    }
    Some(added)
}

fn qb_snippets_path() -> Option<PathBuf> {
    if let Ok(overridden) = std::env::var(ENV_QB_SNIPPETS_PATH_OVERRIDE) {
        if !overridden.is_empty() {
            return Some(PathBuf::from(overridden));
        }
    }
    #[cfg(target_os = "linux")]
    let root = dirs::config_dir()?;
    #[cfg(not(target_os = "linux"))]
    let root = dirs::data_dir()?;
    Some(root.join("QueryBen").join("snippets.json"))
}

fn discriminant_matches(a: &AuthMode, b: &AuthMode) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
}

fn ads_auth_to_qb(ads_auth: &str) -> AuthMode {
    match ads_auth {
        "AzureMFA" | "AzureMFAAndUser" => AuthMode::AadInteractive,
        "AzureManagedIdentity" => AuthMode::AadManagedIdentity,
        // Everything else — SqlLogin, Integrated, dbaas — maps to SQL auth.
        // Integrated Windows auth isn't a first-class QueryBen mode yet;
        // treating it as SqlLogin keeps the connection importable and the
        // user prompted for creds at reopen instead of dropped on the floor.
        _ => AuthMode::SqlAuth,
    }
}

/// Best-effort: for each MSAL account referenced by an imported connection,
/// try to warm the Azure token cache so the first query doesn't have to
/// re-open the browser. Returns the number of accounts we successfully
/// primed. This does NOT persist anything the user hasn't already agreed to
/// — it only calls the existing keychain-backed try_borrow path.
///
/// Skips entirely when the ADS accessTokenCache file isn't present in the
/// configured cache dir — no point spinning up a runtime and hitting MSAL
/// with no cache to trade.
async fn prime_token_cache_for_accounts(account_ids: &std::collections::HashSet<String>) -> u32 {
    if account_ids.is_empty() {
        return 0;
    }
    if std::env::var(super::ENV_ADS_ROOT_OVERRIDE)
        .map(|v| !v.is_empty())
        .unwrap_or(false)
    {
        return 0;
    }
    if !cache_files_present() {
        return 0;
    }
    if try_borrow_ads_token("https://database.windows.net/")
        .await
        .is_some()
    {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ads_auth_maps_azuremfa_to_aadinteractive() {
        assert!(matches!(
            ads_auth_to_qb("AzureMFA"),
            AuthMode::AadInteractive
        ));
        assert!(matches!(ads_auth_to_qb("SqlLogin"), AuthMode::SqlAuth));
        assert!(matches!(ads_auth_to_qb("Integrated"), AuthMode::SqlAuth));
    }
}
