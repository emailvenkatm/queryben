//! Row exporter abstraction. `RowExporter` is the seam where new formats
//! (Parquet, TSV, HTML) plug in without a new IPC command. Today we ship
//! three impls:
//!   * `CsvExporter`  — RFC 4180ish, `\r\n` line endings, quote-on-demand
//!   * `JsonExporter` — array of `{ column: value }` objects, nulls preserved
//!   * `XlsxExporter` — one sheet, one row per input row, header row on top
//!
//! Executors register into `HashMap<ExportFormat, Box<dyn RowExporter>>` via
//! `default_registry()`. The `commands::export::export_result_set` handler
//! grabs the right box by format, hands it a resolved path + rows + columns,
//! and returns an `ExportResult` describing the outcome.
//!
//! NULL handling policy — kept identical across formats so the same input
//! round-trips regardless of what the user picks:
//!   * CSV  — empty field (unquoted). Reads back as empty string in Excel;
//!     the alternative ("NULL" literal) would be ambiguous with real data.
//!   * JSON — JSON `null`. serde_json handles this natively.
//!   * XLSX — empty cell (no write). Excel renders it blank.

use std::collections::HashMap;
use std::path::Path;

use async_trait::async_trait;

use crate::core::export::{ExportFormat, ExportOptions, ExportResult};
use crate::core::query::{CellValue, ColumnMeta};
use crate::error::AppError;

#[async_trait]
pub trait RowExporter: Send + Sync {
    /// Filename extension without the leading dot. Handy for tests + for the
    /// registry lookup path when the caller only has a path in hand.
    fn extension(&self) -> &'static str;

    /// MIME type. Not used by the write path today, but ships in the trait
    /// so a future "share via drag-out" or HTTP endpoint has it available.
    fn mime(&self) -> &'static str;

    /// Serialize `rows` (already row-cap-truncated by the caller) to `path`.
    /// Impls are expected to be atomic — write to `path` directly, not to a
    /// temp file — because a partial file left behind on crash is easier for
    /// the user to spot and delete than a stealth temp file under `~/tmp/`.
    async fn write(
        &self,
        path: &Path,
        columns: &[ColumnMeta],
        rows: &[Vec<CellValue>],
        opts: &ExportOptions,
    ) -> Result<ExportResult, AppError>;
}

// ---- CSV ---------------------------------------------------------------

pub struct CsvExporter;

#[async_trait]
impl RowExporter for CsvExporter {
    fn extension(&self) -> &'static str {
        "csv"
    }
    fn mime(&self) -> &'static str {
        "text/csv"
    }

    async fn write(
        &self,
        path: &Path,
        columns: &[ColumnMeta],
        rows: &[Vec<CellValue>],
        opts: &ExportOptions,
    ) -> Result<ExportResult, AppError> {
        let mut buf = String::new();
        let delim = opts.csv_delimiter;

        if opts.csv_include_header {
            for (i, col) in columns.iter().enumerate() {
                if i > 0 {
                    buf.push(delim);
                }
                buf.push_str(&csv_escape(&col.name, delim));
            }
            buf.push_str("\r\n");
        }

        let mut rows_written: u64 = 0;
        for row in rows {
            for (i, cell) in row.iter().enumerate() {
                if i > 0 {
                    buf.push(delim);
                }
                buf.push_str(&csv_escape(&cell_to_string(cell), delim));
            }
            buf.push_str("\r\n");
            rows_written += 1;
        }

        let bytes = buf.into_bytes();
        let bytes_written = bytes.len() as u64;
        std::fs::write(path, &bytes)
            .map_err(|e| AppError::internal(format!("write {}: {e}", path.display())))?;

        Ok(ExportResult {
            rows_written,
            bytes_written,
            path: path.to_string_lossy().into_owned(),
        })
    }
}

/// RFC 4180 quoting: quote iff the field contains the delimiter, a quote,
/// CR, or LF. Internal quotes get doubled. Deliberately conservative —
/// unquoted output stays readable in `less` / `git diff` for small tables.
pub fn csv_escape(input: &str, delim: char) -> String {
    let needs_quote = input.chars().any(|c| c == delim || c == '"' || c == '\r' || c == '\n');
    if !needs_quote {
        return input.to_string();
    }
    let mut out = String::with_capacity(input.len() + 2);
    out.push('"');
    for ch in input.chars() {
        if ch == '"' {
            out.push('"');
        }
        out.push(ch);
    }
    out.push('"');
    out
}

// Serialize a `CellValue` for CSV/text output. NULLs → empty (see module
// header for rationale). Bytes are already base64-encoded strings on the
// wire so no special handling needed.
fn cell_to_string(cell: &CellValue) -> String {
    match cell {
        CellValue::Null => String::new(),
        CellValue::Bool(b) => if *b { "true" } else { "false" }.to_string(),
        CellValue::Int(n) => n.to_string(),
        CellValue::Float(f) => f.to_string(),
        CellValue::Text(s) => s.clone(),
        CellValue::DateTime(s) => s.clone(),
        CellValue::Bytes(s) => s.clone(),
    }
}

// ---- JSON --------------------------------------------------------------

pub struct JsonExporter;

#[async_trait]
impl RowExporter for JsonExporter {
    fn extension(&self) -> &'static str {
        "json"
    }
    fn mime(&self) -> &'static str {
        "application/json"
    }

    async fn write(
        &self,
        path: &Path,
        columns: &[ColumnMeta],
        rows: &[Vec<CellValue>],
        opts: &ExportOptions,
    ) -> Result<ExportResult, AppError> {
        // Array of objects keyed by column name. Pretty output on by default —
        // the file lives on disk, not on the wire, so bytes are cheap and
        // human readability wins.
        let mut docs: Vec<serde_json::Map<String, serde_json::Value>> =
            Vec::with_capacity(rows.len());
        for row in rows {
            let mut obj = serde_json::Map::with_capacity(columns.len());
            for (i, col) in columns.iter().enumerate() {
                let cell = row.get(i).unwrap_or(&CellValue::Null);
                obj.insert(col.name.clone(), cell_to_json(cell));
            }
            docs.push(obj);
        }

        let json = if opts.json_pretty {
            serde_json::to_vec_pretty(&docs)
        } else {
            serde_json::to_vec(&docs)
        }
        .map_err(|e| AppError::internal(format!("serialize export json: {e}")))?;

        let rows_written = rows.len() as u64;
        let bytes_written = json.len() as u64;
        std::fs::write(path, &json)
            .map_err(|e| AppError::internal(format!("write {}: {e}", path.display())))?;

        Ok(ExportResult {
            rows_written,
            bytes_written,
            path: path.to_string_lossy().into_owned(),
        })
    }
}

fn cell_to_json(cell: &CellValue) -> serde_json::Value {
    match cell {
        CellValue::Null => serde_json::Value::Null,
        CellValue::Bool(b) => serde_json::Value::Bool(*b),
        CellValue::Int(n) => serde_json::Value::from(*n),
        // f64::NaN / Inf are not valid JSON — coerce to null so the file
        // stays parseable in every downstream reader.
        CellValue::Float(f) => serde_json::Number::from_f64(*f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        CellValue::Text(s) => serde_json::Value::String(s.clone()),
        CellValue::DateTime(s) => serde_json::Value::String(s.clone()),
        CellValue::Bytes(s) => serde_json::Value::String(s.clone()),
    }
}

// ---- XLSX --------------------------------------------------------------

pub struct XlsxExporter;

#[async_trait]
impl RowExporter for XlsxExporter {
    fn extension(&self) -> &'static str {
        "xlsx"
    }
    fn mime(&self) -> &'static str {
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
    }

    async fn write(
        &self,
        path: &Path,
        columns: &[ColumnMeta],
        rows: &[Vec<CellValue>],
        opts: &ExportOptions,
    ) -> Result<ExportResult, AppError> {
        use rust_xlsxwriter::{Format, Workbook};

        let mut workbook = Workbook::new();
        let sheet_name = sanitize_sheet_name(&opts.xlsx_sheet_name);
        let worksheet = workbook
            .add_worksheet()
            .set_name(&sheet_name)
            .map_err(|e| AppError::internal(format!("xlsx sheet name: {e}")))?;

        let bold = Format::new().set_bold();

        // Header row.
        for (col_idx, col) in columns.iter().enumerate() {
            worksheet
                .write_string_with_format(0, col_idx as u16, &col.name, &bold)
                .map_err(|e| AppError::internal(format!("xlsx header: {e}")))?;
        }

        // Data rows. NULL cells are skipped (Excel renders empty).
        let mut rows_written: u64 = 0;
        for (row_idx, row) in rows.iter().enumerate() {
            // +1 because row 0 is the header.
            let sheet_row: u32 = (row_idx as u32) + 1;
            for (col_idx, cell) in row.iter().enumerate() {
                let c = col_idx as u16;
                match cell {
                    CellValue::Null => { /* leave empty */ }
                    CellValue::Bool(b) => {
                        worksheet
                            .write_boolean(sheet_row, c, *b)
                            .map_err(|e| AppError::internal(format!("xlsx bool: {e}")))?;
                    }
                    CellValue::Int(n) => {
                        // `write_number` takes f64; large i64 values (>2^53)
                        // will lose precision. Fine for typical row counts,
                        // documented so future-us knows why.
                        worksheet
                            .write_number(sheet_row, c, *n as f64)
                            .map_err(|e| AppError::internal(format!("xlsx int: {e}")))?;
                    }
                    CellValue::Float(f) => {
                        // NaN/Inf aren't representable — coerce to blank so
                        // the file stays valid.
                        if f.is_finite() {
                            worksheet
                                .write_number(sheet_row, c, *f)
                                .map_err(|e| AppError::internal(format!("xlsx float: {e}")))?;
                        }
                    }
                    CellValue::Text(s) | CellValue::DateTime(s) | CellValue::Bytes(s) => {
                        worksheet
                            .write_string(sheet_row, c, s)
                            .map_err(|e| AppError::internal(format!("xlsx string: {e}")))?;
                    }
                }
            }
            rows_written += 1;
        }

        workbook
            .save(path)
            .map_err(|e| AppError::internal(format!("xlsx save {}: {e}", path.display())))?;

        // rust_xlsxwriter writes directly — read back for the byte count.
        let bytes_written = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

        Ok(ExportResult {
            rows_written,
            bytes_written,
            path: path.to_string_lossy().into_owned(),
        })
    }
}

// Excel caps sheet names at 31 chars and forbids `:` `\` `/` `?` `*` `[` `]`.
// We coerce silently — the alternative (surface an error) would block the
// export on a user-provided config typo that they can't fix from the UI.
fn sanitize_sheet_name(input: &str) -> String {
    let cleaned: String = input
        .chars()
        .map(|c| match c {
            ':' | '\\' | '/' | '?' | '*' | '[' | ']' => '_',
            other => other,
        })
        .collect();
    let trimmed = if cleaned.chars().count() > 31 {
        cleaned.chars().take(31).collect()
    } else {
        cleaned
    };
    if trimmed.is_empty() {
        "Results".to_string()
    } else {
        trimmed
    }
}

// ---- Registry ----------------------------------------------------------

pub type ExporterRegistry = HashMap<ExportFormat, Box<dyn RowExporter>>;

/// Build the default registry. Adding a new format is a one-liner: implement
/// `RowExporter`, then insert here.
pub fn default_registry() -> ExporterRegistry {
    let mut map: ExporterRegistry = HashMap::new();
    map.insert(ExportFormat::Csv, Box::new(CsvExporter));
    map.insert(ExportFormat::Json, Box::new(JsonExporter));
    map.insert(ExportFormat::Xlsx, Box::new(XlsxExporter));
    map
}

// ---- Tests -------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::query::ColumnKind;

    fn col(name: &str) -> ColumnMeta {
        ColumnMeta {
            name: name.into(),
            sql_type: "nvarchar".into(),
            column_type: ColumnKind::String,
            nullable: true,
        }
    }

    #[test]
    fn csv_escape_plain_string_unquoted() {
        assert_eq!(csv_escape("hello", ','), "hello");
    }

    #[test]
    fn csv_escape_comma_is_quoted() {
        assert_eq!(csv_escape("a,b", ','), "\"a,b\"");
    }

    #[test]
    fn csv_escape_quote_is_doubled() {
        // `He said "hi"` → `"He said ""hi"""`
        assert_eq!(csv_escape("He said \"hi\"", ','), "\"He said \"\"hi\"\"\"");
    }

    #[test]
    fn csv_escape_newline_is_quoted() {
        assert_eq!(csv_escape("line1\nline2", ','), "\"line1\nline2\"");
    }

    #[test]
    fn csv_escape_crlf_is_quoted() {
        assert_eq!(csv_escape("a\r\nb", ','), "\"a\r\nb\"");
    }

    #[test]
    fn csv_escape_alt_delimiter_tab() {
        // With a tab delimiter, a comma inside the cell is NOT special —
        // stays unquoted. Only a literal tab would force quoting.
        assert_eq!(csv_escape("a,b", '\t'), "a,b");
        assert_eq!(csv_escape("a\tb", '\t'), "\"a\tb\"");
    }

    #[tokio::test]
    async fn csv_writer_null_becomes_empty_field() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("out.csv");
        let cols = vec![col("a"), col("b"), col("c")];
        let rows = vec![vec![
            CellValue::Text("x".into()),
            CellValue::Null,
            CellValue::Int(42),
        ]];
        let opts = ExportOptions::default();
        let result = CsvExporter
            .write(&path, &cols, &rows, &opts)
            .await
            .expect("write");
        assert_eq!(result.rows_written, 1);
        let text = std::fs::read_to_string(&path).expect("read");
        assert_eq!(text, "a,b,c\r\nx,,42\r\n");
    }

    #[tokio::test]
    async fn csv_writer_quotes_and_commas_and_newlines_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("out.csv");
        let cols = vec![col("name"), col("note")];
        let rows = vec![
            vec![
                CellValue::Text("Smith, John".into()),
                CellValue::Text("He said \"hi\"".into()),
            ],
            vec![
                CellValue::Text("multi\nline".into()),
                CellValue::Null,
            ],
        ];
        let opts = ExportOptions::default();
        CsvExporter
            .write(&path, &cols, &rows, &opts)
            .await
            .expect("write");
        let text = std::fs::read_to_string(&path).expect("read");
        assert_eq!(
            text,
            "name,note\r\n\"Smith, John\",\"He said \"\"hi\"\"\"\r\n\"multi\nline\",\r\n"
        );
    }

    #[tokio::test]
    async fn csv_writer_header_can_be_disabled() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("out.csv");
        let cols = vec![col("a"), col("b")];
        let rows = vec![vec![CellValue::Int(1), CellValue::Int(2)]];
        let opts = ExportOptions {
            csv_include_header: false,
            ..ExportOptions::default()
        };
        CsvExporter
            .write(&path, &cols, &rows, &opts)
            .await
            .expect("write");
        let text = std::fs::read_to_string(&path).expect("read");
        assert_eq!(text, "1,2\r\n");
    }

    #[tokio::test]
    async fn json_writer_preserves_null_and_types() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("out.json");
        let cols = vec![col("name"), col("age"), col("active")];
        let rows = vec![
            vec![
                CellValue::Text("Ada".into()),
                CellValue::Int(36),
                CellValue::Bool(true),
            ],
            vec![
                CellValue::Text("Bob".into()),
                CellValue::Null,
                CellValue::Bool(false),
            ],
        ];
        let opts = ExportOptions::default();
        JsonExporter
            .write(&path, &cols, &rows, &opts)
            .await
            .expect("write");
        let text = std::fs::read_to_string(&path).expect("read");
        let parsed: serde_json::Value = serde_json::from_str(&text).expect("parse");
        let arr = parsed.as_array().expect("array");
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["name"], "Ada");
        assert_eq!(arr[0]["age"], 36);
        assert_eq!(arr[0]["active"], true);
        assert!(arr[1]["age"].is_null());
    }

    #[tokio::test]
    async fn xlsx_writer_produces_nonempty_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("out.xlsx");
        let cols = vec![col("a"), col("b")];
        let rows = vec![
            vec![CellValue::Text("x".into()), CellValue::Int(1)],
            vec![CellValue::Null, CellValue::Int(2)],
        ];
        let opts = ExportOptions::default();
        let result = XlsxExporter
            .write(&path, &cols, &rows, &opts)
            .await
            .expect("write");
        assert_eq!(result.rows_written, 2);
        assert!(result.bytes_written > 0, "xlsx file should have content");
        // Minimal smoke check: XLSX is a ZIP, starts with `PK`.
        let bytes = std::fs::read(&path).expect("read");
        assert_eq!(&bytes[..2], b"PK");
    }

    #[test]
    fn sheet_name_sanitizer_strips_forbidden_chars_and_caps_length() {
        assert_eq!(sanitize_sheet_name("Results"), "Results");
        assert_eq!(sanitize_sheet_name("A/B?C"), "A_B_C");
        let long = "x".repeat(50);
        assert_eq!(sanitize_sheet_name(&long).chars().count(), 31);
        assert_eq!(sanitize_sheet_name(""), "Results");
    }

    #[test]
    fn default_registry_has_all_three_formats() {
        let reg = default_registry();
        assert!(reg.contains_key(&ExportFormat::Csv));
        assert!(reg.contains_key(&ExportFormat::Json));
        assert!(reg.contains_key(&ExportFormat::Xlsx));
    }
}
