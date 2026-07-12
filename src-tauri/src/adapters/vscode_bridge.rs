//! Borrow an Azure access token from VS Code's Microsoft Authentication cache.
//!
//! VS Code stores refresh tokens for its Microsoft account provider (the one
//! powering the Azure Account extension, GitHub Copilot's Azure sign-in, and
//! `vscode-microsoft-authentication`) directly in the OS credential store. Unlike
//! ADS, there's no on-disk cipher file — the entire token payload is stored as
//! a plain JSON string in the credential value.
//!
//! Service names in the credential store, per VS Code's built-in
//! `microsoft-authentication` extension:
//!
//!   * Service: `vscode-microsoft-authentication` (also
//!     `vscodemicrosoft` on some older builds; we try both).
//!   * Account: a stable id string built from `<client_id>-<scopes>-<tenant>`
//!     that we can't enumerate up front. We enumerate keychain items on macOS
//!     via `SecItemCopyMatching` with the service filter and no account. On
//!     other OSes the `keyring` crate can only fetch by (service, account), so
//!     the Linux/Windows paths currently return `None` and rely on the ADS +
//!     az CLI + file cache tiers upstream.
//!
//! This module is a *soft* bridge — if VS Code isn't installed, or its cache
//! shape doesn't match, the caller falls through to the next tier. Never
//! panics, never surfaces an ERROR-level log during normal launch.

use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Deserialize;

/// The public "Visual Studio Code" first-party client_id. Referenced here as a
/// documented fallback when parsed cache entries omit it; VS Code always
/// embeds the concrete client_id in the stored payload so we rarely need this.
const VSCODE_CLIENT_ID: &str = "aebc6443-996d-45c2-90f0-388ff96faa56";

/// Env override for tests: bypass the credential-store scan and hand us a raw
/// refresh token directly.
pub const ENV_TEST_REFRESH_TOKEN: &str = "QUERYBEN_VSCODE_TEST_REFRESH_TOKEN";
pub const ENV_TEST_TENANT: &str = "QUERYBEN_VSCODE_TEST_TENANT";

const REFRESH_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone)]
pub struct BorrowedToken {
    pub access_token: String,
    pub expires_at: DateTime<Utc>,
}

/// Try to borrow an access token for `resource` from VS Code's cache.
pub async fn try_borrow_vscode_token(resource: &str) -> Option<BorrowedToken> {
    // Test path: skip the credential store entirely.
    if let Ok(rt) = std::env::var(ENV_TEST_REFRESH_TOKEN) {
        if !rt.is_empty() {
            let tenant = std::env::var(ENV_TEST_TENANT).unwrap_or_else(|_| "common".into());
            return refresh_at_msal(&rt, VSCODE_CLIENT_ID, &tenant, resource).await;
        }
    }

    let entries = read_vscode_credentials()?;
    for entry in entries {
        if let Some(tok) = refresh_at_msal(
            &entry.refresh_token,
            entry.client_id.as_deref().unwrap_or(VSCODE_CLIENT_ID),
            entry.tenant_id.as_deref().unwrap_or("common"),
            resource,
        )
        .await
        {
            return Some(tok);
        }
    }
    None
}

struct VsCodeEntry {
    refresh_token: String,
    client_id: Option<String>,
    tenant_id: Option<String>,
}

/// Enumerate VS Code Microsoft-Authentication credential entries. See module
/// docs — this only works on macOS today; other OSes return `None` because the
/// `keyring` crate can't enumerate.
#[cfg(target_os = "macos")]
fn read_vscode_credentials() -> Option<Vec<VsCodeEntry>> {
    // Try the two service names VS Code uses across versions. The current
    // stable ships with `vscode-microsoft-authentication`; older insiders /
    // 2022-vintage builds used `vscodemicrosoft`.
    let services = ["vscode-microsoft-authentication", "vscodemicrosoft"];
    let mut out = Vec::new();
    for service in services {
        for raw in copy_all_generic_passwords(service) {
            if let Some(entry) = parse_vscode_credential(&raw) {
                out.push(entry);
            }
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

#[cfg(not(target_os = "macos"))]
fn read_vscode_credentials() -> Option<Vec<VsCodeEntry>> {
    // TODO: Windows Credential Manager enumeration via CredEnumerateW.
    // TODO: Linux Secret Service enumeration via `libsecret`'s search API.
    // The ADS bridge covers the common case on both OSes; VS Code borrowing
    // on non-macOS is a future improvement. Silently return None so the
    // upstream chain falls through cleanly.
    None
}

/// VS Code stores each token as a JSON object with (at least):
///   * `refreshToken` (or `refresh_token`) — the value we need.
///   * optional `account.id`, `id`, or `scopes` — from which we can extract
///     tenant. In practice the tenant appears in `scopes` as the authority
///     URL segment, so we scan for a GUID.
fn parse_vscode_credential(raw: &str) -> Option<VsCodeEntry> {
    // Try the modern shape first: `{"refreshToken": "...", ...}`.
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Payload {
        refresh_token: Option<String>,
        #[serde(default)]
        client_id: Option<String>,
        #[serde(default)]
        tenant_id: Option<String>,
        #[serde(default)]
        scopes: Option<Vec<String>>,
        #[serde(default)]
        authority: Option<String>,
    }
    let parsed: Payload = serde_json::from_str(raw).ok()?;
    let refresh_token = parsed.refresh_token?;
    if refresh_token.is_empty() {
        return None;
    }
    let tenant_id = parsed
        .tenant_id
        .or_else(|| parsed.authority.as_deref().and_then(extract_tenant_from_authority))
        .or_else(|| {
            parsed
                .scopes
                .as_ref()
                .and_then(|v| v.iter().find_map(|s| extract_tenant_from_scope(s)))
        });
    Some(VsCodeEntry {
        refresh_token,
        client_id: parsed.client_id,
        tenant_id,
    })
}

fn extract_tenant_from_authority(authority: &str) -> Option<String> {
    // authority looks like `https://login.microsoftonline.com/<guid>` — the
    // last non-empty path segment is the tenant.
    authority
        .rsplit('/')
        .find(|s| !s.is_empty())
        .map(str::to_string)
}

fn extract_tenant_from_scope(scope: &str) -> Option<String> {
    // Some scopes embed tenant as `VSCODE_TENANT:<guid>`.
    let prefix = "VSCODE_TENANT:";
    scope.strip_prefix(prefix).map(str::to_string)
}

// ---- macOS: enumerate all generic passwords for a service -----------------

#[cfg(target_os = "macos")]
fn copy_all_generic_passwords(service: &str) -> Vec<String> {
    use std::ffi::c_void;
    use std::ptr;

    use core_foundation::array::CFArray;
    use core_foundation::base::{CFRelease, CFType, CFTypeRef, TCFType};
    use core_foundation::boolean::CFBoolean;
    use core_foundation::data::CFData;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::CFString;
    use core_foundation_sys::base::CFTypeRef as SysCFTypeRef;
    use security_framework_sys::base::errSecSuccess;
    use security_framework_sys::item::{
        kSecAttrService, kSecClass, kSecClassGenericPassword, kSecMatchLimit, kSecMatchLimitAll,
        kSecReturnData,
    };
    use security_framework_sys::keychain_item::SecItemCopyMatching;

    unsafe {
        let pairs: Vec<(CFString, CFType)> = vec![
            (
                CFString::wrap_under_get_rule(kSecClass),
                CFString::wrap_under_get_rule(kSecClassGenericPassword).as_CFType(),
            ),
            (
                CFString::wrap_under_get_rule(kSecAttrService),
                CFString::new(service).as_CFType(),
            ),
            (
                CFString::wrap_under_get_rule(kSecReturnData),
                CFBoolean::true_value().as_CFType(),
            ),
            (
                CFString::wrap_under_get_rule(kSecMatchLimit),
                CFString::wrap_under_get_rule(kSecMatchLimitAll).as_CFType(),
            ),
        ];
        let dict = CFDictionary::from_CFType_pairs(&pairs);
        let mut out: CFTypeRef = ptr::null();
        let status = SecItemCopyMatching(
            dict.as_concrete_TypeRef(),
            &mut out as *mut _ as *mut SysCFTypeRef,
        );
        if status != errSecSuccess || out.is_null() {
            return Vec::new();
        }
        // The API returns a CFArray of CFData when there are multiple matches,
        // or a single CFData when there's exactly one. We handle both.
        let cf: CFType = CFType::wrap_under_create_rule(out);
        let type_id = cf.type_of();
        let mut results: Vec<String> = Vec::new();

        if type_id == CFArray::<CFData>::type_id() {
            let arr: CFArray<CFData> = CFArray::wrap_under_get_rule(out as _);
            for i in 0..arr.len() {
                if let Some(item) = arr.get(i) {
                    if let Ok(s) = std::str::from_utf8(item.bytes()) {
                        results.push(s.to_string());
                    }
                }
            }
            // wrap_under_get_rule doesn't take ownership; the outer `cf` guard
            // handles the release.
        } else if type_id == CFData::type_id() {
            let d: CFData = CFData::wrap_under_get_rule(out as _);
            if let Ok(s) = std::str::from_utf8(d.bytes()) {
                results.push(s.to_string());
            }
        } else {
            // Unknown shape — release and bail.
            CFRelease(out as *const c_void);
        }
        results
    }
}

// ---- MSAL refresh trade ---------------------------------------------------

async fn refresh_at_msal(
    refresh_token: &str,
    client_id: &str,
    tenant: &str,
    resource: &str,
) -> Option<BorrowedToken> {
    let scope = normalize_scope(resource);
    let tenant = if tenant.is_empty() { "common" } else { tenant };
    let url = format!("https://login.microsoftonline.com/{tenant}/oauth2/v2.0/token");
    let form: Vec<(&str, &str)> = vec![
        ("client_id", client_id),
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
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
        tracing::debug!(
            target: "queryben::vscode_bridge",
            status = resp.status().as_u16(),
            "VS Code refresh trade returned non-2xx"
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_modern_shape() {
        let raw = r#"{"refreshToken":"rt-abc","clientId":"cid-1","tenantId":"tid-1"}"#;
        let entry = parse_vscode_credential(raw).expect("parses");
        assert_eq!(entry.refresh_token, "rt-abc");
        assert_eq!(entry.client_id.as_deref(), Some("cid-1"));
        assert_eq!(entry.tenant_id.as_deref(), Some("tid-1"));
    }

    #[test]
    fn parse_derives_tenant_from_authority() {
        let raw = r#"{"refreshToken":"rt-abc","authority":"https://login.microsoftonline.com/00000000-0000-0000-0000-000000000001"}"#;
        let entry = parse_vscode_credential(raw).expect("parses");
        assert_eq!(
            entry.tenant_id.as_deref(),
            Some("00000000-0000-0000-0000-000000000001")
        );
    }

    #[test]
    fn parse_rejects_no_refresh() {
        let raw = r#"{"foo":"bar"}"#;
        assert!(parse_vscode_credential(raw).is_none());
    }

    #[test]
    fn extract_tenant_from_scope_prefix_works() {
        assert_eq!(
            extract_tenant_from_scope("VSCODE_TENANT:my-tid").as_deref(),
            Some("my-tid")
        );
        assert!(extract_tenant_from_scope("no-prefix").is_none());
    }
}
