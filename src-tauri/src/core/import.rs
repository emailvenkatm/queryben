//! Import domain types. Mirrors the shape of `domain::export` so the two
//! features stay symmetric on the wire: CSV / JSON are the same enum names,
//! the config file shape reads like `export.config.json`, and the frontend
//! preview cell type reuses `CellValue` from `domain::query`.
//!
//! `ImportFormat` is `#[non_exhaustive]` for the same reason `ExportFormat`
//! is — a future Parquet / TSV / Excel importer should be a leaf change
//! (one new `DataImporter` impl + one registry insert), not a cascading
//! refactor of every `match` arm.

use serde::{Deserialize, Serialize};

use crate::core::query::CellValue;

/// Supported input formats. Kept in lockstep with `ImporterRegistry`
/// bindings — every variant here needs a matching `DataImporter` insert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum ImportFormat {
    Csv,
    Json,
}

impl ImportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            ImportFormat::Csv => "csv",
            ImportFormat::Json => "json",
        }
    }
}

/// The narrow set of SQL types the type inferer emits. Rendered directly
/// into `CREATE TABLE` column definitions when the "create if missing"
/// option is on. Ambiguous / mixed-type columns fall through to
/// `defaultStringType` from `import.config.json` (typically NVARCHAR(255) or
/// NVARCHAR(MAX)).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InferredType {
    Int,
    BigInt,
    Float,
    Bool,
    DateTime,
    NVarchar,
}

impl InferredType {
    /// Rendered form for `CREATE TABLE ... (col <this>)`. The nvarchar arm
    /// is a fallback — callers that already have a length from the config
    /// (`NVARCHAR(255)`) render that directly instead of calling this.
    pub fn to_sql(self) -> &'static str {
        match self {
            InferredType::Int => "INT",
            InferredType::BigInt => "BIGINT",
            InferredType::Float => "FLOAT",
            InferredType::Bool => "BIT",
            InferredType::DateTime => "DATETIME2",
            InferredType::NVarchar => "NVARCHAR(MAX)",
        }
    }
}

/// Result of `import_preview`. Columns come from the source header (CSV) or
/// the first object's keys (JSON); rows are the first N raw cells the source
/// produced, in the same order as `columns`.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    pub columns: Vec<InferredColumn>,
    pub rows: Vec<Vec<CellValue>>,
    pub total_rows_scanned: u32,
    pub format: ImportFormat,
}

/// Per-column preview info. `sample_values` isn't populated today; the
/// frontend uses `preview.rows[*][i]` to build its own sample strip.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InferredColumn {
    pub name: String,
    pub inferred_type: InferredType,
    pub nullable: bool,
}

/// One row of the source-column ↔ target-column mapping the user builds in
/// the wizard. `target_type` is what the mapping row's type selector
/// resolves to; when "create table" is on we render this into the CREATE.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ColumnMapping {
    pub source_column: String,
    pub target_column: String,
    pub target_type: String,
    /// `false` skips this column entirely — no CREATE line, no INSERT value.
    pub include: bool,
}

/// Runtime toggles the wizard surfaces as checkboxes. Defaults come from
/// `import.config.json` via `ImportConfig::to_options()`.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportOptions {
    pub create_table_if_missing: bool,
    pub truncate_before_insert: bool,
    pub skip_on_error: bool,
    pub chunk_size: u32,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            create_table_if_missing: true,
            truncate_before_insert: false,
            skip_on_error: false,
            chunk_size: 500,
        }
    }
}

/// Outcome surfaced back to the frontend after `import_execute`. Errors
/// carry the (1-based) source row index so the wizard can highlight which
/// row of the file blew up.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub rows_inserted: u64,
    pub rows_failed: u64,
    pub duration_ms: u32,
    pub errors: Vec<ImportRowError>,
    pub created_table: bool,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportRowError {
    pub row_index: u64,
    pub message: String,
}
