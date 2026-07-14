//! Integration tests: prove the Azure refresh token survives a keychain wipe.
//!
//! These tests exercise the ADS-parity file-cache path in
//! `queryben_lib::adapters::azure::oauth::try_acquire_silent`. They:
//!
//!   * Point the on-disk cache at a `tempfile::TempDir` via
//!     `QUERYBEN_TOKEN_CACHE_PATH` so they never clobber the real user file.
//!   * Disable the CLI probe via `QUERYBEN_DISABLE_AZ_CLI=1` so the file cache
//!     is always the first successful path when we want to test it.
//!   * Pre-populate a non-expired access token so no network call is required.
//!   * Serialize on `#[serial]` because they mutate process-wide env vars.
//!
//! Run with: `cargo test --test token_cache_persists`

use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use serial_test::serial;
use tempfile::TempDir;

use queryben_lib::adapters::azure::oauth::{self, TokenCache};
use queryben_lib::adapters::keychain;
use queryben_lib::adapters::token_file_cache::{
    self, CachedAccessToken, PersistedAccount, TokenCache as FileTokenCache,
    ENV_CACHE_PATH_OVERRIDE,
};

const RESOURCE_SQLDB: &str = "https://database.windows.net/";
const SCOPE_SQLDB: &str = "https://database.windows.net/.default";

/// Point the file cache at a temp path for this test's lifetime and disable
/// the az-CLI probe so we get a deterministic file-cache execution.
struct Guard {
    _tmp: TempDir,
}

impl Guard {
    fn new() -> Self {
        let tmp = tempfile::tempdir().expect("mk tempdir");
        let path = tmp.path().join("azure-cache.json");
        std::env::set_var(ENV_CACHE_PATH_OVERRIDE, &path);
        std::env::set_var("QUERYBEN_DISABLE_AZ_CLI", "1");
        // Also disable the ADS + VS Code bridges so the file cache is the only
        // pre-keychain path the test can reach.
        std::env::set_var("QUERYBEN_DISABLE_BRIDGES", "1");
        Self { _tmp: tmp }
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        std::env::remove_var(ENV_CACHE_PATH_OVERRIDE);
        std::env::remove_var("QUERYBEN_DISABLE_AZ_CLI");
        std::env::remove_var("QUERYBEN_DISABLE_BRIDGES");
    }
}

fn cache_path() -> PathBuf {
    token_file_cache::cache_path().expect("cache path resolves under override")
}

fn write_cache_with_fresh_access_token(refresh_token: &str, access_token: &str) {
    let mut c = FileTokenCache::default();
    c.refresh_token = Some(refresh_token.to_string());
    c.account = Some(PersistedAccount {
        tenant_id: "00000000-0000-0000-0000-000000000001".into(),
        home_account_id: "user.oid.tenant.tid".into(),
        username: "test@example.com".into(),
    });
    c.cached_access_tokens.insert(
        RESOURCE_SQLDB.into(),
        CachedAccessToken {
            token: access_token.into(),
            // 30 minutes in the future — well past the 60s grace window.
            expires_at_unix: chrono::Utc::now().timestamp() + 1800,
        },
    );
    token_file_cache::save(&c).expect("save cache");
}

/// Wipe every macOS keychain item this app writes. Runs on all platforms — the
/// `security` binary is macOS-only, but on Linux / Windows the `keyring` crate
/// stores under a differently-named service that these tests don't touch.
fn wipe_keychain() {
    // Best-effort; missing tools / no matching items is fine.
    #[cfg(target_os = "macos")]
    {
        for _ in 0..8 {
            let status = std::process::Command::new("security")
                .args(["delete-generic-password", "-s", "com.queryben.azure"])
                .status();
            if !matches!(status, Ok(s) if s.success()) {
                break;
            }
        }
    }
}

// ---- test 1 -----------------------------------------------------------------

#[test]
#[serial]
fn refresh_token_survives_keychain_wipe() {
    let _g = Guard::new();

    // Arrange: file cache holds a still-valid access token + refresh token.
    write_cache_with_fresh_access_token("refresh-abc", "access-xyz");
    wipe_keychain();

    // Sanity: file exists after the keychain wipe.
    assert!(cache_path().exists(), "file cache should survive keychain wipe");

    // Act: run try_acquire_silent through the in-memory cache. The az CLI
    // probe is disabled, the keychain is empty, so the file-cache path is the
    // only one that can succeed — and it can, because we pre-populated a
    // non-expired access token.
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let mem_cache = TokenCache::new();
    let token = rt
        .block_on(oauth::try_acquire_silent(
            &mem_cache,
            "00000000-0000-0000-0000-000000000001",
            "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            SCOPE_SQLDB,
            None,
        ))
        .expect("silent acquire should hit file cache");

    // Assert: we got exactly the token we wrote — no network, no CLI, no
    // keychain would have produced this string.
    assert_eq!(token, "access-xyz");
}

// ---- test 2 -----------------------------------------------------------------

#[test]
#[serial]
fn file_cache_atomic_write_and_0600_perms() {
    let _g = Guard::new();

    // Write.
    let mut c = FileTokenCache::default();
    c.refresh_token = Some("r1".into());
    c.cached_access_tokens.insert(
        RESOURCE_SQLDB.into(),
        CachedAccessToken {
            token: "t1".into(),
            expires_at_unix: chrono::Utc::now().timestamp() + 3600,
        },
    );
    token_file_cache::save(&c).expect("save");

    // Re-read + compare the fields we care about.
    let round = token_file_cache::load().expect("load");
    assert_eq!(round.refresh_token.as_deref(), Some("r1"));
    let entry = round
        .cached_access_tokens
        .get(RESOURCE_SQLDB)
        .expect("access token entry");
    assert_eq!(entry.token, "t1");

    // Assert perms.
    #[cfg(unix)]
    {
        let meta = fs::metadata(cache_path()).expect("stat");
        let mode = meta.permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "cache file must be 0600 (owner rw only); got {:o}",
            mode
        );
    }
}

// ---- test 3 -----------------------------------------------------------------

#[test]
#[serial]
#[cfg(target_os = "macos")]
fn migration_from_keychain_copies_to_file_cache() {
    let _g = Guard::new();

    // Clean slate: wipe keychain, then seed a fake refresh token via the
    // app's own keychain wrapper (not the `security` CLI) so the item lands
    // in the exact access group azure_oauth will read from — a keychain item
    // written by an unrelated process lives in a different group and would be
    // invisible to SecItemCopyMatching here.
    wipe_keychain();
    let _ = keychain::delete_password("com.queryben.azure", "refresh_token");
    keychain::set_password("com.queryben.azure", "refresh_token", "kc-refresh-value")
        .expect("seed keychain refresh token");

    // Sanity: our own read sees the value we just wrote. If this fails the
    // test box has an unexpected keychain-group mismatch and we abort early
    // with a clear diagnostic instead of blaming the migration path.
    let readback = keychain::get_password("com.queryben.azure", "refresh_token")
        .expect("keychain read must succeed");
    assert_eq!(
        readback.as_deref(),
        Some("kc-refresh-value"),
        "seed keychain read-back mismatch (access group split?)"
    );

    // Pre-populate the file cache with a valid access token for our scope but
    // NO refresh token. This is what triggers the migration branch (empty
    // refresh_token) AND short-circuits before the refresh trade — so we can
    // assert the migration side effect without a live Azure network call.
    let mut seed = FileTokenCache::default();
    seed.cached_access_tokens.insert(
        RESOURCE_SQLDB.into(),
        CachedAccessToken {
            token: "access-token-for-migration-test".into(),
            expires_at_unix: chrono::Utc::now().timestamp() + 1800,
        },
    );
    token_file_cache::save(&seed).expect("seed file cache");
    let pre = token_file_cache::load().expect("seed load");
    assert!(pre.refresh_token.is_none(), "seed must have no refresh token");

    // Now call try_acquire_silent. Path exercised:
    //   in-memory miss → az CLI disabled → file cache loaded (no refresh
    //   token) → migration branch copies keychain refresh token into file
    //   cache and saves → file cache has a valid access token for scope →
    //   return it. No network, no rejection, no wipe.
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let mem_cache = TokenCache::new();
    let token = rt
        .block_on(oauth::try_acquire_silent(
            &mem_cache,
            "00000000-0000-0000-0000-000000000001",
            "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            SCOPE_SQLDB,
            None,
        ))
        .expect("silent acquire should return the seeded access token");
    assert_eq!(token, "access-token-for-migration-test");

    // Assert: the file cache now carries the refresh token that used to only
    // live in the keychain.
    let loaded = token_file_cache::load().expect("cache should load after migration");
    assert_eq!(
        loaded.refresh_token.as_deref(),
        Some("kc-refresh-value"),
        "keychain refresh token should have been mirrored into the file cache"
    );

    // Cleanup: wipe the keychain item we added so we don't pollute the user's
    // real keychain if this test box has no test isolation.
    let _ = keychain::delete_password("com.queryben.azure", "refresh_token");
    wipe_keychain();
}
