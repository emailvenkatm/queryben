//! Export IPC. One command: `export_result_set(format, path, columns, rows)`.
//! Frontend picks the destination via the Tauri `dialog.save` plugin, hands
//! the path back to us, and we dispatch through the `ExporterRegistry` to
//! whatever `RowExporter` matches the requested format.
//!
//! Row cap: same 10k `ROW_CAP` the query pipeline uses. If the caller ships
//! more rows we truncate before writing and set `truncated` on the return
//! value implicitly (the ExportResult.rows_written reflects what actually
//! landed on disk, not the input length).

use std::path::PathBuf;
use std::sync::OnceLock;

use tauri::AppHandle;
use tauri::Manager;

use crate::core::export::{ExportFormat, ExportResult};
use crate::core::query::{CellValue, ColumnMeta, ROW_CAP};
use crate::error::AppError;
use crate::adapters::export_config::ExportConfig;
use crate::adapters::exporter::{default_registry, ExporterRegistry};

fn registry() -> &'static ExporterRegistry {
    static REG: OnceLock<ExporterRegistry> = OnceLock::new();
    REG.get_or_init(default_registry)
}

fn config(app: &AppHandle) -> ExportConfig {
    static CFG: OnceLock<ExportConfig> = OnceLock::new();
    CFG.get_or_init(|| {
        let dir = app
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| PathBuf::from("."));
        ExportConfig::load(&dir)
    })
    .clone()
}

/// Write `rows` to `path` in `format`. Path comes from the frontend save
/// dialog — we do NOT prompt from here, so the command is decoupled from the
/// dialog plugin and can be called headlessly (e.g. from a future CLI).
///
/// Behavior:
///   * Truncates `rows` to `ROW_CAP` (10k) before writing; anything past
///     that gets dropped silently and the returned `rows_written` reflects
///     what landed on disk. If the caller needs more, they should slice
///     with `LIMIT` in the SQL query itself.
///   * `columns` drives the header row and JSON keys. Extra cells beyond
///     `columns.len()` in a row are ignored; missing cells become NULL.
///   * NULL policy: empty CSV field, JSON `null`, empty XLSX cell.
#[tauri::command]
#[specta::specta]
pub async fn export_result_set(
    app: AppHandle,
    format: ExportFormat,
    path: String,
    columns: Vec<ColumnMeta>,
    rows: Vec<Vec<CellValue>>,
) -> Result<ExportResult, AppError> {
    let exporter = registry().get(&format).ok_or_else(|| {
        AppError::NotImplemented(format!("no exporter registered for {:?}", format))
    })?;

    // Row cap. The frontend already limits what it renders to ROW_CAP; this
    // is belt-and-suspenders for headless callers (notebook cells, CLI).
    let capped: Vec<Vec<CellValue>> = if rows.len() > ROW_CAP {
        tracing::warn!(
            target: "queryben::export",
            input_rows = rows.len(),
            cap = ROW_CAP,
            "row cap hit — truncating export"
        );
        rows.into_iter().take(ROW_CAP).collect()
    } else {
        rows
    };

    let opts = config(&app).to_options();
    let target = PathBuf::from(path);

    exporter.write(&target, &columns, &capped, &opts).await
}
