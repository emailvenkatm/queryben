//! Detect an installed ADS by looking for its user-data directory and
//! summarizing what's inside it (connection count, first AAD email, snippet
//! count, bundle version).

use std::path::PathBuf;

use serde::Serialize;

use super::msal_cache::ENV_CACHE_DIR_OVERRIDE;

/// Direct override for tests that only exercise the settings.json half of
/// the flow (detection / import) without the token cache.
pub const ENV_ADS_USER_DIR_OVERRIDE: &str = "QUERYBEN_ADS_USER_DIR";

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

pub(super) fn ads_user_dir() -> Option<PathBuf> {
    if let Ok(root) = std::env::var(super::ENV_ADS_ROOT_OVERRIDE) {
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

pub(super) fn extract_email_from_ads_user_field(user: &str) -> Option<&str> {
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
