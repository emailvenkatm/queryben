//! Query result shape + row cap.

use serde::{Deserialize, Serialize};

// Hard cap on rows we ship over IPC. Streaming lands with pagination later.
pub const ROW_CAP: usize = 10_000;

/// Single result set inside a batch. A multi-statement query
/// (`SELECT 1; SELECT 2`) surfaces N of these — SSMS / ADS parity. Single-
/// statement queries still land as a one-element vec so the grid loop below
/// treats both cases identically.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ResultSet {
    pub columns: Vec<ColumnMeta>,
    pub rows: Vec<Vec<CellValue>>,
    pub row_count: u64,
    pub duration_ms: u32,
    pub truncated: bool,
}

/// Batch outcome. `result_sets` holds every SELECT that succeeded in-order;
/// `error` is Some(msg) when statement N blew up — earlier successful sets are
/// still returned so the frontend can render "N-1 grids then a red banner".
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct QueryOutcome {
    pub result_sets: Vec<ResultSet>,
    pub total_duration_ms: u32,
    pub error: Option<String>,
}

/// Legacy alias — the browse-mode path and pending-changes tray still call the
/// single-result shape `QueryResult`. Kept as a type alias so nothing else has
/// to churn. The command layer now returns `QueryOutcome`; browse-mode picks
/// `result_sets[0]` (single-statement SELECT is guaranteed to emit one set).
pub type QueryResult = ResultSet;

/// Column metadata surfaced with each result set. `column_type` is the
/// frontend-facing enum ("number" | "string" | ...) so the browse grid's
/// type-badge picker can look it up directly. `sql_type` keeps the raw
/// tiberius debug label around for the tooltip / editor input picker.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ColumnMeta {
    pub name: String,
    pub sql_type: String,
    pub column_type: ColumnKind,
    pub nullable: bool,
}

/// Broad JS-side value category. Mirrors `ColumnType` in `src/types/index.ts`
/// so the frontend can key its TYPE_STYLES table by this string directly.
/// Serialized with `#[serde(rename_all = "camelCase")]` on the wire.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ColumnKind {
    Number,
    String,
    Boolean,
    Datetime,
    Null,
    Unknown,
}

/// Untagged so each variant serializes as its bare JS payload:
///   `Null`          -> `null`
///   `Text("foo")`   -> `"foo"`
///   `Int(42)`       -> `42`
///   `Float(1.5)`    -> `1.5`
///   `Bool(true)`    -> `true`
///   `DateTime(...)` -> `"2024-01-02T03:04:05"`
///   `Bytes(...)`    -> `"<base64>"`
///
/// The browse grid's `CellDisplay` calls `String(value)` on the raw JSON
/// value; a tagged shape (`{ "type": "Int", "value": 42 }`) would render as
/// `[object Object]`. Frontend `CellValue` type is `string | number |
/// boolean | null`, which matches this untagged wire shape exactly.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(untagged)]
pub enum CellValue {
    // Order matters for serde(untagged) deserialization, but we only
    // serialize; the order below is just for readability.
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    // Covers Text, DateTime (ISO-8601), and Bytes (base64): all string on
    // the wire. The frontend distinguishes them via the column's SQL type,
    // not the cell shape.
    Text(String),
    DateTime(String),
    Bytes(String),
}
