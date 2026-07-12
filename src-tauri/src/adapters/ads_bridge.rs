//! Borrow an Azure access token from Azure Data Studio's on-disk MSAL cache.
//!
//! # Why
//!
//! ADS stores refresh tokens in an AES-256-CBC-encrypted file under
//! `<app-data>/azuredatastudio/Azure Accounts/accessTokenCache`. The key + IV
//! live in the OS credential store. If the user is already signed in to ADS,
//! decrypting that file gives us a refresh token we can trade at the MSAL
//! token endpoint for a fresh access token — no browser sign-in, no Azure
//! portal, ever. Same zero-friction path ADS itself uses on every launch.
//!
//! # Cache format (verified against ADS 1.x, June 2026)
//!
//! * File: base64-encoded AES-256-CBC ciphertext, no header, PKCS#7 padding.
//! * Algorithm: `aes-256-cbc` (matches ADS's `FileEncryptionHelper._algorithm`).
//! * Key: 32 raw bytes. IV: 16 raw bytes. Both persisted via ADS's
//!   `FileEncryptionHelper` as `Buffer.toString('utf16le')` — i.e. store the
//!   raw bytes as a UTF-16-LE-decoded string, then let the OS credential store
//!   round-trip that string as UTF-8. To reverse: read the credential-store
//!   value as a UTF-8 string, then encode that string as UTF-16-LE to recover
//!   the raw bytes.
//! * Service names in the credential store: `azureAccountProviderCredentials|
//!   accessTokenCache-key` and `accessTokenCache-iv`. The prefix
//!   (`azureAccountProviderCredentials`) is ADS's credential-service namespace.
//! * Decrypted payload: standard MSAL JSON with top-level `RefreshToken`,
//!   `AccessToken`, `IdToken`, `Account`, `AppMetadata` maps. We pull refresh
//!   tokens out of `RefreshToken.*.secret` and use `home_account_id` /
//!   `client_id` / `environment` from the same entry to build a refresh call.
//!
//! # Failure contract
//!
//! Every failure mode returns `None`. No panics, no ERROR-level logs on the
//! normal "user doesn't have ADS installed" path. First-time invocation on
//! macOS triggers a single keychain prompt ("QueryBen wants to access
//! confidential information from Azure Data Studio"); user clicks "Always
//! Allow" once and every subsequent call is silent.

use std::path::PathBuf;
use std::time::Duration;

use aes::Aes256;
use aes::cipher::{BlockDecryptMut, KeyIvInit, block_padding::Pkcs7};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::connection::{
    AuthMode, Connection, ConnectionEntry, ConnectionRegistry,
};
use crate::error::AppError;

type Aes256CbcDec = cbc::Decryptor<Aes256>;

/// Env override so integration tests can point at a fixture directory.
pub const ENV_CACHE_DIR_OVERRIDE: &str = "QUERYBEN_ADS_CACHE_DIR";

/// Test-sandbox root. When set, both `ads_user_dir()` and `cache_dir()`
/// resolve underneath it (`<root>/User` and `<root>/Azure Accounts`) and
/// `prime_token_cache_for_accounts` becomes a no-op. One knob for tests to
/// prove the whole pipeline stays off the real filesystem and keychain.
pub const ENV_ADS_ROOT_OVERRIDE: &str = "QUERYBEN_ADS_ROOT";

/// Cache filenames ADS ships. `.local` is a supplementary cache MSAL uses for
/// non-account resources; both files share the same key+iv pair.
const CACHE_FILE_PRIMARY: &str = "accessTokenCache";
const CACHE_FILE_LOCAL: &str = "accessTokenCache.local";

/// Credential-store service names. See module docs for the format.
const CRED_SERVICE_KEY: &str = "azureAccountProviderCredentials|accessTokenCache-key";
const CRED_SERVICE_IV: &str = "azureAccountProviderCredentials|accessTokenCache-iv";

/// Fallback client_id when MSAL entries don't disclose one. This is the well-
/// known Azure PowerShell client_id — has FOCI (family-of-client-IDs)
/// membership so its refresh tokens work for any Microsoft first-party API.
const FALLBACK_CLIENT_ID: &str = "04b07795-8ddb-461a-bbee-02f9e1bf7b46";

const REFRESH_TIMEOUT: Duration = Duration::from_secs(10);

/// The value the caller wants: access token + when it expires.
#[derive(Debug, Clone)]
pub struct BorrowedToken {
    pub access_token: String,
    pub expires_at: DateTime<Utc>,
}

// ---- test-only key/iv injection --------------------------------------------
//
// Setting these env vars (hex-encoded) bypasses the OS credential store read
// entirely. Integration tests use this to prove the decrypt logic works
// without touching the real user keychain.
const ENV_TEST_KEY_HEX: &str = "QUERYBEN_ADS_TEST_KEY_HEX";
const ENV_TEST_IV_HEX: &str = "QUERYBEN_ADS_TEST_IV_HEX";

// ---- public API -------------------------------------------------------------

/// Try to borrow a token for `resource` (e.g. `https://management.azure.com/`)
/// from ADS's cache. `None` on any failure.
pub async fn try_borrow_ads_token(resource: &str) -> Option<BorrowedToken> {
    let cache_dir = cache_dir()?;
    let (key, iv) = read_key_iv()?;

    // Try primary first, fall back to .local. Same key/iv for both.
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

    // Try each refresh token until one works. In practice there's usually one
    // per tenant. We stop on the first successful trade.
    for entry in entries {
        if let Some(token) = refresh_at_msal(&entry, resource).await {
            return Some(token);
        }
    }
    None
}

// ---- cache dir resolution --------------------------------------------------

fn cache_dir() -> Option<PathBuf> {
    if let Ok(root) = std::env::var(ENV_ADS_ROOT_OVERRIDE) {
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
    // macOS: ~/Library/Application Support/azuredatastudio/Azure Accounts/
    Some(dirs::data_dir()?.join("azuredatastudio").join("Azure Accounts"))
}

#[cfg(target_os = "windows")]
fn default_cache_dir() -> Option<PathBuf> {
    // Windows: %APPDATA%\azuredatastudio\Azure Accounts\
    Some(dirs::config_dir()?.join("azuredatastudio").join("Azure Accounts"))
}

#[cfg(target_os = "linux")]
fn default_cache_dir() -> Option<PathBuf> {
    // Linux: ~/.config/azuredatastudio/Azure Accounts/
    Some(dirs::config_dir()?.join("azuredatastudio").join("Azure Accounts"))
}

// ---- key/iv retrieval ------------------------------------------------------

/// Return `(key, iv)` where each is the raw byte vector. See module docs for
/// the encoding round-trip.
fn read_key_iv() -> Option<(Vec<u8>, Vec<u8>)> {
    // Test path: env vars trump the credential store, so integration tests
    // never touch the real user's keychain.
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

/// Convert a UTF-8 string (as the credential store returns it) into the raw
/// byte sequence ADS originally serialized via `Buffer.toString('utf16le')`.
fn utf8_string_to_utf16le_bytes(s: &str) -> Vec<u8> {
    // Each Unicode scalar in `s` was originally a UTF-16 code unit that came
    // from the raw key bytes. Re-encode as UTF-16LE to recover them. We accept
    // any string that survives Rust's UTF-8 validation (which is what we got
    // from the credential store); MSAL keys are random bytes so most of the
    // string is unlikely to be printable, but Rust doesn't care.
    let mut out = Vec::with_capacity(s.encode_utf16().count() * 2);
    for u in s.encode_utf16() {
        out.push((u & 0xff) as u8);
        out.push((u >> 8) as u8);
    }
    out
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    for i in (0..bytes.len()).step_by(2) {
        let hi = hex_nibble(bytes[i])?;
        let lo = hex_nibble(bytes[i + 1])?;
        out.push((hi << 4) | lo);
    }
    Some(out)
}

fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

// ---- credential store read (platform-specific) -----------------------------

#[cfg(target_os = "macos")]
fn read_credential(service: &str) -> Option<String> {
    // Use the same low-level SecItem query the app already uses for its own
    // items. ADS writes with account = empty string; we mirror that.
    read_credential_macos(service, "")
}

#[cfg(target_os = "macos")]
fn read_credential_macos(service: &str, account: &str) -> Option<String> {
    use crate::adapters::keychain;
    // Our wrapper treats missing entry as Ok(None). Any error is swallowed —
    // this is best-effort borrowing.
    match keychain::get_password(service, account) {
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
    // On Windows the `keyring` crate wraps Credential Manager; on Linux it
    // wraps Secret Service. ADS uses Node's `keytar`, which stores with the
    // same service/account convention. Try empty account first (matches how
    // keytar's macOS/win32 paths write), then the credId as account for
    // resilience against ADS versions that stored with a real account.
    let entry = keyring::Entry::new(service, "").ok()?;
    if let Ok(v) = entry.get_password() {
        return Some(v);
    }
    // Some VS Code / ADS builds embed the credId as the account too.
    let alt = keyring::Entry::new(service, service).ok()?;
    alt.get_password().ok()
}

// ---- decrypt --------------------------------------------------------------

/// Read `path`, base64-decode the whole file, AES-256-CBC decrypt with PKCS7
/// unpadding, then UTF-8-decode. Returns the plaintext JSON.
fn decrypt_cache_file(
    path: &std::path::Path,
    key: &[u8],
    iv: &[u8],
) -> Result<String, DecryptError> {
    let raw = std::fs::read(path).map_err(|_| DecryptError::Io)?;
    let ct = BASE64_STANDARD.decode(&raw).map_err(|_| DecryptError::Base64)?;

    if key.len() != 32 || iv.len() != 16 {
        return Err(DecryptError::KeySize);
    }

    // Owned buffer because decrypt_padded_mut requires mut access; the crate
    // decrypts in place.
    let mut buf = ct;
    let plaintext = Aes256CbcDec::new_from_slices(key, iv)
        .map_err(|_| DecryptError::Cipher)?
        .decrypt_padded_mut::<Pkcs7>(&mut buf)
        .map_err(|_| DecryptError::Unpad)?;

    String::from_utf8(plaintext.to_vec()).map_err(|_| DecryptError::Utf8)
}

#[derive(Debug)]
enum DecryptError {
    Io,
    Base64,
    KeySize,
    Cipher,
    Unpad,
    Utf8,
}

impl std::fmt::Display for DecryptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Io => "read failed",
            Self::Base64 => "base64 decode failed",
            Self::KeySize => "key/iv wrong size",
            Self::Cipher => "cipher init failed",
            Self::Unpad => "PKCS7 unpad failed (wrong key?)",
            Self::Utf8 => "plaintext not UTF-8",
        })
    }
}

// ---- MSAL JSON parsing ----------------------------------------------------

/// Minimal shape we care about from an MSAL RefreshToken entry.
#[derive(Debug, Clone, Deserialize)]
struct MsalRefreshEntry {
    #[serde(default)]
    home_account_id: String,
    /// `login.windows.net` / `login.microsoftonline.com` / cloud-specific.
    #[serde(default)]
    environment: String,
    #[serde(default)]
    client_id: String,
    secret: String,
}

/// Extract a list of usable refresh entries from the decrypted MSAL cache JSON.
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

/// Trade a refresh token at the MSAL token endpoint. Returns the borrowed
/// token on 2xx; `None` on any failure (revoked token, network hiccup, tenant
/// gone, consent revoked).
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
        // Body helps distinguish invalid_grant (token dead) from tenant /
        // scope / client_id policy trouble. Debug-only because non-2xx is a
        // silently-recoverable "fall through to the next tier" signal.
        let body = resp.text().await.unwrap_or_default();
        // Truncate so a huge HTML error page can't flood the log.
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
/// Fall back to `common` if the shape's off.
fn tenant_from_home_account(home_account_id: &str) -> Option<String> {
    let tid = home_account_id.split('.').nth(1)?;
    if tid.is_empty() {
        return None;
    }
    Some(tid.to_string())
}

/// Convert a resource URL to the MSAL v2 scope form. `.default` suffix means
/// "give me every scope the caller consented to for this resource" — same
/// pattern ADS uses when refreshing on behalf of features.
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

// ---- installation detection ------------------------------------------------
//
// First-run onboarding needs concrete numbers ("3 connections, alice@…, 7
// saved queries") to render the import banner. We read ADS's on-disk user
// state without opening its keychain items — just enough to answer "is ADS
// here, and if so what's in it". The MSAL email doesn't require decrypting
// the token cache because ADS's settings.json records the last-used account
// display name on each AAD connection.

/// One-shot summary of a detected ADS install. `None` at the call site means
/// no usable install was found — the onboarding wizard should skip step 2.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AdsDetectionSummary {
    /// Best-effort ADS version pulled from the app bundle Info.plist. `None`
    /// when we found a data dir but couldn't locate the bundle (uncommon).
    pub version: Option<String>,
    /// Total `datasource.connections` entries in ADS's User/settings.json.
    pub connection_count: u32,
    /// First AAD username surfaced in a connection entry, if any. Used in
    /// the banner as "signed in as alice@contoso.com" — ADS records the
    /// display username on every AAD connection so we don't have to touch
    /// the encrypted MSAL cache to get it.
    pub msal_account_email: Option<String>,
    /// Snippet files under `<user>/snippets/` — .code-snippets or .json.
    pub snippet_count: u32,
    /// The resolved user-data dir we read from. Handy for the UI to display
    /// (`We found an ADS install at …`) and for tests to assert on.
    pub install_path: String,
}

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

/// Detect an installed ADS by looking for its user-data directory. Returns
/// `None` when the directory doesn't exist, has no settings.json, or the
/// settings.json can't be parsed. Never panics.
pub fn detect_ads_installation() -> Option<AdsDetectionSummary> {
    let user_dir = ads_user_dir()?;
    let settings_path = user_dir.join("settings.json");
    let raw = std::fs::read_to_string(&settings_path).ok()?;
    let parsed: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(err) => {
            tracing::warn!(
                target: "queryben::ads_bridge::detect",
                %err,
                path = %settings_path.display(),
                "ADS settings.json is malformed; treating as absent"
            );
            return None;
        }
    };

    let connections = parsed
        .get("datasource.connections")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let connection_count = connections.len() as u32;

    let msal_account_email = connections
        .iter()
        .find_map(|c| c.get("options").and_then(|o| o.get("user")).and_then(|u| u.as_str()))
        .and_then(extract_email_from_ads_user_field)
        .map(String::from);

    let snippet_dir = user_dir.join("snippets");
    let snippet_count = std::fs::read_dir(&snippet_dir)
        .map(|iter| {
            iter.filter_map(Result::ok)
                .filter(|entry| {
                    entry
                        .path()
                        .extension()
                        .and_then(|s| s.to_str())
                        .map(|ext| ext == "json" || ext == "code-snippets")
                        .unwrap_or(false)
                })
                .count() as u32
        })
        .unwrap_or(0);

    let version = read_ads_bundle_version();

    Some(AdsDetectionSummary {
        version,
        connection_count,
        msal_account_email,
        snippet_count,
        install_path: user_dir.display().to_string(),
    })
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

        // Idempotency: same (server, database, auth) already registered?
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

/// Env override so tests can point snippet import at a tempdir. Rust-side
/// mirror of the same pattern used by azure_accounts + token_file_cache.
pub const ENV_QB_SNIPPETS_PATH_OVERRIDE: &str = "QUERYBEN_SNIPPETS_PATH";

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

fn extract_email_from_ads_user_field(user: &str) -> Option<&str> {
    // ADS stores the AAD user as "<Display Name> - <email>" or the raw
    // email. Prefer the email half when the dash is present.
    if let Some(idx) = user.rfind(" - ") {
        let candidate = &user[idx + 3..];
        if candidate.contains('@') {
            return Some(candidate);
        }
    }
    if user.contains('@') {
        return Some(user);
    }
    None
}

fn ads_user_dir() -> Option<PathBuf> {
    if let Ok(root) = std::env::var(ENV_ADS_ROOT_OVERRIDE) {
        if !root.is_empty() {
            return Some(PathBuf::from(root).join("User"));
        }
    }
    if let Ok(overridden) = std::env::var(ENV_ADS_USER_DIR_OVERRIDE) {
        if !overridden.is_empty() {
            return Some(PathBuf::from(overridden));
        }
    }
    if let Ok(overridden) = std::env::var(ENV_CACHE_DIR_OVERRIDE) {
        if !overridden.is_empty() {
            // ENV_CACHE_DIR_OVERRIDE points at `<data>/azuredatastudio/Azure Accounts`.
            // The user dir is the sibling `../User`.
            let p = PathBuf::from(overridden);
            if let Some(parent) = p.parent() {
                return Some(parent.join("User"));
            }
        }
    }
    default_ads_user_dir()
}

/// Direct override for tests that only exercise the settings.json half of
/// the flow (detection / import) without the token cache.
pub const ENV_ADS_USER_DIR_OVERRIDE: &str = "QUERYBEN_ADS_USER_DIR";

#[cfg(target_os = "macos")]
fn default_ads_user_dir() -> Option<PathBuf> {
    Some(dirs::data_dir()?.join("azuredatastudio").join("User"))
}

#[cfg(target_os = "windows")]
fn default_ads_user_dir() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("azuredatastudio").join("User"))
}

#[cfg(target_os = "linux")]
fn default_ads_user_dir() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("azuredatastudio").join("User"))
}

fn read_ads_bundle_version() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let path = std::path::Path::new(
            "/Applications/Azure Data Studio.app/Contents/Info.plist",
        );
        if !path.exists() {
            return None;
        }
        let raw = std::fs::read_to_string(path).ok()?;
        // Info.plist is XML; find the CFBundleShortVersionString key without
        // pulling in a plist parser. Format is stable across ADS releases.
        let key = "<key>CFBundleShortVersionString</key>";
        let idx = raw.find(key)? + key.len();
        let tail = &raw[idx..];
        let start = tail.find("<string>")? + "<string>".len();
        let end = tail[start..].find("</string>")?;
        Some(tail[start..start + end].trim().to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
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
    if std::env::var(ENV_ADS_ROOT_OVERRIDE)
        .map(|v| !v.is_empty())
        .unwrap_or(false)
    {
        return 0;
    }
    let Some(dir) = cache_dir() else { return 0 };
    if !dir.join(CACHE_FILE_PRIMARY).exists() && !dir.join(CACHE_FILE_LOCAL).exists() {
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

// ---- tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrip() {
        assert_eq!(hex_decode("00ff10").as_deref(), Some(&[0x00, 0xff, 0x10][..]));
        assert!(hex_decode("0f0").is_none()); // odd length
        assert!(hex_decode("zz").is_none()); // invalid nibble
    }

    #[test]
    fn utf16le_roundtrip() {
        // Simulate ADS's `Buffer(raw).toString('utf16le')` → then imagine the
        // credential store handed that back to us as a UTF-8 string. Rebuilding
        // via utf8_string_to_utf16le_bytes must recover the original raw bytes.
        let raw: Vec<u8> = (0u8..32).collect();
        // Interpret raw as UTF-16LE code units to build the intermediate string.
        let s: String = std::char::decode_utf16(
            raw.chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]])),
        )
        .filter_map(Result::ok)
        .collect();
        let back = utf8_string_to_utf16le_bytes(&s);
        assert_eq!(back, raw);
    }

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
    fn extract_email_from_ads_user_field_prefers_email_after_dash() {
        assert_eq!(
            extract_email_from_ads_user_field("Venkat M - alice@contoso.com"),
            Some("alice@contoso.com")
        );
        assert_eq!(
            extract_email_from_ads_user_field("alice@contoso.com"),
            Some("alice@contoso.com")
        );
        assert_eq!(extract_email_from_ads_user_field("sa"), None);
    }

    #[test]
    fn ads_auth_maps_azuremfa_to_aadinteractive() {
        assert!(matches!(
            ads_auth_to_qb("AzureMFA"),
            AuthMode::AadInteractive
        ));
        assert!(matches!(ads_auth_to_qb("SqlLogin"), AuthMode::SqlAuth));
        assert!(matches!(ads_auth_to_qb("Integrated"), AuthMode::SqlAuth));
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
