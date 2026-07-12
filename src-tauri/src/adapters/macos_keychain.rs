//! macOS keychain wrapper — uses the DEFAULT access group (per-team-ID).
//!
//! # Why this file exists
//!
//! Keychain items written by the `keyring` crate on macOS land in the process's
//! *default* access group. That default group is derived from the writing
//! process's code-signing identity:
//!
//!   * Code-signed with a stable **Developer ID team**: default group is
//!     per-team-ID (e.g. `H4RK9DC4UP`). Items persist across `cargo build`s,
//!     across notarized releases, and across app updates. Zero "Always Allow"
//!     prompts as long as every build is signed with the same team's cert.
//!   * **Ad-hoc signed** (linker default, no team): the default group is
//!     per-binary-hash. Every rebuild changes the hash → new group → macOS
//!     re-prompts "QueryBen wants to use your confidential information...".
//!
//! Historical attempt: this module used to pin `kSecAttrAccessGroup =
//! "H4RK9DC4UP.com.queryben"` and rely on a matching `keychain-access-groups`
//! entitlement embedded via `codesign --entitlements`. That approach fails for
//! dev binaries because the entitlement requires a provisioning profile
//! (impossible for `cargo run` binaries) and hardened runtime requires
//! notarization (obviously not available for dev builds). Gatekeeper's kernel
//! launch check SIGKILLs any binary that has hardened runtime + un-notarized
//! sig. The dev-sign.sh script now signs with plain Developer ID (no
//! entitlements, no runtime flag), which pins the DEFAULT group to the team
//! ID — same stability, none of the Gatekeeper problems.
//!
//! Concretely: this file no longer sets `kSecAttrAccessGroup` on any query.
//! macOS routes reads/writes to the default group, which for a properly signed
//! dev binary IS the team-ID-backed group we want.
//!
//! # Unsigned-binary fallback
//!
//! When `scripts/dev-sign.sh` can't find a Developer ID cert on this Mac (fork
//! contributors without an Apple Developer account), it ad-hoc-signs the
//! binary. Items still write to the default group, but that group is now
//! per-binary-hash → macOS re-prompts every rebuild. We emit one WARN on first
//! keychain operation so the developer knows why.
//!
//! # API
//!
//! This module intentionally mirrors the subset of `keyring::Entry` we use so
//! the domain layer stays platform-agnostic. Non-macOS platforms shouldn't
//! import this file at all — they use `keyring` directly. See
//! `super::keychain` for the cross-platform dispatch.

use std::ffi::c_void;
use std::ptr;
use std::sync::Once;

use core_foundation::base::{CFRelease, CFType, CFTypeRef, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::data::CFData;
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_foundation_sys::base::OSStatus;
use core_foundation_sys::dictionary::CFDictionaryRef;
use core_foundation_sys::string::CFStringRef;
use security_framework_sys::base::errSecSuccess;
use security_framework_sys::code_signing::{SecCodeCopySelf, SecCodeRef};
use security_framework_sys::item::{
    kSecAttrAccount, kSecAttrService, kSecClass, kSecClassGenericPassword,
    kSecMatchLimit, kSecReturnData, kSecValueData,
};
use security_framework_sys::keychain_item::{
    SecItemAdd, SecItemCopyMatching, SecItemDelete, SecItemUpdate,
};

// SecCodeCopySigningInformation lives in the Security framework but isn't
// re-exported by security-framework-sys 2.17. We declare the minimum FFI here
// so we can inspect our own process's kernel-tracked code-signing identity —
// which is the ONLY signal that actually determines the default keychain
// access group. Reading `codesign -dvv` on the on-disk file gives a false
// answer because `cargo run`'s watcher can overwrite target/debug/queryben
// with a fresh linker-signed rebuild AFTER the running process has already
// exec'd from a signed inode.
#[link(name = "Security", kind = "framework")]
extern "C" {
    fn SecCodeCopySigningInformation(
        code: SecCodeRef,
        flags: u32,
        information: *mut CFDictionaryRef,
    ) -> OSStatus;

    // Bit 1 << 1 in the SecCS flag space; documented as kSecCSSigningInformation
    // in <Security/SecCode.h>. Requests the signing dict (Authority, teamid,
    // identifier, flags, ...) without validating on-disk resource hashes.
    static kSecCodeInfoTeamIdentifier: CFStringRef;
}
const K_SEC_CS_SIGNING_INFORMATION: u32 = 1 << 1;

use crate::error::AppError;

// SecItem error we treat as "no such entry" — mirrors keyring::Error::NoEntry.
// -25300 = errSecItemNotFound; this constant is not re-exported by the -sys
// crate as of 3.x, so we hard-code it.
const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;
// -25299 = errSecDuplicateItem; SecItemAdd returns this when the (service,
// account) tuple already has an entry. We map it to an Update.
const ERR_SEC_DUPLICATE_ITEM: i32 = -25299;

static UNSIGNED_WARNING: Once = Once::new();

/// Emitted once per process if the current binary is ad-hoc signed (no team).
/// With no team, macOS uses a per-binary-hash default access group, so keychain
/// items don't survive a rebuild and the user gets re-prompted every time.
fn warn_unsigned_once() {
    UNSIGNED_WARNING.call_once(|| {
        if process_teamid().is_none() {
            tracing::warn!(
                target: "queryben::keychain",
                "This binary is ad-hoc signed (no Developer ID team). The default macOS \
                 keychain access group will be per-binary-hash, so items will NOT survive \
                 the next rebuild and macOS will re-prompt for keychain access every launch. \
                 Run scripts/dev-sign.sh with a Developer ID cert to fix."
            );
        }
    });
}

/// Returns the Developer ID team identifier the kernel captured for this
/// process at exec time, or `None` if the process is ad-hoc signed (or the
/// Security Framework call fails, which we treat as "unknown → don't warn").
///
/// Why not `codesign -dvv <path>`: the on-disk path can be overwritten by a
/// subsequent `cargo build` while this process is still running — the file
/// changes but the kernel's captured identity for our PID does not. Shelling
/// out to codesign on the path therefore false-positives ad-hoc after any
/// concurrent rebuild.
fn process_teamid() -> Option<String> {
    unsafe {
        let mut code: SecCodeRef = ptr::null_mut();
        // Flags = 0 → default behavior (no strict validate, no network).
        let status = SecCodeCopySelf(0, &mut code);
        if status != errSecSuccess || code.is_null() {
            return None;
        }
        let _code_guard = ReleaseOnDrop(code as *const c_void);

        let mut info: CFDictionaryRef = ptr::null();
        let status = SecCodeCopySigningInformation(
            code,
            K_SEC_CS_SIGNING_INFORMATION,
            &mut info,
        );
        if status != errSecSuccess || info.is_null() {
            return None;
        }
        let dict: CFDictionary = CFDictionary::wrap_under_create_rule(info);

        // teamid key is missing entirely for ad-hoc / linker-signed binaries;
        // present and non-empty for Developer ID / App Store signed ones.
        let key = kSecCodeInfoTeamIdentifier as *const c_void;
        let value = dict.find(key)?;
        // The value under kSecCodeInfoTeamIdentifier is a CFString.
        let team: CFString = CFString::wrap_under_get_rule(*value as CFStringRef);
        let team = team.to_string();
        if team.is_empty() { None } else { Some(team) }
    }
}

/// Tiny RAII guard so we don't leak the SecCodeRef on early returns.
struct ReleaseOnDrop(*const c_void);
impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CFRelease(self.0) };
        }
    }
}

// ---- public API -------------------------------------------------------------

pub fn set_password(service: &str, account: &str, password: &str) -> Result<(), AppError> {
    warn_unsigned_once();
    try_set(service, account, password.as_bytes())
        .map_err(|s| AppError::internal(format!("macOS keychain write (OSStatus {s})")))
}

pub fn get_password(service: &str, account: &str) -> Result<Option<String>, AppError> {
    warn_unsigned_once();
    try_get(service, account)
        .map_err(|s| AppError::internal(format!("macOS keychain read (OSStatus {s})")))
}

pub fn delete_password(service: &str, account: &str) -> Result<(), AppError> {
    warn_unsigned_once();
    match try_delete(service, account) {
        Ok(()) => Ok(()),
        Err(ERR_SEC_ITEM_NOT_FOUND) => Ok(()),
        Err(status) => Err(AppError::internal(format!(
            "macOS keychain delete (OSStatus {status})"
        ))),
    }
}

// ---- internal helpers -------------------------------------------------------

/// Build the (service, account) query. No `kSecAttrAccessGroup` — macOS uses
/// the process's default group (per-team for signed binaries, per-hash for
/// ad-hoc), which is what we want. See module docs.
fn base_query(service: &str, account: &str) -> Vec<(CFString, CFType)> {
    unsafe {
        vec![
            (
                CFString::wrap_under_get_rule(kSecClass),
                CFString::wrap_under_get_rule(kSecClassGenericPassword).as_CFType(),
            ),
            (
                CFString::wrap_under_get_rule(kSecAttrService),
                CFString::new(service).as_CFType(),
            ),
            (
                CFString::wrap_under_get_rule(kSecAttrAccount),
                CFString::new(account).as_CFType(),
            ),
        ]
    }
}

/// Add-or-update. Returns `Ok(())` on success, or the raw OSStatus that
/// finally killed the operation.
fn try_set(service: &str, account: &str, value: &[u8]) -> Result<(), i32> {
    let status = sec_item_add(service, account, value);
    if status == errSecSuccess {
        return Ok(());
    }
    if status == ERR_SEC_DUPLICATE_ITEM {
        let update_status = sec_item_update(service, account, value);
        if update_status == errSecSuccess {
            return Ok(());
        }
        return Err(update_status);
    }
    Err(status)
}

/// Copy-password. Returns `Ok(None)` on not-found and `Ok(Some(_))` on success.
fn try_get(service: &str, account: &str) -> Result<Option<String>, i32> {
    let (status, data_ptr) = sec_item_copy_password(service, account);
    if status == ERR_SEC_ITEM_NOT_FOUND {
        return Ok(None);
    }
    if status != errSecSuccess {
        return Err(status);
    }
    if data_ptr.is_null() {
        // Kernel returned success but no data — treat as no entry rather than
        // panicking. Shouldn't happen in practice.
        return Ok(None);
    }
    // Wrap in CFData so it's released when the guard drops.
    let data: CFData = unsafe { CFData::wrap_under_create_rule(data_ptr as _) };
    let bytes: Vec<u8> = data.bytes().to_vec();
    // UTF-8 failure is a hard error, returned via a sentinel OSStatus.
    match String::from_utf8(bytes) {
        Ok(s) => Ok(Some(s)),
        Err(_) => Err(-1),
    }
}

fn try_delete(service: &str, account: &str) -> Result<(), i32> {
    let status = sec_item_delete(service, account);
    if status == errSecSuccess {
        return Ok(());
    }
    Err(status)
}

fn sec_item_add(service: &str, account: &str, value: &[u8]) -> i32 {
    let data = CFData::from_buffer(value);
    let mut pairs = base_query(service, account);
    unsafe {
        pairs.push((
            CFString::wrap_under_get_rule(kSecValueData),
            data.as_CFType(),
        ));
    }
    let dict = CFDictionary::from_CFType_pairs(&pairs);
    let mut result: CFTypeRef = ptr::null();
    let status = unsafe {
        SecItemAdd(
            dict.as_concrete_TypeRef(),
            &mut result as *mut _ as *mut _,
        )
    };
    // SecItemAdd may return an object we don't need; release if non-null.
    if !result.is_null() {
        unsafe { CFRelease(result as *const c_void) };
    }
    status
}

fn sec_item_update(service: &str, account: &str, value: &[u8]) -> i32 {
    let query = CFDictionary::from_CFType_pairs(&base_query(service, account));
    let data = CFData::from_buffer(value);
    let attrs = unsafe {
        CFDictionary::from_CFType_pairs(&[(
            CFString::wrap_under_get_rule(kSecValueData),
            data.as_CFType(),
        )])
    };
    unsafe {
        SecItemUpdate(
            query.as_concrete_TypeRef(),
            attrs.as_concrete_TypeRef(),
        )
    }
}

fn sec_item_copy_password(service: &str, account: &str) -> (i32, CFTypeRef) {
    let mut pairs = base_query(service, account);
    unsafe {
        pairs.push((
            CFString::wrap_under_get_rule(kSecReturnData),
            CFBoolean::true_value().as_CFType(),
        ));
        // Apple's SecItem API accepts either the kSecMatchLimitOne CFString
        // or a CFNumber(1). The -sys crate exports only kSecMatchLimitAll, so
        // we use the CFNumber form — documented equivalent per
        // Security/SecItem.h.
        pairs.push((
            CFString::wrap_under_get_rule(kSecMatchLimit),
            CFNumber::from(1i64).as_CFType(),
        ));
    }
    let dict = CFDictionary::from_CFType_pairs(&pairs);
    let mut out: CFTypeRef = ptr::null();
    let status = unsafe {
        SecItemCopyMatching(
            dict.as_concrete_TypeRef(),
            &mut out as *mut _ as *mut _,
        )
    };
    (status, out)
}

fn sec_item_delete(service: &str, account: &str) -> i32 {
    let dict = CFDictionary::from_CFType_pairs(&base_query(service, account));
    unsafe { SecItemDelete(dict.as_concrete_TypeRef()) }
}
