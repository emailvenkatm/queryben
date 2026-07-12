//! Theme override loader. Users drop a JSON file at
//! `<app_data_dir>/theme.json` (macOS: `~/Library/Application
//! Support/QueryBen/theme.json`; Windows: `%APPDATA%\com.queryben.app\theme.json`)
//! and it wins over the selected preset. Best-effort: missing file, invalid
//! JSON, or IO error all return `None` — no error is surfaced to the frontend.
//!
//! Returns the JSON as a raw string rather than a parsed value so specta
//! (which cannot codegen `serde_json::Value`) stays happy. Frontend parses it.

use tauri::{AppHandle, Manager};

/// Read `theme.json` from the app-data directory if it exists. Returns the
/// file contents as a UTF-8 string, or `None` when missing/unreadable. We do
/// a cheap JSON syntax check before returning so obviously-malformed files
/// don't reach the frontend.
#[tauri::command]
#[specta::specta]
pub async fn read_theme_override_file(
    app: AppHandle,
) -> Result<Option<String>, String> {
    let dir = match app.path().app_data_dir() {
        Ok(d) => d,
        Err(_) => return Ok(None),
    };
    let path = dir.join("theme.json");
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(_) => return Ok(None),
    };
    let text = match String::from_utf8(bytes) {
        Ok(t) => t,
        Err(_) => return Ok(None),
    };
    // Cheap validity gate: parse-and-discard so a broken file surfaces as
    // "no override" instead of a JS-side JSON.parse throw.
    if serde_json::from_str::<serde_json::Value>(&text).is_err() {
        return Ok(None);
    }
    Ok(Some(text))
}
