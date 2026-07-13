//! SQLite-backed repo for saved queries + query history.
//!
//! Both features share ONE database file (`queries.db` in the app data dir)
//! but live in separate tables. The trait boundary (`QueriesRepo`) exists so
//! we can swap in a future encrypted or remote-sync backend without touching
//! any of the `commands::queries_repo` command surface.
//!
//! ## Schema (see `SCHEMA` const)
//!   * `saved_queries` — user-authored, named + foldered SQL.
//!   * `query_history` — one row per executed query (opt-in via config).
//!
//! ## Migration policy
//! Forward-only. Every schema change lands as `CREATE TABLE IF NOT EXISTS` or
//! `ALTER TABLE ... ADD COLUMN` — we never DROP or rename in place, because
//! the repo persists across app upgrades and users would lose data.
//! `init_schema` is idempotent and safe to call at every startup.
//!
//! ## Threading
//! `rusqlite::Connection` is `!Send`, so we wrap it in `Mutex` behind a
//! `SqliteQueriesRepo`. The trait methods are `async` for future-proofing but
//! today they run the SQL synchronously inside the lock — SQLite is fast
//! enough that history writes stay sub-millisecond and never block the
//! frontend execute path (which is fire-and-forget on the JS side anyway).

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::core::saved_query::{HistoryEntry, HistoryFilter, SavedQuery, SavedQueryFilter};
use crate::error::AppError;

/// Idempotent DDL. Everything is `IF NOT EXISTS`; ALTER additions live in
/// `migrate` below for anything we add after v1.
const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS saved_queries (
    id            TEXT PRIMARY KEY NOT NULL,
    name          TEXT NOT NULL,
    folder        TEXT NOT NULL DEFAULT 'General',
    sql           TEXT NOT NULL,
    connection_id TEXT,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_saved_queries_folder ON saved_queries(folder);
CREATE INDEX IF NOT EXISTS idx_saved_queries_name   ON saved_queries(name);

CREATE TABLE IF NOT EXISTS query_history (
    id            TEXT PRIMARY KEY NOT NULL,
    sql           TEXT NOT NULL,
    connection_id TEXT,
    executed_at   TEXT NOT NULL,
    row_count     INTEGER,
    duration_ms   INTEGER,
    error         TEXT
);

-- Reverse-chronological scan is the dominant read pattern.
CREATE INDEX IF NOT EXISTS idx_history_executed_at   ON query_history(executed_at DESC);
CREATE INDEX IF NOT EXISTS idx_history_connection_id ON query_history(connection_id);
"#;

// ---- Trait -----------------------------------------------------------------

#[async_trait]
pub trait QueriesRepo: Send + Sync {
    async fn save_query(
        &self,
        name: &str,
        folder: Option<&str>,
        sql: &str,
        connection_id: Option<Uuid>,
    ) -> Result<SavedQuery, AppError>;

    async fn list_saved(&self, filter: SavedQueryFilter) -> Result<Vec<SavedQuery>, AppError>;

    async fn delete_saved(&self, id: Uuid) -> Result<(), AppError>;

    async fn rename_saved(&self, id: Uuid, name: &str) -> Result<SavedQuery, AppError>;

    async fn log_history(&self, entry: HistoryEntry) -> Result<(), AppError>;

    async fn list_history(&self, filter: HistoryFilter) -> Result<Vec<HistoryEntry>, AppError>;

    /// Delete rows older than `older_than_days` (uses config's retention when
    /// `None`). Returns the number of rows removed.
    async fn clear_history(&self, older_than_days: Option<u32>) -> Result<u64, AppError>;
}

// ---- Sqlite impl -----------------------------------------------------------

pub struct SqliteQueriesRepo {
    conn: Mutex<Connection>,
    /// Cap enforced by `log_history` after every insert. Reflects
    /// `QueriesConfig::history_max_rows` at construction time; not hot-
    /// reloaded (config isn't either — see `NotebookConfig::load`).
    max_rows: u32,
    /// Default folder for `save_query` when the caller didn't pass one.
    default_folder: String,
}

impl SqliteQueriesRepo {
    /// Build a repo against `<app_data_dir>/queries.db`. Creates the file if
    /// missing, runs the idempotent schema migration, and prunes any rows
    /// past `retention_days` so the DB doesn't grow forever between launches.
    pub fn open(
        app_data_dir: &Path,
        max_rows: u32,
        default_folder: String,
        retention_days: u32,
    ) -> Result<Self, AppError> {
        std::fs::create_dir_all(app_data_dir).map_err(|e| {
            AppError::internal(format!("mkdir {}: {e}", app_data_dir.display()))
        })?;
        let path = app_data_dir.join("queries.db");
        Self::open_at(&path, max_rows, default_folder, retention_days)
    }

    /// Test-friendly variant: point at an explicit path (e.g. a
    /// `TempDir::path().join("queries.db")`) instead of the app-data dir.
    pub fn open_at(
        path: &Path,
        max_rows: u32,
        default_folder: String,
        retention_days: u32,
    ) -> Result<Self, AppError> {
        let conn = Connection::open(path).map_err(|e| {
            AppError::internal(format!("open {}: {e}", path.display()))
        })?;
        // WAL is friendlier to concurrent readers; the history table gets
        // sampled from the UI at the same time as a background log_history
        // write can land.
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON;",
        )
        .map_err(|e| AppError::internal(format!("pragma: {e}")))?;

        init_schema(&conn)?;

        let repo = Self {
            conn: Mutex::new(conn),
            max_rows,
            default_folder,
        };

        // Startup vacuum: drop anything past the retention window so the DB
        // doesn't leak forever if the user tweaks the window down.
        if retention_days > 0 {
            if let Err(err) = repo.prune_older_than(retention_days) {
                tracing::warn!(
                    target: "queryben::queries_db::init",
                    %err,
                    "startup history prune failed"
                );
            }
        }

        Ok(repo)
    }

    fn with_conn<T>(
        &self,
        f: impl FnOnce(&mut Connection) -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        let mut guard = self
            .conn
            .lock()
            .map_err(|_| AppError::internal("queries_db mutex poisoned"))?;
        f(&mut guard)
    }

    fn prune_older_than(&self, days: u32) -> Result<u64, AppError> {
        let cutoff = Utc::now() - Duration::days(days as i64);
        let cutoff_str = cutoff.to_rfc3339();
        self.with_conn(|conn| {
            let n = conn
                .execute(
                    "DELETE FROM query_history WHERE executed_at < ?1",
                    params![cutoff_str],
                )
                .map_err(|e| AppError::internal(format!("prune: {e}")))?;
            Ok(n as u64)
        })
    }

    /// Cap history to `max_rows` after every insert. Cheap because it's a
    /// single DELETE + covering index; nothing to optimize further until row
    /// counts hit millions (config default caps them at 5k).
    fn enforce_max_rows(conn: &Connection, max_rows: u32) -> Result<(), AppError> {
        if max_rows == 0 {
            return Ok(());
        }
        // Keep the newest `max_rows`; drop everything older.
        conn.execute(
            "DELETE FROM query_history
             WHERE id IN (
                 SELECT id FROM query_history
                 ORDER BY executed_at DESC, rowid DESC
                 LIMIT -1 OFFSET ?1
             )",
            params![max_rows as i64],
        )
        .map_err(|e| AppError::internal(format!("enforce max rows: {e}")))?;
        Ok(())
    }
}

fn init_schema(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(SCHEMA)
        .map_err(|e| AppError::internal(format!("init schema: {e}")))?;
    migrate(conn)?;
    Ok(())
}

/// Forward-only additive migrations. Add new columns with
/// `ALTER TABLE ... ADD COLUMN` inside an `IF NOT EXISTS`-style guard so
/// running it against an already-migrated DB is a no-op.
fn migrate(conn: &Connection) -> Result<(), AppError> {
    // v1 columns are covered by the CREATE TABLE above; no ALTERs yet. When
    // we add a column later, use `column_exists(conn, "table", "col")?` and
    // gate the ALTER on that.
    let _ = conn;
    Ok(())
}

#[allow(dead_code)]
fn column_exists(conn: &Connection, table: &str, col: &str) -> Result<bool, AppError> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|e| AppError::internal(format!("prepare table_info: {e}")))?;
    let mut rows = stmt
        .query([])
        .map_err(|e| AppError::internal(format!("query table_info: {e}")))?;
    while let Some(row) = rows
        .next()
        .map_err(|e| AppError::internal(format!("next table_info: {e}")))?
    {
        let name: String = row
            .get(1)
            .map_err(|e| AppError::internal(format!("read col name: {e}")))?;
        if name == col {
            return Ok(true);
        }
    }
    Ok(false)
}

// ---- Row mapping helpers ---------------------------------------------------

fn parse_uuid(s: &str, field: &str) -> Result<Uuid, AppError> {
    Uuid::parse_str(s)
        .map_err(|e| AppError::internal(format!("bad uuid in {field}: {e}")))
}

fn parse_opt_uuid(s: Option<String>, field: &str) -> Result<Option<Uuid>, AppError> {
    match s {
        Some(v) if !v.is_empty() => Ok(Some(parse_uuid(&v, field)?)),
        _ => Ok(None),
    }
}

fn parse_dt(s: &str, field: &str) -> Result<DateTime<Utc>, AppError> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .map_err(|e| AppError::internal(format!("bad datetime in {field}: {e}")))
}

// ---- Trait impl ------------------------------------------------------------

#[async_trait]
impl QueriesRepo for SqliteQueriesRepo {
    async fn save_query(
        &self,
        name: &str,
        folder: Option<&str>,
        sql: &str,
        connection_id: Option<Uuid>,
    ) -> Result<SavedQuery, AppError> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let folder = folder.unwrap_or(&self.default_folder).to_string();
        let entry = SavedQuery {
            id,
            name: name.to_string(),
            folder: folder.clone(),
            sql: sql.to_string(),
            connection_id,
            created_at: now,
            updated_at: now,
        };
        let conn_id_str = connection_id.map(|u| u.to_string());
        let created_at_str = now.to_rfc3339();
        let updated_at_str = now.to_rfc3339();
        let id_str = id.to_string();
        let name = name.to_string();
        let sql = sql.to_string();
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO saved_queries (id, name, folder, sql, connection_id, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![id_str, name, folder, sql, conn_id_str, created_at_str, updated_at_str],
            )
            .map_err(|e| AppError::internal(format!("insert saved_queries: {e}")))?;
            Ok(())
        })?;
        Ok(entry)
    }

    async fn list_saved(&self, filter: SavedQueryFilter) -> Result<Vec<SavedQuery>, AppError> {
        self.with_conn(|conn| {
            let mut sql = String::from(
                "SELECT id, name, folder, sql, connection_id, created_at, updated_at \
                 FROM saved_queries WHERE 1=1",
            );
            let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
            if let Some(search) = filter.search.as_ref().filter(|s| !s.is_empty()) {
                sql.push_str(" AND lower(name) LIKE ?");
                args.push(Box::new(format!("%{}%", search.to_lowercase())));
            }
            if let Some(folder) = filter.folder.as_ref() {
                sql.push_str(" AND folder = ?");
                args.push(Box::new(folder.clone()));
            }
            if let Some(cid) = filter.connection_id {
                sql.push_str(" AND connection_id = ?");
                args.push(Box::new(cid.to_string()));
            }
            sql.push_str(" ORDER BY folder ASC, name ASC");

            let mut stmt = conn
                .prepare(&sql)
                .map_err(|e| AppError::internal(format!("prepare list_saved: {e}")))?;

            let arg_refs: Vec<&dyn rusqlite::ToSql> =
                args.iter().map(|b| b.as_ref()).collect();
            let mut rows = stmt
                .query(rusqlite::params_from_iter(arg_refs.iter()))
                .map_err(|e| AppError::internal(format!("query list_saved: {e}")))?;
            let mut out = Vec::new();
            while let Some(row) = rows
                .next()
                .map_err(|e| AppError::internal(format!("next saved: {e}")))?
            {
                let id_str: String = row.get(0).map_err(|e| AppError::internal(format!("id: {e}")))?;
                let name: String = row.get(1).map_err(|e| AppError::internal(format!("name: {e}")))?;
                let folder: String = row.get(2).map_err(|e| AppError::internal(format!("folder: {e}")))?;
                let sql_text: String = row.get(3).map_err(|e| AppError::internal(format!("sql: {e}")))?;
                let cid: Option<String> = row.get(4).map_err(|e| AppError::internal(format!("cid: {e}")))?;
                let created: String = row.get(5).map_err(|e| AppError::internal(format!("created: {e}")))?;
                let updated: String = row.get(6).map_err(|e| AppError::internal(format!("updated: {e}")))?;
                out.push(SavedQuery {
                    id: parse_uuid(&id_str, "saved_queries.id")?,
                    name,
                    folder,
                    sql: sql_text,
                    connection_id: parse_opt_uuid(cid, "saved_queries.connection_id")?,
                    created_at: parse_dt(&created, "saved_queries.created_at")?,
                    updated_at: parse_dt(&updated, "saved_queries.updated_at")?,
                });
            }
            Ok(out)
        })
    }

    async fn delete_saved(&self, id: Uuid) -> Result<(), AppError> {
        let id_str = id.to_string();
        self.with_conn(|conn| {
            let n = conn
                .execute(
                    "DELETE FROM saved_queries WHERE id = ?1",
                    params![id_str],
                )
                .map_err(|e| AppError::internal(format!("delete saved: {e}")))?;
            if n == 0 {
                return Err(AppError::NotFound(format!("saved query {id}")));
            }
            Ok(())
        })
    }

    async fn rename_saved(&self, id: Uuid, name: &str) -> Result<SavedQuery, AppError> {
        let now = Utc::now();
        let id_str = id.to_string();
        let name = name.to_string();
        let updated_at_str = now.to_rfc3339();
        self.with_conn(|conn| {
            let n = conn
                .execute(
                    "UPDATE saved_queries SET name = ?1, updated_at = ?2 WHERE id = ?3",
                    params![name, updated_at_str, id_str],
                )
                .map_err(|e| AppError::internal(format!("rename saved: {e}")))?;
            if n == 0 {
                return Err(AppError::NotFound(format!("saved query {id}")));
            }
            // Re-read the row so the returned SavedQuery reflects the true
            // stored state (name updated, updated_at bumped, everything else
            // preserved).
            let row = conn
                .query_row(
                    "SELECT id, name, folder, sql, connection_id, created_at, updated_at \
                     FROM saved_queries WHERE id = ?1",
                    params![id_str],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                            row.get::<_, Option<String>>(4)?,
                            row.get::<_, String>(5)?,
                            row.get::<_, String>(6)?,
                        ))
                    },
                )
                .map_err(|e| AppError::internal(format!("re-read saved: {e}")))?;
            Ok(SavedQuery {
                id: parse_uuid(&row.0, "id")?,
                name: row.1,
                folder: row.2,
                sql: row.3,
                connection_id: parse_opt_uuid(row.4, "connection_id")?,
                created_at: parse_dt(&row.5, "created_at")?,
                updated_at: parse_dt(&row.6, "updated_at")?,
            })
        })
    }

    async fn log_history(&self, entry: HistoryEntry) -> Result<(), AppError> {
        let id_str = entry.id.to_string();
        let conn_id_str = entry.connection_id.map(|u| u.to_string());
        let executed_at_str = entry.executed_at.to_rfc3339();
        let row_count = entry.row_count.map(|n| n as i64);
        let duration_ms = entry.duration_ms.map(|n| n as i64);
        let max_rows = self.max_rows;
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO query_history \
                 (id, sql, connection_id, executed_at, row_count, duration_ms, error) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    id_str,
                    entry.sql,
                    conn_id_str,
                    executed_at_str,
                    row_count,
                    duration_ms,
                    entry.error,
                ],
            )
            .map_err(|e| AppError::internal(format!("insert history: {e}")))?;
            Self::enforce_max_rows(conn, max_rows)?;
            Ok(())
        })
    }

    async fn list_history(&self, filter: HistoryFilter) -> Result<Vec<HistoryEntry>, AppError> {
        let cap = filter.limit.unwrap_or(self.max_rows).max(1);
        self.with_conn(|conn| {
            let mut sql = String::from(
                "SELECT id, sql, connection_id, executed_at, row_count, duration_ms, error \
                 FROM query_history WHERE 1=1",
            );
            let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
            if let Some(search) = filter.search.as_ref().filter(|s| !s.is_empty()) {
                sql.push_str(" AND lower(sql) LIKE ?");
                args.push(Box::new(format!("%{}%", search.to_lowercase())));
            }
            if let Some(cid) = filter.connection_id {
                sql.push_str(" AND connection_id = ?");
                args.push(Box::new(cid.to_string()));
            }
            sql.push_str(" ORDER BY executed_at DESC, rowid DESC LIMIT ?");
            args.push(Box::new(cap as i64));

            let mut stmt = conn
                .prepare(&sql)
                .map_err(|e| AppError::internal(format!("prepare list_history: {e}")))?;
            let arg_refs: Vec<&dyn rusqlite::ToSql> =
                args.iter().map(|b| b.as_ref()).collect();
            let mut rows = stmt
                .query(rusqlite::params_from_iter(arg_refs.iter()))
                .map_err(|e| AppError::internal(format!("query list_history: {e}")))?;
            let mut out = Vec::new();
            while let Some(row) = rows
                .next()
                .map_err(|e| AppError::internal(format!("next history: {e}")))?
            {
                let id_str: String = row.get(0).map_err(|e| AppError::internal(format!("id: {e}")))?;
                let sql_text: String = row.get(1).map_err(|e| AppError::internal(format!("sql: {e}")))?;
                let cid: Option<String> = row.get(2).map_err(|e| AppError::internal(format!("cid: {e}")))?;
                let executed_at: String = row.get(3).map_err(|e| AppError::internal(format!("executed_at: {e}")))?;
                let row_count: Option<i64> = row.get(4).map_err(|e| AppError::internal(format!("row_count: {e}")))?;
                let duration_ms: Option<i64> = row.get(5).map_err(|e| AppError::internal(format!("duration_ms: {e}")))?;
                let error: Option<String> = row.get(6).map_err(|e| AppError::internal(format!("error: {e}")))?;
                out.push(HistoryEntry {
                    id: parse_uuid(&id_str, "query_history.id")?,
                    sql: sql_text,
                    connection_id: parse_opt_uuid(cid, "query_history.connection_id")?,
                    executed_at: parse_dt(&executed_at, "query_history.executed_at")?,
                    row_count: row_count.map(|n| n.max(0) as u64),
                    duration_ms: duration_ms.map(|n| n.max(0) as u64),
                    error,
                });
            }
            Ok(out)
        })
    }

    async fn clear_history(&self, older_than_days: Option<u32>) -> Result<u64, AppError> {
        match older_than_days {
            Some(days) => self.prune_older_than(days),
            None => self.with_conn(|conn| {
                let n = conn
                    .execute("DELETE FROM query_history", [])
                    .map_err(|e| AppError::internal(format!("clear history: {e}")))?;
                Ok(n as u64)
            }),
        }
    }
}

// Kept public so tests can borrow the connection helper if they need to
// probe the DB directly.
#[allow(dead_code)]
pub fn db_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("queries.db")
}

#[allow(dead_code)]
fn optional_row<T>(r: Result<T, rusqlite::Error>) -> Result<Option<T>, AppError> {
    r.optional()
        .map_err(|e| AppError::internal(format!("optional row: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn mk_repo(tmp: &TempDir) -> SqliteQueriesRepo {
        SqliteQueriesRepo::open(tmp.path(), 100, "General".into(), 90)
            .expect("open repo")
    }

    #[tokio::test]
    async fn migration_is_idempotent() {
        let tmp = TempDir::new().expect("tempdir");
        // First open runs the schema init.
        let _r1 = mk_repo(&tmp);
        // Second open on the same path re-runs it — must not fail on existing
        // tables/indexes.
        let _r2 = mk_repo(&tmp);
        // Third for good measure.
        let _r3 = mk_repo(&tmp);
    }

    #[tokio::test]
    async fn save_list_delete_roundtrip() {
        let tmp = TempDir::new().expect("tempdir");
        let repo = mk_repo(&tmp);

        let cid = Uuid::new_v4();
        let saved = repo
            .save_query("first", Some("Reports"), "SELECT 1", Some(cid))
            .await
            .expect("save");
        assert_eq!(saved.name, "first");
        assert_eq!(saved.folder, "Reports");
        assert_eq!(saved.connection_id, Some(cid));

        let list = repo
            .list_saved(SavedQueryFilter::default())
            .await
            .expect("list");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, saved.id);

        repo.delete_saved(saved.id).await.expect("delete");

        let empty = repo
            .list_saved(SavedQueryFilter::default())
            .await
            .expect("list after delete");
        assert!(empty.is_empty());
    }

    #[tokio::test]
    async fn default_folder_used_when_none_passed() {
        let tmp = TempDir::new().expect("tempdir");
        let repo = SqliteQueriesRepo::open(tmp.path(), 100, "MyDefault".into(), 90)
            .expect("open");

        let saved = repo
            .save_query("no-folder", None, "SELECT 1", None)
            .await
            .expect("save");
        assert_eq!(saved.folder, "MyDefault");
    }

    #[tokio::test]
    async fn saved_search_matches_case_insensitively() {
        let tmp = TempDir::new().expect("tempdir");
        let repo = mk_repo(&tmp);

        repo.save_query("Users Report", None, "SELECT *", None)
            .await
            .expect("save1");
        repo.save_query("Orders", None, "SELECT *", None)
            .await
            .expect("save2");

        let filter = SavedQueryFilter {
            search: Some("USER".into()),
            ..Default::default()
        };
        let hits = repo.list_saved(filter).await.expect("list");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].name, "Users Report");
    }

    #[tokio::test]
    async fn saved_filter_by_connection_id() {
        let tmp = TempDir::new().expect("tempdir");
        let repo = mk_repo(&tmp);
        let cid_a = Uuid::new_v4();
        let cid_b = Uuid::new_v4();

        repo.save_query("A", None, "SELECT 1", Some(cid_a)).await.expect("a");
        repo.save_query("B", None, "SELECT 2", Some(cid_b)).await.expect("b");
        repo.save_query("Any", None, "SELECT 3", None).await.expect("any");

        let hits = repo
            .list_saved(SavedQueryFilter {
                connection_id: Some(cid_a),
                ..Default::default()
            })
            .await
            .expect("list");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].name, "A");
    }

    #[tokio::test]
    async fn rename_updates_name_and_bumps_updated_at() {
        let tmp = TempDir::new().expect("tempdir");
        let repo = mk_repo(&tmp);
        let saved = repo
            .save_query("Old Name", None, "SELECT 1", None)
            .await
            .expect("save");
        // Sleep just enough so updated_at is definitely different (RFC3339
        // second precision is enough; use tokio to keep the test async).
        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
        let renamed = repo
            .rename_saved(saved.id, "New Name")
            .await
            .expect("rename");
        assert_eq!(renamed.name, "New Name");
        assert!(renamed.updated_at > saved.updated_at);
    }

    #[tokio::test]
    async fn delete_nonexistent_returns_not_found() {
        let tmp = TempDir::new().expect("tempdir");
        let repo = mk_repo(&tmp);
        let err = repo.delete_saved(Uuid::new_v4()).await.unwrap_err();
        assert!(matches!(err, AppError::NotFound(_)));
    }

    fn mk_history(sql: &str, cid: Option<Uuid>, offset_min: i64) -> HistoryEntry {
        HistoryEntry {
            id: Uuid::new_v4(),
            sql: sql.into(),
            connection_id: cid,
            executed_at: Utc::now() - Duration::minutes(offset_min),
            row_count: Some(10),
            duration_ms: Some(42),
            error: None,
        }
    }

    #[tokio::test]
    async fn history_insert_and_list() {
        let tmp = TempDir::new().expect("tempdir");
        let repo = mk_repo(&tmp);

        repo.log_history(mk_history("SELECT 1", None, 5)).await.expect("h1");
        repo.log_history(mk_history("SELECT 2", None, 3)).await.expect("h2");
        repo.log_history(mk_history("SELECT 3", None, 1)).await.expect("h3");

        let all = repo
            .list_history(HistoryFilter::default())
            .await
            .expect("list");
        assert_eq!(all.len(), 3);
        // Newest first.
        assert_eq!(all[0].sql, "SELECT 3");
        assert_eq!(all[2].sql, "SELECT 1");
    }

    #[tokio::test]
    async fn history_search_and_connection_filter() {
        let tmp = TempDir::new().expect("tempdir");
        let repo = mk_repo(&tmp);
        let cid_a = Uuid::new_v4();
        let cid_b = Uuid::new_v4();

        repo.log_history(mk_history("SELECT * FROM Users", Some(cid_a), 5)).await.expect("h1");
        repo.log_history(mk_history("SELECT * FROM Orders", Some(cid_b), 3)).await.expect("h2");
        repo.log_history(mk_history("UPDATE Users SET x=1", Some(cid_a), 1)).await.expect("h3");

        // Substring search
        let hits = repo
            .list_history(HistoryFilter {
                search: Some("users".into()),
                ..Default::default()
            })
            .await
            .expect("list");
        assert_eq!(hits.len(), 2);

        // Connection filter
        let hits_b = repo
            .list_history(HistoryFilter {
                connection_id: Some(cid_b),
                ..Default::default()
            })
            .await
            .expect("list b");
        assert_eq!(hits_b.len(), 1);
        assert_eq!(hits_b[0].sql, "SELECT * FROM Orders");
    }

    #[tokio::test]
    async fn history_retention_prune_drops_old_rows() {
        let tmp = TempDir::new().expect("tempdir");
        let repo = mk_repo(&tmp);
        // Directly insert an old row and a fresh row.
        let old = HistoryEntry {
            id: Uuid::new_v4(),
            sql: "old".into(),
            connection_id: None,
            executed_at: Utc::now() - Duration::days(120),
            row_count: None,
            duration_ms: None,
            error: None,
        };
        let fresh = HistoryEntry {
            id: Uuid::new_v4(),
            sql: "fresh".into(),
            connection_id: None,
            executed_at: Utc::now(),
            row_count: None,
            duration_ms: None,
            error: None,
        };
        repo.log_history(old).await.expect("log old");
        repo.log_history(fresh).await.expect("log fresh");

        let deleted = repo.clear_history(Some(30)).await.expect("prune");
        assert_eq!(deleted, 1);

        let remaining = repo
            .list_history(HistoryFilter::default())
            .await
            .expect("list");
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].sql, "fresh");
    }

    #[tokio::test]
    async fn history_max_rows_cap_enforced() {
        let tmp = TempDir::new().expect("tempdir");
        // Cap of 3 rows.
        let repo = SqliteQueriesRepo::open(tmp.path(), 3, "General".into(), 90)
            .expect("open");
        for i in 0..5 {
            let mut h = mk_history(&format!("SELECT {i}"), None, 0);
            // Stagger executed_at so ordering is deterministic.
            h.executed_at = Utc::now() - Duration::seconds((5 - i) as i64);
            repo.log_history(h).await.expect("log");
        }
        let all = repo
            .list_history(HistoryFilter::default())
            .await
            .expect("list");
        assert_eq!(all.len(), 3);
        // The three most recent (by executed_at DESC) should remain.
        assert_eq!(all[0].sql, "SELECT 4");
        assert_eq!(all[1].sql, "SELECT 3");
        assert_eq!(all[2].sql, "SELECT 2");
    }

    #[tokio::test]
    async fn clear_history_none_wipes_everything() {
        let tmp = TempDir::new().expect("tempdir");
        let repo = mk_repo(&tmp);
        repo.log_history(mk_history("a", None, 1)).await.expect("a");
        repo.log_history(mk_history("b", None, 2)).await.expect("b");
        let n = repo.clear_history(None).await.expect("clear all");
        assert_eq!(n, 2);
        let empty = repo
            .list_history(HistoryFilter::default())
            .await
            .expect("list");
        assert!(empty.is_empty());
    }
}
