//! Export domain types. Pure data, no filesystem or Tauri deps — keeps the
//! type surface reusable by tests, notebooks (future), and eventually a CLI.
//!
//! `ExportFormat` is `#[non_exhaustive]` on purpose: adding a new format
//! (Parquet, TSV, HTML) is meant to be a leaf change — a new `RowExporter`
//! impl + a `default_registry()` insert — and marking the enum non-exhaustive
//! forces any downstream `match` to grow a wildcard arm on the same commit
//! that added the variant, not on some later refactor.

use serde::{Deserialize, Serialize};

/// Supported output formats. Frontend picks one via the export dialog radio.
/// `#[non_exhaustive]` — see module comment for the rationale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum ExportFormat {
    Csv,
    Json,
    Xlsx,
}

impl ExportFormat {
    /// Filename extension without the leading dot. Used by the frontend to
    /// pre-fill the save dialog's default filename and by the registry lookup
    /// path when the caller only has a path (rare).
    pub fn extension(self) -> &'static str {
        match self {
            ExportFormat::Csv => "csv",
            ExportFormat::Json => "json",
            ExportFormat::Xlsx => "xlsx",
        }
    }
}

/// Outcome surfaced to the frontend after a successful write. `path` is the
/// resolved absolute path (tilde expanded) so the toast can show it verbatim.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub rows_written: u64,
    pub bytes_written: u64,
    pub path: String,
}

/// Runtime knobs that came from `export.config.json`. Passed to each
/// `RowExporter` via `ExportOptions::for_format()` — that indirection keeps
/// format-specific options siloed instead of exploding one shared struct.
#[derive(Debug, Clone)]
pub struct ExportOptions {
    pub csv_delimiter: char,
    pub csv_include_header: bool,
    pub json_pretty: bool,
    pub xlsx_sheet_name: String,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            csv_delimiter: ',',
            csv_include_header: true,
            json_pretty: true,
            xlsx_sheet_name: "Results".into(),
        }
    }
}
