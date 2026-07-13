//! Integration tests for `SqliteQueriesRepo`.
//!
//! Exercises the public `QueriesRepo` trait through the sqlite implementation
//! against a per-test `TempDir` DB. Complements the unit tests inside the
//! `infra::queries_db` module by driving the same paths from outside the
//! crate, which is what the frontend does at runtime.

use chrono::{Duration, Utc};
use tempfile::TempDir;
use uuid::Uuid;

use queryben_lib::core::saved_query::{
    HistoryEntry, HistoryFilter, SavedQueryFilter,
};
use queryben_lib::adapters::queries_db::{QueriesRepo, SqliteQueriesRepo};

fn open_repo(dir: &TempDir) -> SqliteQueriesRepo {
    SqliteQueriesRepo::open(dir.path(), 500, "General".into(), 90)
        .expect("open repo")
}

#[tokio::test]
async fn saved_query_roundtrip_via_trait() {
    let tmp = TempDir::new().expect("tempdir");
    let repo: Box<dyn QueriesRepo> = Box::new(open_repo(&tmp));
    let cid = Uuid::new_v4();

    let saved = repo
        .save_query("Weekly report", Some("Ops"), "SELECT * FROM logs", Some(cid))
        .await
        .expect("save");
    assert_eq!(saved.folder, "Ops");
    assert_eq!(saved.connection_id, Some(cid));

    // Filter by folder
    let by_folder = repo
        .list_saved(SavedQueryFilter {
            folder: Some("Ops".into()),
            ..Default::default()
        })
        .await
        .expect("list by folder");
    assert_eq!(by_folder.len(), 1);

    let by_wrong_folder = repo
        .list_saved(SavedQueryFilter {
            folder: Some("Nope".into()),
            ..Default::default()
        })
        .await
        .expect("list wrong folder");
    assert!(by_wrong_folder.is_empty());

    repo.delete_saved(saved.id).await.expect("delete");
    let after = repo
        .list_saved(SavedQueryFilter::default())
        .await
        .expect("list");
    assert!(after.is_empty());
}

#[tokio::test]
async fn history_insertion_and_retention_prune() {
    let tmp = TempDir::new().expect("tempdir");
    let repo: Box<dyn QueriesRepo> = Box::new(open_repo(&tmp));

    // Three rows: one old (past retention), two recent.
    let old = HistoryEntry {
        id: Uuid::new_v4(),
        sql: "SELECT 'old'".into(),
        connection_id: None,
        executed_at: Utc::now() - Duration::days(200),
        row_count: None,
        duration_ms: None,
        error: None,
    };
    let recent_a = HistoryEntry {
        id: Uuid::new_v4(),
        sql: "SELECT 'a'".into(),
        connection_id: None,
        executed_at: Utc::now() - Duration::minutes(5),
        row_count: Some(1),
        duration_ms: Some(12),
        error: None,
    };
    let recent_b = HistoryEntry {
        id: Uuid::new_v4(),
        sql: "SELECT 'b'".into(),
        connection_id: None,
        executed_at: Utc::now(),
        row_count: Some(3),
        duration_ms: Some(8),
        error: None,
    };
    repo.log_history(old).await.expect("log old");
    repo.log_history(recent_a).await.expect("log a");
    repo.log_history(recent_b).await.expect("log b");

    // Prune anything older than 90 days.
    let dropped = repo.clear_history(Some(90)).await.expect("prune");
    assert_eq!(dropped, 1);

    let rows = repo
        .list_history(HistoryFilter::default())
        .await
        .expect("list");
    assert_eq!(rows.len(), 2);
    // Newest first.
    assert_eq!(rows[0].sql, "SELECT 'b'");
}

#[tokio::test]
async fn history_connection_and_text_filters() {
    let tmp = TempDir::new().expect("tempdir");
    let repo: Box<dyn QueriesRepo> = Box::new(open_repo(&tmp));
    let cid_a = Uuid::new_v4();
    let cid_b = Uuid::new_v4();

    for (sql, cid, mins) in [
        ("SELECT id FROM users", Some(cid_a), 5),
        ("SELECT name FROM orders", Some(cid_b), 4),
        ("UPDATE users SET x=1", Some(cid_a), 3),
        ("DELETE FROM stale_rows", None, 2),
    ] {
        repo.log_history(HistoryEntry {
            id: Uuid::new_v4(),
            sql: sql.into(),
            connection_id: cid,
            executed_at: Utc::now() - Duration::minutes(mins),
            row_count: Some(1),
            duration_ms: Some(1),
            error: None,
        })
        .await
        .expect("log");
    }

    // Connection filter.
    let for_a = repo
        .list_history(HistoryFilter {
            connection_id: Some(cid_a),
            ..Default::default()
        })
        .await
        .expect("list a");
    assert_eq!(for_a.len(), 2);

    // Search filter (case-insensitive substring).
    let has_users = repo
        .list_history(HistoryFilter {
            search: Some("USERS".into()),
            ..Default::default()
        })
        .await
        .expect("list users");
    assert_eq!(has_users.len(), 2);

    // Combined.
    let a_and_users = repo
        .list_history(HistoryFilter {
            search: Some("users".into()),
            connection_id: Some(cid_a),
            ..Default::default()
        })
        .await
        .expect("list combined");
    assert_eq!(a_and_users.len(), 2);
}

#[tokio::test]
async fn schema_init_is_idempotent_across_opens() {
    let tmp = TempDir::new().expect("tempdir");

    // Open, insert, close.
    {
        let r = open_repo(&tmp);
        r.save_query("first", None, "SELECT 1", None)
            .await
            .expect("save");
    }
    // Re-open the same DB. Schema init must be a no-op — no dropped rows, no
    // error on the CREATE TABLE IF NOT EXISTS.
    {
        let r = open_repo(&tmp);
        let rows = r
            .list_saved(SavedQueryFilter::default())
            .await
            .expect("list");
        assert_eq!(rows.len(), 1);
    }
    // Third open for paranoia.
    {
        let _r = open_repo(&tmp);
    }
}
