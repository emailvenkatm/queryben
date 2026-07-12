//! User-defined query snippet loader. Reads
//! `<app_data_dir>/snippets.json` and returns the raw text so the frontend
//! can parse + coerce it against the same `Snippet` shape used by the
//! bundled MSSQL catalog. Mirrors `theme::read_theme_override_file`:
//! missing file → Ok(None); IO or UTF-8 failure → Ok(None). We do a cheap
//! JSON syntax check before returning so a malformed file surfaces as
//! "no user snippets" instead of throwing a parse error on the JS side.
//!
//! We never *create* the file — users author it manually. See the
//! Snippets docs in the app for the shape they need to match.

use std::path::Path;

use tauri::{AppHandle, Manager};

use crate::error::AppError;

pub const SNIPPETS_FILE: &str = "snippets.json";

/// Pure-Rust core so integration tests can drive it against a `TempDir`
/// without spinning up Tauri. Returns `Ok(None)` for any recoverable
/// failure (missing file, non-UTF-8, invalid JSON).
pub fn load_snippets_from(dir: &Path) -> Result<Option<String>, AppError> {
    let path = dir.join(SNIPPETS_FILE);
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(_) => return Ok(None),
    };
    let text = match String::from_utf8(bytes) {
        Ok(t) => t,
        Err(_) => return Ok(None),
    };
    if serde_json::from_str::<serde_json::Value>(&text).is_err() {
        return Ok(None);
    }
    Ok(Some(text))
}

#[tauri::command]
#[specta::specta]
pub async fn read_user_snippets_file(app: AppHandle) -> Result<Option<String>, AppError> {
    let dir = match app.path().app_data_dir() {
        Ok(d) => d,
        Err(_) => return Ok(None),
    };
    load_snippets_from(&dir)
}
