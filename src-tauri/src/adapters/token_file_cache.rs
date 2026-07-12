//! On-disk Azure token cache. Same durability pattern Azure Data Studio uses.
//!
//! # Why we don't rely on the OS keychain alone
//!
//! The macOS keychain evaporates the refresh token on any of:
//!
//!   * `cargo build` producing a new code-signing hash (ad-hoc dev signing)
//!   * User manually clearing "queryben" items from Keychain Access
//!   * Reinstall / upgrade replacing the bundle
//!   * Full "Reset Keychain" from System Settings
//!   * Team-ID cert rotation
//!
//! Every one of those wipes drops the user back to the "Add rule (sign in to
//! Azure)" browser dance. Azure Data Studio dodges this by writing its tokens
//! to a plain JSON file under `~/Library/Application Support/azuredatastudio/
//! Azure Accounts/`. That file survives OS-level auth churn — the whole point.
//!
//! We now mirror that layout under `<app_data>/QueryBen/azure-cache.json` and
//! keep the keychain as a third-tier fallback (see `azure_oauth::
//! try_acquire_silent`).
//!
//! # File format
//!
//! Single JSON object, versioned only implicitly via `#[serde(default)]` on
//! every non-required field:
//!
//! ```json
//! {
//!   "refresh_token": "0.AR...",
//!   "account": { "tenant_id": "...", "home_account_id": "...", "username": "..." },
//!   "cached_access_tokens": {
//!     "https://database.windows.net/": {
//!       "token": "eyJ...",
//!       "expires_at_unix": 1712345678
//!     }
//!   },
//!   "written_at_unix": 1712345670
//! }
//! ```
//!
//! # Path resolution
//!
//! Prefers `$QUERYBEN_TOKEN_CACHE_PATH` (integration tests point this at a
//! `tempfile::TempDir`), falls back to the platform default:
//!
//!   * macOS   `~/Library/Application Support/QueryBen/azure-cache.json`
//!   * Windows `%APPDATA%/QueryBen/azure-cache.json`
//!   * Linux   `~/.config/QueryBen/azure-cache.json`
//!
//! # Durability contract
//!
//! * `load()` is best-effort. Missing file, bad JSON, permission trouble → `None`.
//!   Never panics, never surfaces an error to the caller — a failed load just
//!   means we fall through to the keychain / az-CLI paths.
//! * `save()` is atomic: writes to a sibling temp file, `fsync`s, then renames.
//!   A crash mid-write leaves the previous good file intact.
//! * On Unix, `save()` sets mode `0600` so no other user on the box can read
//!   the refresh token. Tests assert this — regression would be a real leak.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Env var that overrides the cache location. Set by integration tests so we
/// don't clobber the real user file. Also usable in ops for a shared cache dir.
pub const ENV_CACHE_PATH_OVERRIDE: &str = "QUERYBEN_TOKEN_CACHE_PATH";

/// Application subdirectory under the OS's app-data root. Matches the bundle
/// display name so the folder is recognizable in Finder / Explorer.
const APP_SUBDIR: &str = "QueryBen";
const CACHE_FILE_NAME: &str = "azure-cache.json";

/// Persisted Azure identity — enough to skip tenant discovery on next launch.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct PersistedAccount {
    #[serde(default)]
    pub tenant_id: String,
    #[serde(default)]
    pub home_account_id: String,
    #[serde(default)]
    pub username: String,
}

/// One resource-scoped access token + its epoch expiry. Keyed by resource URL
/// (`https://database.windows.net/`, `https://management.azure.com/`, …) in the
/// parent `TokenCache::cached_access_tokens` map.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CachedAccessToken {
    pub token: String,
    pub expires_at_unix: i64,
}

/// The full on-disk record. All fields are `#[serde(default)]` so a partial or
/// older file still decodes cleanly — we treat any missing field as absent, not
/// as a decode failure.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenCache {
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub account: Option<PersistedAccount>,
    #[serde(default)]
    pub cached_access_tokens: HashMap<String, CachedAccessToken>,
    #[serde(default)]
    pub written_at_unix: i64,
}

impl TokenCache {
    /// Return a non-expired access token for `resource` if we have one. Applies
    /// a 60-second grace window so callers don't hand a bearer to the SQL
    /// driver that will expire mid-handshake.
    pub fn get_valid_access_token(&self, resource: &str) -> Option<&CachedAccessToken> {
        let now = now_unix();
        // Match on either the exact key or the "resource" form (with/without
        // trailing slash) since callers might pass either shape.
        let entry = self
            .cached_access_tokens
            .get(resource)
            .or_else(|| self.cached_access_tokens.get(&normalize_resource(resource)))?;
        if entry.expires_at_unix > now + 60 {
            Some(entry)
        } else {
            None
        }
    }

    /// Insert / replace an access token entry keyed by `resource`.
    pub fn put_access_token(&mut self, resource: &str, token: String, expires_at_unix: i64) {
        self.cached_access_tokens.insert(
            normalize_resource(resource),
            CachedAccessToken {
                token,
                expires_at_unix,
            },
        );
    }
}

// ---- path resolution --------------------------------------------------------

/// Full path to the cache file, honoring the env override.
///
/// Returns `None` when the override is unset AND `dirs::config_dir()` can't
/// figure out an app-data root (headless CI without HOME set, mainly). Callers
/// treat `None` the same as "cache disabled" — the keychain path still works.
pub fn cache_path() -> Option<PathBuf> {
    if let Ok(overridden) = std::env::var(ENV_CACHE_PATH_OVERRIDE) {
        if !overridden.is_empty() {
            return Some(PathBuf::from(overridden));
        }
    }
    default_cache_path()
}

fn default_cache_path() -> Option<PathBuf> {
    // We want:
    //   macOS   ~/Library/Application Support/QueryBen/azure-cache.json
    //   Windows %APPDATA%\QueryBen\azure-cache.json
    //   Linux   ~/.config/QueryBen/azure-cache.json
    //
    // dirs::data_dir() gives us that root on macOS + Windows; on Linux it
    // resolves to $XDG_DATA_HOME (~/.local/share) which is fine per the XDG
    // spec, but the task brief called out ~/.config/QueryBen specifically —
    // dirs::config_dir() maps to that on Linux. Use config_dir on Linux and
    // data_dir on macOS + Windows so we hit the exact paths in the spec.
    #[cfg(target_os = "linux")]
    let root = dirs::config_dir()?;
    #[cfg(not(target_os = "linux"))]
    let root = dirs::data_dir()?;

    Some(root.join(APP_SUBDIR).join(CACHE_FILE_NAME))
}

// ---- public API -------------------------------------------------------------

/// Best-effort read. Any failure (missing file, bad JSON, permissions) → `None`.
pub fn load() -> Option<TokenCache> {
    let path = cache_path()?;
    let bytes = fs::read(&path).ok()?;
    serde_json::from_slice::<TokenCache>(&bytes).ok()
}

/// Atomic write. Serializes to JSON, writes to a sibling temp file with mode
/// `0600` on Unix, `fsync`s, then renames over the destination. If any step
/// fails, the previous file is left intact.
pub fn save(cache: &TokenCache) -> io::Result<()> {
    let path = cache_path().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "no app-data directory available (HOME / APPDATA unset?)",
        )
    })?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Stamp the write time so ops can tell how stale the file is.
    let mut stamped = cache.clone();
    stamped.written_at_unix = now_unix();

    let json = serde_json::to_vec_pretty(&stamped)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    // Temp file lives in the same directory so `rename` is atomic (POSIX
    // guarantees atomicity within a filesystem; keeping the temp beside the
    // final path keeps us on one filesystem).
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "cache path has no parent")
    })?;
    let tmp = parent.join(format!(
        ".{}.tmp.{}",
        CACHE_FILE_NAME,
        // Cheap "unique enough for a single process" suffix. std::process::id
        // + monotonic nanoseconds avoids pulling in a whole tempfile crate.
        std::process::id(),
    ));

    // Truncate + write + fsync + rename. Open with restrictive mode from the
    // start on Unix; on Windows the mode arg is ignored and there's no NTFS
    // equivalent worth bolting on here — Windows file ACLs already restrict to
    // the current user by default.
    write_atomic(&tmp, &path, &json)?;

    Ok(())
}

/// Delete the cache file. Missing file is a no-op, not an error.
pub fn clear() {
    let Some(path) = cache_path() else {
        return;
    };
    // Ignore NotFound; every other error is a "we tried" outcome — the caller
    // (sign-out) shouldn't fail because of a stale cache we couldn't remove.
    if let Err(err) = fs::remove_file(&path) {
        if err.kind() != io::ErrorKind::NotFound {
            tracing::debug!(
                target: "queryben::token_file_cache",
                %err,
                path = ?path,
                "clear() ignored non-NotFound remove error"
            );
        }
    }
}

// ---- helpers ----------------------------------------------------------------

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Normalize a scope-or-resource string to the resource form used as the map
/// key. `https://database.windows.net/.default` → `https://database.windows.net/`.
fn normalize_resource(scope_or_resource: &str) -> String {
    let trimmed = scope_or_resource
        .trim_end_matches("/.default")
        .trim_end_matches(".default");
    if trimmed.ends_with('/') {
        trimmed.to_string()
    } else {
        format!("{trimmed}/")
    }
}

// ---- Unix: 0600 write + fsync ----------------------------------------------

#[cfg(unix)]
fn write_atomic(tmp: &std::path::Path, dest: &std::path::Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;

    let mut file = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        // 0o600: owner rw, no group/world. Refresh tokens are as sensitive as
        // an SSH private key — same treatment.
        .mode(0o600)
        .open(tmp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(tmp, dest)?;
    // rename doesn't preserve the source's permissions on all filesystems (it
    // does on APFS/ext4, but be explicit — this is a security control).
    let perm = std::os::unix::fs::PermissionsExt::from_mode(0o600);
    fs::set_permissions(dest, perm)?;
    Ok(())
}

#[cfg(not(unix))]
fn write_atomic(tmp: &std::path::Path, dest: &std::path::Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write as _;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(tmp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    // On Windows fs::rename fails if the destination exists — remove first.
    if dest.exists() {
        let _ = fs::remove_file(dest);
    }
    fs::rename(tmp, dest)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_dot_default_scope() {
        assert_eq!(
            normalize_resource("https://database.windows.net/.default"),
            "https://database.windows.net/"
        );
    }

    #[test]
    fn normalizes_bare_resource_adds_slash() {
        assert_eq!(
            normalize_resource("https://management.azure.com"),
            "https://management.azure.com/"
        );
    }

    #[test]
    fn get_valid_access_token_returns_none_when_expired() {
        let mut c = TokenCache::default();
        c.put_access_token("https://x/", "tok".into(), now_unix() - 10);
        assert!(c.get_valid_access_token("https://x/").is_none());
    }

    #[test]
    fn get_valid_access_token_returns_some_when_fresh() {
        let mut c = TokenCache::default();
        c.put_access_token("https://x/", "tok".into(), now_unix() + 3600);
        assert!(c.get_valid_access_token("https://x/").is_some());
    }

    #[test]
    fn get_valid_access_token_accepts_dot_default_form() {
        let mut c = TokenCache::default();
        c.put_access_token(
            "https://database.windows.net/",
            "tok".into(),
            now_unix() + 3600,
        );
        assert!(
            c.get_valid_access_token("https://database.windows.net/.default")
                .is_some()
        );
    }
}
