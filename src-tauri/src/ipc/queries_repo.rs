//! IPC surface for Saved Queries + Query History.
//!
//! Thin `#[tauri::command]` wrappers that resolve an `Arc<dyn QueriesRepo>`
//! (lazily built against `<app_data_dir>/queries.db`) and delegate. The repo
//! is a `OnceLock` so we open the DB once per process lifetime and reuse the
//! connection + WAL journal across every command.

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::core::saved_query::{HistoryEntry, HistoryFilter, SavedQuery, SavedQueryFilter};
use crate::error::AppError;
use crate::adapters::queries_config::QueriesConfig;
use crate::adapters::queries_store::{QueriesRepo, SqliteQueriesRepo};

// One repo per process. Cargo tests that hit the trait directly build their
// own `SqliteQueriesRepo` via `open` — this cache is only for the runtime IPC
// layer.
static REPO: OnceLock<Arc<dyn QueriesRepo>> = OnceLock::new();

fn app_data_dir(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn repo(app: &AppHandle) -> Result<Arc<dyn QueriesRepo>, AppError> {
    if let Some(existing) = REPO.get() {
        return Ok(existing.clone());
    }
    let dir = app_data_dir(app);
    let cfg = QueriesConfig::load(&dir);
    let sqlite = SqliteQueriesRepo::open(
        &dir,
        cfg.history_max_rows,
        cfg.saved_queries_default_folder.clone(),
        cfg.history_retention_days,
    )?;
    let arc: Arc<dyn QueriesRepo> = Arc::new(sqlite);
    // Race-safe get_or_init substitute — if another thread beat us, use theirs.
    let _ = REPO.set(arc.clone());
    Ok(REPO.get().cloned().unwrap_or(arc))
}

// ---- Saved queries ---------------------------------------------------------

#[tauri::command]
#[specta::specta]
pub async fn save_query(
    app: AppHandle,
    name: String,
    folder: Option<String>,
    sql: String,
    connection_id: Option<Uuid>,
) -> Result<SavedQuery, AppError> {
    let repo = repo(&app)?;
    repo.save_query(&name, folder.as_deref(), &sql, connection_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_saved_queries(
    app: AppHandle,
    filter: Option<SavedQueryFilter>,
) -> Result<Vec<SavedQuery>, AppError> {
    let repo = repo(&app)?;
    repo.list_saved(filter.unwrap_or_default()).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_saved_query(app: AppHandle, id: Uuid) -> Result<(), AppError> {
    let repo = repo(&app)?;
    repo.delete_saved(id).await
}

#[tauri::command]
#[specta::specta]
pub async fn rename_saved_query(
    app: AppHandle,
    id: Uuid,
    name: String,
) -> Result<SavedQuery, AppError> {
    let repo = repo(&app)?;
    repo.rename_saved(id, &name).await
}

// ---- Query history ---------------------------------------------------------

#[tauri::command]
#[specta::specta]
pub async fn log_query_history(
    app: AppHandle,
    entry: HistoryEntry,
) -> Result<(), AppError> {
    // Config gate: users who flip `autoLogHistory` off in queries.config.json
    // get a silent no-op instead of a persisted row.
    let cfg = QueriesConfig::load(&app_data_dir(&app));
    if !cfg.auto_log_history {
        return Ok(());
    }
    let repo = repo(&app)?;
    repo.log_history(entry).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_query_history(
    app: AppHandle,
    filter: Option<HistoryFilter>,
) -> Result<Vec<HistoryEntry>, AppError> {
    let repo = repo(&app)?;
    repo.list_history(filter.unwrap_or_default()).await
}

#[tauri::command]
#[specta::specta]
pub async fn clear_query_history(
    app: AppHandle,
    older_than_days: Option<u32>,
) -> Result<u64, AppError> {
    let repo = repo(&app)?;
    repo.clear_history(older_than_days).await
}
