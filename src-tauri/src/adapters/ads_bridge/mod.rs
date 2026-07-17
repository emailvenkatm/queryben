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

mod decrypt;
mod detection;
mod import;
mod msal_cache;

/// Test-sandbox root. When set, both `ads_user_dir()` and `cache_dir()`
/// resolve underneath it (`<root>/User` and `<root>/Azure Accounts`) and
/// `prime_token_cache_for_accounts` becomes a no-op. One knob for tests to
/// prove the whole pipeline stays off the real filesystem and keychain.
pub const ENV_ADS_ROOT_OVERRIDE: &str = "QUERYBEN_ADS_ROOT";

pub use detection::{detect_ads_installation, AdsDetectionSummary, ENV_ADS_USER_DIR_OVERRIDE};
pub use import::{import_from_ads, AdsImportSummary, ENV_QB_SNIPPETS_PATH_OVERRIDE};
pub use msal_cache::{
    try_borrow_ads_token, try_borrow_from_file, BorrowedToken, ENV_CACHE_DIR_OVERRIDE,
};
