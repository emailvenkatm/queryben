//! Read + parse ADS's MSAL cache and trade its refresh tokens at the MSAL
//! token endpoint.

use std::path::PathBuf;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::decrypt::{decrypt_cache_file, hex_decode, utf8_string_to_utf16le_bytes};

/// Env override so integration tests can point at a fixture directory.
pub const ENV_CACHE_DIR_OVERRIDE: &str = "QUERYBEN_ADS_CACHE_DIR";

/// Cache filenames ADS ships. `.local` is a supplementary cache MSAL uses for
/// non-account resources; both files share the same key+iv pair.
const CACHE_FILE_PRIMARY: &str = "accessTokenCache";
const CACHE_FILE_LOCAL: &str = "accessTokenCache.local";

/// Credential-store service names. See the parent module docs for the format.
const CRED_SERVICE_KEY: &str = "azureAccountProviderCredentials|accessTokenCache-key";
const CRED_SERVICE_IV: &str = "azureAccountProviderCredentials|accessTokenCache-iv";

/// Fallback client_id when MSAL entries don't disclose one. This is the well-
/// known Azure PowerShell client_id — has FOCI (family-of-client-IDs)
/// membership so its refresh tokens work for any Microsoft first-party API.
const FALLBACK_CLIENT_ID: &str = "04b07795-8ddb-461a-bbee-02f9e1bf7b46";

const REFRESH_TIMEOUT: Duration = Duration::from_secs(10);

// Setting these env vars (hex-encoded) bypasses the OS credential store read
// entirely. Integration tests use this to prove the decrypt logic works
// without touching the real user keychain.
const ENV_TEST_KEY_HEX: &str = "QUERYBEN_ADS_TEST_KEY_HEX";
const ENV_TEST_IV_HEX: &str = "QUERYBEN_ADS_TEST_IV_HEX";

/// The value the caller wants: access token + when it expires.
#[derive(Debug, Clone)]
pub struct BorrowedToken {
    pub access_token: String,
    pub expires_at: DateTime<Utc>,
}

/// Try to borrow a token for `resource` (e.g. `https://management.azure.com/`)
/// from ADS's cache. `None` on any failure.
pub async fn try_borrow_ads_token(resource: &str) -> Option<BorrowedToken> {
    let cache_dir = cache_dir()?;
    let (key, iv) = read_key_iv()?;

    let paths = [cache_dir.join(CACHE_FILE_PRIMARY), cache_dir.join(CACHE_FILE_LOCAL)];
    for path in paths.iter().filter(|p| p.exists()) {
        if let Some(token) = try_borrow_from_file(path, &key, &iv, resource).await {
            return Some(token);
        }
    }
    None
}

/// Public for tests: decrypt a specific file with an injected key+iv and try
/// to refresh a token against `resource`. Skips the credential-store read.
pub async fn try_borrow_from_file(
    path: &std::path::Path,
    key: &[u8],
    iv: &[u8],
    resource: &str,
) -> Option<BorrowedToken> {
    let cache_json = decrypt_cache_file(path, key, iv).ok()?;
    let entries = extract_refresh_entries(&cache_json)?;

    for entry in entries {
        if let Some(token) = refresh_at_msal(&entry, resource).await {
            return Some(token);
        }
    }
    None
}

pub(super) fn cache_dir() -> Option<PathBuf> {
    if let Ok(root) = std::env::var(super::ENV_ADS_ROOT_OVERRIDE) {
        if !root.is_empty() {
            return Some(PathBuf::from(root).join("Azure Accounts"));
        }
    }
    if let Ok(overridden) = std::env::var(ENV_CACHE_DIR_OVERRIDE) {
        if !overridden.is_empty() {
            return Some(PathBuf::from(overridden));
        }
    }
    default_cache_dir()
}

#[cfg(target_os = "macos")]
fn default_cache_dir() -> Option<PathBuf> {
    Some(dirs::data_dir()?.join("azuredatastudio").join("Azure Accounts"))
}

#[cfg(target_os = "windows")]
fn default_cache_dir() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("azuredatastudio").join("Azure Accounts"))
}

#[cfg(target_os = "linux")]
fn default_cache_dir() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("azuredatastudio").join("Azure Accounts"))
}

fn read_key_iv() -> Option<(Vec<u8>, Vec<u8>)> {
    if let (Ok(k), Ok(i)) = (std::env::var(ENV_TEST_KEY_HEX), std::env::var(ENV_TEST_IV_HEX)) {
        let key = hex_decode(&k)?;
        let iv = hex_decode(&i)?;
        if key.len() == 32 && iv.len() == 16 {
            return Some((key, iv));
        }
        return None;
    }

    let key_str = read_credential(CRED_SERVICE_KEY)?;
    let iv_str = read_credential(CRED_SERVICE_IV)?;
    let key = utf8_string_to_utf16le_bytes(&key_str);
    let iv = utf8_string_to_utf16le_bytes(&iv_str);
    if key.len() != 32 || iv.len() != 16 {
        tracing::debug!(
            target: "queryben::ads_bridge",
            key_len = key.len(),
            iv_len = iv.len(),
            "ADS key/iv wrong size after utf16le decode; skipping"
        );
        return None;
    }
    Some((key, iv))
}

#[cfg(target_os = "macos")]
fn read_credential(service: &str) -> Option<String> {
    use crate::adapters::keychain;
    match keychain::get_password(service, "") {
        Ok(v) => v,
        Err(err) => {
            tracing::debug!(
                target: "queryben::ads_bridge",
                %err,
                service,
                "ADS credential read failed"
            );
            None
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn read_credential(service: &str) -> Option<String> {
    let entry = keyring::Entry::new(service, "").ok()?;
    if let Ok(v) = entry.get_password() {
        return Some(v);
    }
    let alt = keyring::Entry::new(service, service).ok()?;
    alt.get_password().ok()
}

/// Minimal shape we care about from an MSAL RefreshToken entry.
#[derive(Debug, Clone, Deserialize)]
struct MsalRefreshEntry {
    #[serde(default)]
    home_account_id: String,
    #[serde(default)]
    environment: String,
    #[serde(default)]
    client_id: String,
    secret: String,
}

fn extract_refresh_entries(json: &str) -> Option<Vec<MsalRefreshEntry>> {
    let root: serde_json::Value = serde_json::from_str(json).ok()?;
    let map = root.get("RefreshToken")?.as_object()?;
    let mut out = Vec::with_capacity(map.len());
    for (_k, v) in map {
        if let Ok(entry) = serde_json::from_value::<MsalRefreshEntry>(v.clone()) {
            if !entry.secret.is_empty() {
                out.push(entry);
            }
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

async fn refresh_at_msal(entry: &MsalRefreshEntry, resource: &str) -> Option<BorrowedToken> {
    let tenant = tenant_from_home_account(&entry.home_account_id).unwrap_or("common".to_string());
    let environment = if entry.environment.is_empty() {
        "login.microsoftonline.com"
    } else {
        entry.environment.as_str()
    };
    let client_id = if entry.client_id.is_empty() {
        FALLBACK_CLIENT_ID
    } else {
        entry.client_id.as_str()
    };
    let url = format!("https://{environment}/{tenant}/oauth2/v2.0/token");
    let scope = normalize_scope(resource);

    let form: Vec<(&str, &str)> = vec![
        ("client_id", client_id),
        ("grant_type", "refresh_token"),
        ("refresh_token", entry.secret.as_str()),
        ("scope", scope.as_str()),
    ];

    let client = reqwest::Client::builder()
        .user_agent("QueryBen/0.1.0")
        .timeout(REFRESH_TIMEOUT)
        .build()
        .ok()?;

    let resp = client
        .post(&url)
        .header("Accept", "application/json")
        .form(&form)
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        let snippet: String = body.chars().take(400).collect();
        tracing::debug!(
            target: "queryben::ads_bridge",
            status = status,
            body = %snippet,
            "ADS refresh trade returned non-2xx"
        );
        return None;
    }

    #[derive(Deserialize)]
    struct TokenResponse {
        access_token: String,
        #[serde(default)]
        expires_in: i64,
    }

    let parsed = resp.json::<TokenResponse>().await.ok()?;
    if parsed.access_token.is_empty() {
        return None;
    }
    let expires_at = Utc::now()
        + chrono::Duration::seconds(if parsed.expires_in > 0 { parsed.expires_in } else { 3300 });
    Some(BorrowedToken {
        access_token: parsed.access_token,
        expires_at,
    })
}

/// MSAL home_account_id shape: `<oid>.<tid>` where `<tid>` is the tenant GUID.
fn tenant_from_home_account(home_account_id: &str) -> Option<String> {
    let tid = home_account_id.split('.').nth(1)?;
    if tid.is_empty() {
        return None;
    }
    Some(tid.to_string())
}

fn normalize_scope(resource: &str) -> String {
    let trimmed = resource
        .trim_end_matches("/.default")
        .trim_end_matches(".default");
    let base = if trimmed.ends_with('/') {
        trimmed.trim_end_matches('/').to_string()
    } else {
        trimmed.to_string()
    };
    format!("{base}/.default")
}

/// Whether either ADS cache file exists in the resolved cache dir. Called by
/// the import path before spinning up a runtime to prime the token cache.
pub(super) fn cache_files_present() -> bool {
    let Some(dir) = cache_dir() else {
        return false;
    };
    dir.join(CACHE_FILE_PRIMARY).exists() || dir.join(CACHE_FILE_LOCAL).exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_scope_dot_default_form() {
        assert_eq!(
            normalize_scope("https://management.azure.com/"),
            "https://management.azure.com/.default"
        );
        assert_eq!(
            normalize_scope("https://management.azure.com/.default"),
            "https://management.azure.com/.default"
        );
    }

    #[test]
    fn tenant_from_home_account_extracts_second_segment() {
        assert_eq!(
            tenant_from_home_account("oid-here.tenant-guid-here").as_deref(),
            Some("tenant-guid-here")
        );
        assert!(tenant_from_home_account("single-segment").is_none());
    }

    #[test]
    fn extract_refresh_entries_returns_none_for_empty_map() {
        let json = r#"{ "RefreshToken": {} }"#;
        assert!(extract_refresh_entries(json).is_none());
    }

    #[test]
    fn extract_refresh_entries_picks_secret_bearing_entries() {
        let json = r#"{
            "RefreshToken": {
                "k1": {
                    "home_account_id": "a.b",
                    "environment": "login.windows.net",
                    "client_id": "cid",
                    "secret": "rt-xyz",
                    "credential_type": "RefreshToken"
                }
            }
        }"#;
        let entries = extract_refresh_entries(json).expect("some");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].secret, "rt-xyz");
    }
}
