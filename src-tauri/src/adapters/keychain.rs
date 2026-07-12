//! Cross-platform keychain wrapper with a transparent macOS migration path.
//!
//! # Contract
//!
//! ```
//! keychain::set_password(service, account, password) -> Result<(), AppError>
//! keychain::get_password(service, account)           -> Result<Option<String>, AppError>
//! keychain::delete_password(service, account)        -> Result<(), AppError>
//! ```
//!
//! Missing entries are returned as `Ok(None)` from `get_password`, and are a
//! no-op for `delete_password`. Only true I/O failures bubble as `AppError`.
//!
//! # Platform behavior
//!
//! * **macOS** — uses `security-framework` (SecItem*) against the process's
//!   *default* keychain access group. When the binary is signed with a stable
//!   Developer ID team (via `scripts/dev-sign.sh`), that default group is
//!   per-team-ID and survives rebuilds + notarized releases + updates without
//!   re-prompting. Ad-hoc-signed builds (fork contributors without an Apple
//!   Developer cert) fall back to a per-binary-hash default group and WILL
//!   re-prompt on each rebuild — see `macos_keychain` module docs.
//! * **Windows** — plain `keyring` crate (Credential Manager). Windows has no
//!   equivalent to access groups; Credential Manager scopes by SID + service
//!   name and doesn't re-prompt.
//! * **Linux** — plain `keyring` crate (Secret Service / libsecret). Same story
//!   as Windows for our purposes.
//!
//! # Legacy migration
//!
//! Earlier versions attempted to pin `kSecAttrAccessGroup =
//! "H4RK9DC4UP.com.queryben"` via a `keychain-access-groups` entitlement. That
//! approach broke Gatekeeper's launch check and was reverted. Both the old and
//! new code paths ultimately write to the same default keychain group (the
//! `keyring` crate never set an access group either), so no data migration is
//! needed — `get_password` reads whatever the default group has, which is the
//! same place the old code wrote to.

use crate::error::AppError;

#[cfg(target_os = "macos")]
use super::macos_keychain;

// ---- Windows + Linux: pass-through to `keyring` -----------------------------

#[cfg(not(target_os = "macos"))]
pub fn set_password(service: &str, account: &str, password: &str) -> Result<(), AppError> {
    let entry = keyring::Entry::new(service, account)
        .map_err(|e| AppError::internal(format!("keychain open: {e}")))?;
    entry
        .set_password(password)
        .map_err(|e| AppError::internal(format!("keychain write: {e}")))
}

#[cfg(not(target_os = "macos"))]
pub fn get_password(service: &str, account: &str) -> Result<Option<String>, AppError> {
    let entry = keyring::Entry::new(service, account)
        .map_err(|e| AppError::internal(format!("keychain open: {e}")))?;
    match entry.get_password() {
        Ok(v) => Ok(Some(v)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(AppError::internal(format!("keychain read: {e}"))),
    }
}

#[cfg(not(target_os = "macos"))]
pub fn delete_password(service: &str, account: &str) -> Result<(), AppError> {
    let entry = keyring::Entry::new(service, account)
        .map_err(|e| AppError::internal(format!("keychain open: {e}")))?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(AppError::internal(format!("keychain delete: {e}"))),
    }
}

// ---- macOS: access-group aware + legacy migration ---------------------------

#[cfg(target_os = "macos")]
pub fn set_password(service: &str, account: &str, password: &str) -> Result<(), AppError> {
    macos_keychain::set_password(service, account, password)
}

#[cfg(target_os = "macos")]
pub fn get_password(service: &str, account: &str) -> Result<Option<String>, AppError> {
    macos_keychain::get_password(service, account)
}

#[cfg(target_os = "macos")]
pub fn delete_password(service: &str, account: &str) -> Result<(), AppError> {
    macos_keychain::delete_password(service, account)
}
