//! Notebook IPC. Four commands power the frontend:
//!   * `notebook_list`   — enumerate `.ipynb` files in the storage dir
//!   * `notebook_read`   — parse one file to a `Notebook`
//!   * `notebook_write`  — serialize a `Notebook` back to disk (nbformat 4.5)
//!   * `notebook_run_cell` — dispatch through the `NotebookCellExecutor`
//!     registry so future kernels plug in without a new command.
//!
//! The storage directory + max-rows cap come from `notebook.config.json`
//! (see `infra::notebook_config`); missing config falls back to defaults.

use std::path::PathBuf;
use std::sync::OnceLock;

use chrono::Utc;
use serde::Deserialize;
use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

use crate::core::notebook::{Cell, CellKind, Notebook, NotebookMeta, NotebookSummary};
use crate::error::AppError;
use crate::adapters::notebook_config::NotebookConfig;
use crate::adapters::notebook_executor::{
    default_registry, CellRegistry, CellRunContext, CellRunResult,
};
use crate::state::AppState;

fn registry() -> &'static CellRegistry {
    static REG: OnceLock<CellRegistry> = OnceLock::new();
    REG.get_or_init(default_registry)
}

fn config(app: &AppHandle) -> NotebookConfig {
    static CFG: OnceLock<NotebookConfig> = OnceLock::new();
    CFG.get_or_init(|| {
        let dir = app
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| PathBuf::from("."));
        NotebookConfig::load(&dir)
    })
    .clone()
}

fn storage_dir(app: &AppHandle) -> Result<PathBuf, AppError> {
    config(app)
        .resolve_storage_dir()
        .map_err(|e| AppError::internal(format!("resolve notebook dir: {e}")))
}

fn notebook_path(app: &AppHandle, id: &str) -> Result<PathBuf, AppError> {
    if id.contains('/') || id.contains('\\') || id.contains("..") {
        return Err(AppError::internal(format!("invalid notebook id: {id}")));
    }
    let file = if id.ends_with(".ipynb") {
        id.to_string()
    } else {
        format!("{id}.ipynb")
    };
    Ok(storage_dir(app)?.join(file))
}

#[tauri::command]
#[specta::specta]
pub async fn notebook_list(app: AppHandle) -> Result<Vec<NotebookSummary>, AppError> {
    let dir = storage_dir(&app)?;
    let read_dir = std::fs::read_dir(&dir)
        .map_err(|e| AppError::internal(format!("read {}: {e}", dir.display())))?;
    let mut out: Vec<NotebookSummary> = Vec::new();
    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("ipynb") {
            continue;
        }
        let stem = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        let modified_at = entry
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| {
                let dt: chrono::DateTime<Utc> = t.into();
                Some(dt.to_rfc3339())
            });
        out.push(NotebookSummary {
            id: stem.clone(),
            name: stem,
            path: path.to_string_lossy().to_string(),
            modified_at,
        });
    }
    // Newest first so the sidebar shows what the user was last editing.
    out.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
    Ok(out)
}

#[tauri::command]
#[specta::specta]
pub async fn notebook_read(app: AppHandle, id: String) -> Result<Notebook, AppError> {
    let path = notebook_path(&app, &id)?;
    let bytes = std::fs::read(&path)
        .map_err(|e| AppError::NotFound(format!("read {}: {e}", path.display())))?;
    let nb: Notebook = serde_json::from_slice(&bytes)
        .map_err(|e| AppError::internal(format!("parse {}: {e}", path.display())))?;
    Ok(nb)
}

#[tauri::command]
#[specta::specta]
pub async fn notebook_write(
    app: AppHandle,
    id: String,
    notebook: Notebook,
) -> Result<(), AppError> {
    let path = notebook_path(&app, &id)?;
    let mut nb = notebook;
    let now = Utc::now().to_rfc3339();
    if nb.metadata.created_at.is_none() {
        nb.metadata.created_at = Some(now.clone());
    }
    nb.metadata.updated_at = Some(now);
    let json = serde_json::to_vec_pretty(&nb)
        .map_err(|e| AppError::internal(format!("serialize notebook: {e}")))?;
    std::fs::write(&path, json)
        .map_err(|e| AppError::internal(format!("write {}: {e}", path.display())))?;
    Ok(())
}

/// Rename a notebook by moving its `.ipynb` file to a new stem and rewriting
/// the embedded `metadata.title` so the sidebar + toolbar stay in sync. The
/// new_name is sanitized to a filesystem-safe stem (path separators + `..`
/// stripped, control chars removed) before use.
#[tauri::command]
#[specta::specta]
pub async fn notebook_rename(
    app: AppHandle,
    id: String,
    new_name: String,
) -> Result<NotebookSummary, AppError> {
    let new_stem = sanitize_stem(&new_name);
    if new_stem.is_empty() {
        return Err(AppError::internal("new notebook name is empty".to_string()));
    }
    let old_path = notebook_path(&app, &id)?;
    let new_path = notebook_path(&app, &new_stem)?;

    if new_path != old_path && new_path.exists() {
        return Err(AppError::internal(format!(
            "a notebook named \"{new_stem}\" already exists"
        )));
    }

    let bytes = std::fs::read(&old_path)
        .map_err(|e| AppError::NotFound(format!("read {}: {e}", old_path.display())))?;
    let mut nb: Notebook = serde_json::from_slice(&bytes)
        .map_err(|e| AppError::internal(format!("parse {}: {e}", old_path.display())))?;
    nb.metadata.title = Some(new_name.clone());
    nb.metadata.updated_at = Some(Utc::now().to_rfc3339());
    let json = serde_json::to_vec_pretty(&nb)
        .map_err(|e| AppError::internal(format!("serialize notebook: {e}")))?;

    if new_path != old_path {
        std::fs::write(&new_path, json)
            .map_err(|e| AppError::internal(format!("write {}: {e}", new_path.display())))?;
        std::fs::remove_file(&old_path)
            .map_err(|e| AppError::internal(format!("remove {}: {e}", old_path.display())))?;
    } else {
        std::fs::write(&new_path, json)
            .map_err(|e| AppError::internal(format!("write {}: {e}", new_path.display())))?;
    }

    let modified_at = std::fs::metadata(&new_path)
        .ok()
        .and_then(|m| m.modified().ok())
        .map(|t| {
            let dt: chrono::DateTime<Utc> = t.into();
            dt.to_rfc3339()
        });

    Ok(NotebookSummary {
        id: new_stem.clone(),
        name: new_stem,
        path: new_path.to_string_lossy().to_string(),
        modified_at,
    })
}

fn sanitize_stem(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .filter(|c| !matches!(*c, '/' | '\\' | ':' | '\0'))
        .filter(|c| !c.is_control())
        .collect();
    let trimmed = cleaned.trim().trim_matches('.').trim();
    trimmed
        .strip_suffix(".ipynb")
        .unwrap_or(trimmed)
        .to_string()
}

#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RunCellInput {
    pub kind: CellKind,
    pub source: String,
    pub connection_id: Option<Uuid>,
}

#[tauri::command]
#[specta::specta]
pub async fn notebook_run_cell(
    app: AppHandle,
    state: State<'_, AppState>,
    input: RunCellInput,
) -> Result<CellRunResult, AppError> {
    let executor = registry().get(&input.kind).ok_or_else(|| {
        AppError::NotImplemented(format!("no executor registered for {:?}", input.kind))
    })?;
    let max_rows = config(&app).max_rows_per_cell;
    let ctx = CellRunContext {
        state,
        connection_id: input.connection_id,
        source: input.source,
        max_rows,
    };
    executor.run(ctx).await
}

// `Cell` is re-exported for the ipc surface so specta ships the type even
// though `notebook_read` returns a `Notebook` (which embeds Vec<Cell>).
#[allow(dead_code)]
fn _touch_cell_type(c: Cell) -> Cell {
    c
}

// `NotebookMeta` likewise — the Notebook shell exposes it, but keep the
// symbol reachable so external tooling (a future test) can construct one.
#[allow(dead_code)]
fn _touch_meta(m: NotebookMeta) -> NotebookMeta {
    m
}
