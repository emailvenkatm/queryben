//! Integration tests for the multi-account registry + connection migration.
//!
//! These exercise the pure JSON side of `azure_accounts` (no keychain, no
//! network) and the connection-registry backfill that lets legacy single-
//! account installs upgrade transparently. Points the on-disk registry at a
//! `tempfile::TempDir` via `QUERYBEN_ACCOUNTS_PATH` so we never clobber the
//! real user file.

use serial_test::serial;
use tempfile::TempDir;

use queryben_lib::core::connection::{
    AuthMode, Connection, ConnectionEntry, ConnectionRegistry,
};
use queryben_lib::adapters::azure_accounts::{
    self, AccountRegistryEntry, ENV_ACCOUNTS_PATH_OVERRIDE,
};

use chrono::Utc;
use uuid::Uuid;

struct Guard {
    _tmp: TempDir,
}

impl Guard {
    fn new() -> Self {
        let tmp = tempfile::tempdir().expect("mk tempdir");
        let path = tmp.path().join("azure-accounts.json");
        std::env::set_var(ENV_ACCOUNTS_PATH_OVERRIDE, &path);
        Self { _tmp: tmp }
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        std::env::remove_var(ENV_ACCOUNTS_PATH_OVERRIDE);
    }
}

fn mk_registry_entry(account_id: &str, username: &str) -> AccountRegistryEntry {
    AccountRegistryEntry {
        account_id: account_id.into(),
        username: username.into(),
        tenant_id: "tid-1".into(),
        display_name: Some(username.into()),
        last_signed_in: Utc::now(),
    }
}

fn mk_legacy_aad_connection(id: Uuid) -> ConnectionEntry {
    ConnectionEntry {
        connection: Connection {
            id,
            name: "legacy-aad".into(),
            server: "srv.database.windows.net".into(),
            database: "db".into(),
            port: None,
            username: None,
            auth_mode: AuthMode::AadToken,
            created_at: Utc::now(),
            last_used: None,
            // The whole point: legacy connections have this as None. Silent
            // reauth must still work for them via the fallback path.
            account_id: None,
            nickname: None,
            color: None,
        },
        password: None,
        trust_server_certificate: false,
        tenant_id: Some("tid".into()),
        client_id: Some("cid".into()),
        server_arm_id: None,
    }
}

// ---- test 1: N-account roundtrip -------------------------------------------

#[test]
#[serial]
fn three_accounts_roundtrip_through_registry_file() {
    let _g = Guard::new();

    azure_accounts::upsert(mk_registry_entry("a.t1", "alice@x")).expect("1");
    azure_accounts::upsert(mk_registry_entry("b.t1", "bob@x")).expect("2");
    azure_accounts::upsert(mk_registry_entry("c.t2", "carol@y")).expect("3");

    let list = azure_accounts::load();
    assert_eq!(list.len(), 3);
    assert!(list.iter().any(|e| e.username == "alice@x"));
    assert!(list.iter().any(|e| e.username == "bob@x"));
    assert!(list.iter().any(|e| e.username == "carol@y"));
}

// ---- test 2: removing one account leaves the rest intact -------------------

#[test]
#[serial]
fn removing_one_account_leaves_others_intact() {
    let _g = Guard::new();

    azure_accounts::upsert(mk_registry_entry("a.t1", "alice@x")).expect("1");
    azure_accounts::upsert(mk_registry_entry("b.t1", "bob@x")).expect("2");
    azure_accounts::upsert(mk_registry_entry("c.t2", "carol@y")).expect("3");

    let after = azure_accounts::remove("b.t1").expect("remove");
    assert_eq!(after.len(), 2);
    assert!(after.iter().any(|e| e.account_id == "a.t1"));
    assert!(after.iter().any(|e| e.account_id == "c.t2"));
    assert!(!after.iter().any(|e| e.account_id == "b.t1"));
}

// ---- test 3: connection migration backfills account_id ---------------------

#[test]
#[serial]
fn legacy_connection_gets_account_id_backfilled_from_only_account() {
    let _g = Guard::new();

    // Registry has exactly one signed-in account — the "legacy single-swap"
    // pre-migration shape.
    azure_accounts::upsert(mk_registry_entry("only.acct", "only@x")).expect("seed");

    // Fresh temp dir for the connection registry so its persisted file
    // doesn't clobber the user's real connections.json.
    let conn_tmp = TempDir::new().expect("conn tempdir");
    let registry = ConnectionRegistry::new(conn_tmp.path()).expect("conn registry");
    let id = Uuid::new_v4();
    registry
        .insert(mk_legacy_aad_connection(id))
        .expect("insert legacy");

    // Simulate the migration path from state::AppState::new: look up the
    // only account in the registry and backfill.
    let only = azure_accounts::only_account().expect("exactly one account present");
    let touched = registry
        .backfill_missing_account_id(&only.account_id)
        .expect("backfill");
    assert_eq!(touched, 1);

    let snap = registry.snapshot(id).expect("snap");
    assert_eq!(
        snap.connection.account_id.as_deref(),
        Some("only.acct"),
        "legacy connection should carry the promoted account_id"
    );
}

// ---- test 4: only_account returns None when multiple accounts present ------

#[test]
#[serial]
fn only_account_returns_none_when_multiple_present() {
    let _g = Guard::new();

    azure_accounts::upsert(mk_registry_entry("a.t1", "alice@x")).expect("1");
    azure_accounts::upsert(mk_registry_entry("b.t1", "bob@x")).expect("2");

    // With two accounts registered, the "single-account" migration heuristic
    // must decline — we don't know which one to backfill against.
    assert!(azure_accounts::only_account().is_none());
}

// ---- test 5: find() returns the right entry --------------------------------

#[test]
#[serial]
fn find_returns_specific_entry_by_account_id() {
    let _g = Guard::new();

    azure_accounts::upsert(mk_registry_entry("a.t1", "alice@x")).expect("1");
    azure_accounts::upsert(mk_registry_entry("b.t1", "bob@x")).expect("2");

    let entry = azure_accounts::find("a.t1").expect("found");
    assert_eq!(entry.username, "alice@x");
    assert!(azure_accounts::find("missing.id").is_none());
}
