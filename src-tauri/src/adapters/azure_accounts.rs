//! Persistent list of signed-in Azure accounts.
//!
//! One JSON file at `<app_data>/azure-accounts.json` records every account the
//! user has interactively signed in with. Keyed by the MSAL-style
//! `home_account_id` (`<oid>.<tenant_id>`); the same key that scopes each
//! account's refresh token in the OS keychain.
//!
//! Layout:
//!
//! ```json
//! [
//!   {
//!     "account_id": "<oid>.<tenant>",
//!     "username":   "alice@company.com",
//!     "tenant_id":  "…",
//!     "display_name": "Alice Anderson",
//!     "last_signed_in": "2026-07-15T22:11:00Z"
//!   }
//! ]
//! ```
//!
//! Persistence rules:
//!   * `load()` on missing / corrupt file returns an empty list, not an error —
//!     an unreadable registry should never brick sign-in.
//!   * `save()` is atomic (temp file + rename) so a crash mid-write leaves the
//!     previous file intact.
//!   * `$QUERYBEN_ACCOUNTS_PATH` overrides the on-disk location so tests can
//!     point at a `tempfile::TempDir`.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::AppError;

pub const ENV_ACCOUNTS_PATH_OVERRIDE: &str = "QUERYBEN_ACCOUNTS_PATH";

const FILE_NAME: &str = "azure-accounts.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AccountRegistryEntry {
    pub account_id: String,
    pub username: String,
    pub tenant_id: String,
    pub display_name: Option<String>,
    pub last_signed_in: DateTime<Utc>,
}

/// Resolve the on-disk path.
///
/// Prefers `$QUERYBEN_ACCOUNTS_PATH` (tests + ops). Otherwise falls back to
/// `<app_data_dir>/azure-accounts.json`, where `app_data_dir` matches the same
/// `dirs::data_dir()` / `dirs::config_dir()` resolution used by
/// `token_file_cache::cache_path()` — so both files live side by side under
/// `~/Library/Application Support/QueryBen/` on macOS, `%APPDATA%\QueryBen\`
/// on Windows, and `~/.config/QueryBen/` on Linux.
pub fn accounts_path() -> Option<PathBuf> {
    if let Ok(overridden) = std::env::var(ENV_ACCOUNTS_PATH_OVERRIDE) {
        if !overridden.is_empty() {
            return Some(PathBuf::from(overridden));
        }
    }

    #[cfg(target_os = "linux")]
    let root = dirs::config_dir()?;
    #[cfg(not(target_os = "linux"))]
    let root = dirs::data_dir()?;

    Some(root.join("QueryBen").join(FILE_NAME))
}

/// Best-effort read. Missing file / bad JSON → empty list.
pub fn load() -> Vec<AccountRegistryEntry> {
    let Some(path) = accounts_path() else {
        return Vec::new();
    };
    let Ok(bytes) = fs::read(&path) else {
        return Vec::new();
    };
    serde_json::from_slice(&bytes).unwrap_or_default()
}

/// Atomic write. Temp file + rename so a crash mid-write leaves the previous
/// good file intact.
pub fn save(entries: &[AccountRegistryEntry]) -> Result<(), AppError> {
    let path = accounts_path().ok_or_else(|| {
        AppError::internal("no app-data directory for azure-accounts.json (HOME / APPDATA unset?)")
    })?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            AppError::internal(format!(
                "create parent {}: {e}",
                parent.display()
            ))
        })?;
    }

    let json = serde_json::to_vec_pretty(entries)
        .map_err(|e| AppError::internal(format!("serialize accounts: {e}")))?;

    let parent = path.parent().ok_or_else(|| {
        AppError::internal(format!("accounts path has no parent: {}", path.display()))
    })?;
    let tmp = parent.join(format!(".{FILE_NAME}.tmp.{}", std::process::id()));

    write_atomic(&tmp, &path, &json)
        .map_err(|e| AppError::internal(format!("write accounts: {e}")))?;

    Ok(())
}

#[cfg(unix)]
fn write_atomic(tmp: &Path, dest: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt as _;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(tmp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(tmp, dest)?;
    let perm = std::os::unix::fs::PermissionsExt::from_mode(0o600);
    fs::set_permissions(dest, perm)?;
    Ok(())
}

#[cfg(not(unix))]
fn write_atomic(tmp: &Path, dest: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(tmp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    if dest.exists() {
        let _ = fs::remove_file(dest);
    }
    fs::rename(tmp, dest)?;
    Ok(())
}

/// Insert-or-update by `account_id`. Refreshes `last_signed_in` on hit.
pub fn upsert(entry: AccountRegistryEntry) -> Result<Vec<AccountRegistryEntry>, AppError> {
    let mut list = load();
    if let Some(existing) = list.iter_mut().find(|e| e.account_id == entry.account_id) {
        existing.username = entry.username;
        existing.tenant_id = entry.tenant_id;
        existing.display_name = entry.display_name;
        existing.last_signed_in = entry.last_signed_in;
    } else {
        list.push(entry);
    }
    save(&list)?;
    Ok(list)
}

/// Remove by `account_id`. No-op if absent.
pub fn remove(account_id: &str) -> Result<Vec<AccountRegistryEntry>, AppError> {
    let mut list = load();
    list.retain(|e| e.account_id != account_id);
    save(&list)?;
    Ok(list)
}

/// Convenience: find one by id.
pub fn find(account_id: &str) -> Option<AccountRegistryEntry> {
    load().into_iter().find(|e| e.account_id == account_id)
}

/// When exactly one account is signed in, return it. Used by the connection
/// migration path (single-swap legacy → per-account) and by silent reauth as
/// the "the field wasn't set, but there's only one plausible account" fallback.
pub fn only_account() -> Option<AccountRegistryEntry> {
    let list = load();
    if list.len() == 1 {
        list.into_iter().next()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use tempfile::TempDir;

    struct Guard {
        _tmp: TempDir,
    }

    impl Guard {
        fn new() -> Self {
            let tmp = tempfile::tempdir().expect("mk tempdir");
            let path = tmp.path().join(FILE_NAME);
            std::env::set_var(ENV_ACCOUNTS_PATH_OVERRIDE, &path);
            Self { _tmp: tmp }
        }
    }

    impl Drop for Guard {
        fn drop(&mut self) {
            std::env::remove_var(ENV_ACCOUNTS_PATH_OVERRIDE);
        }
    }

    fn mk(id: &str, user: &str) -> AccountRegistryEntry {
        AccountRegistryEntry {
            account_id: id.to_string(),
            username: user.to_string(),
            tenant_id: "tid-1".into(),
            display_name: None,
            last_signed_in: Utc::now(),
        }
    }

    #[test]
    #[serial]
    fn load_missing_file_returns_empty() {
        let _g = Guard::new();
        assert!(load().is_empty());
    }

    #[test]
    #[serial]
    fn upsert_inserts_new_entry() {
        let _g = Guard::new();
        let list = upsert(mk("a1.t1", "alice@x")).expect("upsert");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].account_id, "a1.t1");
    }

    #[test]
    #[serial]
    fn upsert_updates_existing_entry() {
        let _g = Guard::new();
        upsert(mk("a1.t1", "old@x")).expect("first");
        let list = upsert(mk("a1.t1", "new@x")).expect("second");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].username, "new@x");
    }

    #[test]
    #[serial]
    fn multiple_accounts_persist_and_roundtrip() {
        let _g = Guard::new();
        upsert(mk("a1.t1", "alice@x")).expect("1");
        upsert(mk("b2.t1", "bob@x")).expect("2");
        upsert(mk("c3.t2", "carol@y")).expect("3");
        let list = load();
        assert_eq!(list.len(), 3);
        assert!(list.iter().any(|e| e.username == "alice@x"));
        assert!(list.iter().any(|e| e.username == "bob@x"));
        assert!(list.iter().any(|e| e.username == "carol@y"));
    }

    #[test]
    #[serial]
    fn remove_deletes_only_named_entry() {
        let _g = Guard::new();
        upsert(mk("a1.t1", "alice@x")).expect("1");
        upsert(mk("b2.t1", "bob@x")).expect("2");
        let list = remove("a1.t1").expect("remove");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].account_id, "b2.t1");
    }

    #[test]
    #[serial]
    fn remove_missing_id_is_noop() {
        let _g = Guard::new();
        upsert(mk("a1.t1", "alice@x")).expect("1");
        let list = remove("does-not-exist").expect("remove");
        assert_eq!(list.len(), 1);
    }

    #[test]
    #[serial]
    fn only_account_returns_lone_entry() {
        let _g = Guard::new();
        assert!(only_account().is_none());
        upsert(mk("a1.t1", "alice@x")).expect("1");
        assert_eq!(only_account().map(|e| e.account_id), Some("a1.t1".into()));
        upsert(mk("b2.t1", "bob@x")).expect("2");
        assert!(only_account().is_none());
    }

    #[test]
    #[serial]
    fn find_returns_matching_entry() {
        let _g = Guard::new();
        upsert(mk("a1.t1", "alice@x")).expect("1");
        upsert(mk("b2.t1", "bob@x")).expect("2");
        assert_eq!(find("b2.t1").map(|e| e.username), Some("bob@x".into()));
        assert!(find("missing").is_none());
    }

    #[cfg(unix)]
    #[test]
    #[serial]
    fn saved_file_is_0600() {
        use std::os::unix::fs::PermissionsExt as _;
        let _g = Guard::new();
        upsert(mk("a1.t1", "alice@x")).expect("upsert");
        let path = accounts_path().expect("path");
        let mode = fs::metadata(&path).expect("stat").permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "accounts file must be 0600; got {:o}", mode);
    }
}
