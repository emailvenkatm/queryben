//! Data importer abstraction. `DataImporter` is the seam where new source
//! formats (Parquet, JSONL, XLSX) plug in without touching the IPC layer or
//! the wizard shell. Today we ship two impls:
//!   * `CsvImporter`  — RFC 4180ish reader, header row optional
//!   * `JsonImporter` — array-of-objects; object keys become columns
//!
//! Symmetric to `infra::exporter::RowExporter`. The registry pattern is the
//! same one the export path uses so a future "convert format" flow can bolt
//! an importer + exporter of different formats together with no glue.
//!
//! Type inference (`infer_column_types`) scans the first N rows and returns
//! the narrowest `InferredType` that satisfies every non-empty cell in a
//! column. Rules:
//!   * All cells parse as `i32` -> `Int`
//!   * All cells parse as `i64` but some overflow `i32` -> `BigInt`
//!   * All cells parse as `f64` (but not all integer) -> `Float`
//!   * All cells are `true`/`false`/`0`/`1` (case-insensitive) -> `Bool`
//!   * All cells parse as an ISO-8601 datetime -> `DateTime`
//!   * Anything else / mixed / empty -> `NVarchar`
//! Empty strings and NULLs never disqualify a stricter type; they just
//! don't contribute evidence.

use std::collections::HashMap;
use std::io::{BufReader, Read};
use std::path::Path;

use async_trait::async_trait;
use serde_json::Value as JsonValue;

use crate::core::import::{ImportFormat, ImportPreview, InferredColumn, InferredType};
use crate::core::query::CellValue;
use crate::error::AppError;

#[async_trait]
pub trait DataImporter: Send + Sync {
    fn extension(&self) -> &'static str;

    /// Read enough of `path` to fill `n` preview rows and infer per-column
    /// types across whatever the caller's config allows (typically 500).
    async fn preview(&self, path: &Path, n: usize) -> Result<ImportPreview, AppError>;

    /// Read every row into memory. Used by the import executor to hand rows
    /// to the chunked INSERT loop. For 100k+ row files this is fine on
    /// desktops but not on mobile — the wizard is desktop-only for now.
    async fn read_all(&self, path: &Path) -> Result<Vec<Vec<CellValue>>, AppError>;
}

// ---- CSV ---------------------------------------------------------------

pub struct CsvImporter {
    pub delimiter: u8,
    pub has_header: bool,
    pub sample_rows_for_inference: usize,
}

impl Default for CsvImporter {
    fn default() -> Self {
        Self {
            delimiter: b',',
            has_header: true,
            sample_rows_for_inference: 500,
        }
    }
}

impl CsvImporter {
    fn open_reader(&self, path: &Path) -> Result<csv::Reader<BufReader<std::fs::File>>, AppError> {
        let file = std::fs::File::open(path)
            .map_err(|e| AppError::internal(format!("open {}: {e}", path.display())))?;
        let reader = csv::ReaderBuilder::new()
            .delimiter(self.delimiter)
            .has_headers(self.has_header)
            .flexible(true)
            .from_reader(BufReader::new(file));
        Ok(reader)
    }

    fn headers_from(&self, rdr: &mut csv::Reader<BufReader<std::fs::File>>, first_row: Option<&csv::StringRecord>) -> Vec<String> {
        if self.has_header {
            rdr.headers()
                .map(|h| h.iter().map(String::from).collect())
                .unwrap_or_default()
        } else {
            match first_row {
                Some(r) => (0..r.len()).map(|i| format!("col{}", i + 1)).collect(),
                None => Vec::new(),
            }
        }
    }
}

#[async_trait]
impl DataImporter for CsvImporter {
    fn extension(&self) -> &'static str {
        "csv"
    }

    async fn preview(&self, path: &Path, n: usize) -> Result<ImportPreview, AppError> {
        let path = path.to_path_buf();
        let importer = CsvImporter {
            delimiter: self.delimiter,
            has_header: self.has_header,
            sample_rows_for_inference: self.sample_rows_for_inference,
        };
        let sample_target = importer.sample_rows_for_inference.max(n);
        tokio::task::spawn_blocking(move || {
            let mut rdr = importer.open_reader(&path)?;
            let mut all_string_rows: Vec<Vec<String>> = Vec::with_capacity(sample_target);
            for (idx, rec) in rdr.records().enumerate() {
                if idx >= sample_target {
                    break;
                }
                let rec = rec.map_err(|e| AppError::internal(format!("csv row {idx}: {e}")))?;
                all_string_rows.push(rec.iter().map(String::from).collect());
            }

            let headers: Vec<String> = if importer.has_header {
                rdr.headers()
                    .map(|h| h.iter().map(String::from).collect())
                    .unwrap_or_default()
            } else {
                match all_string_rows.first() {
                    Some(r) => (0..r.len()).map(|i| format!("col{}", i + 1)).collect(),
                    None => Vec::new(),
                }
            };

            let columns = infer_column_types(&headers, &all_string_rows);

            let preview_rows: Vec<Vec<CellValue>> = all_string_rows
                .iter()
                .take(n)
                .map(|r| {
                    r.iter()
                        .map(|s| {
                            if s.is_empty() {
                                CellValue::Null
                            } else {
                                CellValue::Text(s.clone())
                            }
                        })
                        .collect()
                })
                .collect();

            Ok(ImportPreview {
                columns,
                rows: preview_rows,
                total_rows_scanned: all_string_rows.len() as u32,
                format: ImportFormat::Csv,
            })
        })
        .await
        .map_err(|e| AppError::internal(format!("csv preview join: {e}")))?
    }

    async fn read_all(&self, path: &Path) -> Result<Vec<Vec<CellValue>>, AppError> {
        let path = path.to_path_buf();
        let delimiter = self.delimiter;
        let has_header = self.has_header;
        tokio::task::spawn_blocking(move || {
            let file = std::fs::File::open(&path)
                .map_err(|e| AppError::internal(format!("open {}: {e}", path.display())))?;
            let mut rdr = csv::ReaderBuilder::new()
                .delimiter(delimiter)
                .has_headers(has_header)
                .flexible(true)
                .from_reader(BufReader::new(file));
            let mut out: Vec<Vec<CellValue>> = Vec::new();
            for (idx, rec) in rdr.records().enumerate() {
                let rec = rec.map_err(|e| AppError::internal(format!("csv row {idx}: {e}")))?;
                let row: Vec<CellValue> = rec
                    .iter()
                    .map(|s| {
                        if s.is_empty() {
                            CellValue::Null
                        } else {
                            CellValue::Text(s.to_string())
                        }
                    })
                    .collect();
                out.push(row);
            }
            Ok(out)
        })
        .await
        .map_err(|e| AppError::internal(format!("csv read_all join: {e}")))?
    }
}

// ---- JSON --------------------------------------------------------------

pub struct JsonImporter {
    pub sample_rows_for_inference: usize,
}

impl Default for JsonImporter {
    fn default() -> Self {
        Self {
            sample_rows_for_inference: 500,
        }
    }
}

impl JsonImporter {
    fn parse_file(&self, path: &Path) -> Result<Vec<serde_json::Map<String, JsonValue>>, AppError> {
        let mut file = std::fs::File::open(path)
            .map_err(|e| AppError::internal(format!("open {}: {e}", path.display())))?;
        let mut buf = String::new();
        file.read_to_string(&mut buf)
            .map_err(|e| AppError::internal(format!("read {}: {e}", path.display())))?;

        // Empty file -> empty array. Blank/whitespace-only counts too.
        let trimmed = buf.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }

        let value: JsonValue = serde_json::from_str(trimmed)
            .map_err(|e| AppError::internal(format!("parse json: {e}")))?;
        match value {
            JsonValue::Array(items) => {
                let mut out: Vec<serde_json::Map<String, JsonValue>> = Vec::with_capacity(items.len());
                for (i, item) in items.into_iter().enumerate() {
                    match item {
                        JsonValue::Object(m) => out.push(m),
                        other => {
                            return Err(AppError::internal(format!(
                                "json row {i} is not an object (got {})",
                                describe_json(&other)
                            )));
                        }
                    }
                }
                Ok(out)
            }
            other => Err(AppError::internal(format!(
                "json root must be an array of objects (got {})",
                describe_json(&other)
            ))),
        }
    }
}

#[async_trait]
impl DataImporter for JsonImporter {
    fn extension(&self) -> &'static str {
        "json"
    }

    async fn preview(&self, path: &Path, n: usize) -> Result<ImportPreview, AppError> {
        let path = path.to_path_buf();
        let importer = JsonImporter {
            sample_rows_for_inference: self.sample_rows_for_inference,
        };
        tokio::task::spawn_blocking(move || {
            let items = importer.parse_file(&path)?;
            let sample_target = importer.sample_rows_for_inference.max(n).min(items.len());

            // Column discovery: union of keys across the first sample_target
            // rows, in the order they're first seen. Object-property order in
            // serde_json::Map preserves insertion order (BTreeMap replaced in
            // serde_json >=1.0 via `preserve_order` feature — we rely on
            // its stability enough that the wizard renders columns in a
            // deterministic order).
            let mut header_order: Vec<String> = Vec::new();
            let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
            for row in items.iter().take(sample_target) {
                for (k, _) in row.iter() {
                    if seen.insert(k.clone()) {
                        header_order.push(k.clone());
                    }
                }
            }

            // Build string samples for inference: JSON already carries typed
            // primitives, but we run the strings through the same inferer as
            // CSV so both formats resolve to the same InferredType set.
            let string_rows: Vec<Vec<String>> = items
                .iter()
                .take(sample_target)
                .map(|obj| {
                    header_order
                        .iter()
                        .map(|k| match obj.get(k) {
                            None | Some(JsonValue::Null) => String::new(),
                            Some(JsonValue::Bool(b)) => if *b { "true".into() } else { "false".into() },
                            Some(JsonValue::Number(n)) => n.to_string(),
                            Some(JsonValue::String(s)) => s.clone(),
                            Some(other) => other.to_string(),
                        })
                        .collect()
                })
                .collect();
            let columns = infer_column_types(&header_order, &string_rows);

            let preview_rows: Vec<Vec<CellValue>> = items
                .iter()
                .take(n)
                .map(|obj| {
                    header_order
                        .iter()
                        .map(|k| json_value_to_cell(obj.get(k)))
                        .collect()
                })
                .collect();

            Ok(ImportPreview {
                columns,
                rows: preview_rows,
                total_rows_scanned: items.len() as u32,
                format: ImportFormat::Json,
            })
        })
        .await
        .map_err(|e| AppError::internal(format!("json preview join: {e}")))?
    }

    async fn read_all(&self, path: &Path) -> Result<Vec<Vec<CellValue>>, AppError> {
        let path = path.to_path_buf();
        let importer = JsonImporter {
            sample_rows_for_inference: self.sample_rows_for_inference,
        };
        tokio::task::spawn_blocking(move || {
            let items = importer.parse_file(&path)?;
            // Column universe from the whole file, not just the sample —
            // otherwise a key that first appears past row 500 gets dropped
            // silently on import.
            let mut header_order: Vec<String> = Vec::new();
            let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
            for row in items.iter() {
                for (k, _) in row.iter() {
                    if seen.insert(k.clone()) {
                        header_order.push(k.clone());
                    }
                }
            }
            let rows: Vec<Vec<CellValue>> = items
                .into_iter()
                .map(|obj| {
                    header_order
                        .iter()
                        .map(|k| json_value_to_cell(obj.get(k)))
                        .collect()
                })
                .collect();
            Ok(rows)
        })
        .await
        .map_err(|e| AppError::internal(format!("json read_all join: {e}")))?
    }
}

fn json_value_to_cell(v: Option<&JsonValue>) -> CellValue {
    match v {
        None | Some(JsonValue::Null) => CellValue::Null,
        Some(JsonValue::Bool(b)) => CellValue::Bool(*b),
        Some(JsonValue::Number(n)) => {
            if let Some(i) = n.as_i64() {
                CellValue::Int(i)
            } else if let Some(f) = n.as_f64() {
                CellValue::Float(f)
            } else {
                CellValue::Text(n.to_string())
            }
        }
        Some(JsonValue::String(s)) => CellValue::Text(s.clone()),
        // Nested arrays/objects are dumped as JSON text — the importer treats
        // them as opaque nvarchar values, which is what the user gets in the
        // preview grid too.
        Some(other) => CellValue::Text(other.to_string()),
    }
}

fn describe_json(v: &JsonValue) -> &'static str {
    match v {
        JsonValue::Null => "null",
        JsonValue::Bool(_) => "bool",
        JsonValue::Number(_) => "number",
        JsonValue::String(_) => "string",
        JsonValue::Array(_) => "array",
        JsonValue::Object(_) => "object",
    }
}

// ---- Type inference ----------------------------------------------------

/// Scan `rows` column-by-column and pick the narrowest `InferredType` that
/// fits every non-empty cell in that column. Empty cells contribute nothing
/// to the decision; a column that's 100% empty falls back to `NVarchar`.
///
/// Detection order — strictest first, so an all-integer column doesn't get
/// misread as boolean or float:
///   1. Bool  — every value is `true` / `false` / `0` / `1`
///      (case-insensitive) AND at least one non-integer form appears (else
///      the column looks integer, and Int wins over Bool for `0/1`).
///   2. Int   — every value parses as `i32`.
///   3. BigInt — every value parses as `i64`, at least one exceeds i32.
///   4. Float — every value parses as `f64`, and at least one has a `.` or
///      exponent (integer strings alone shouldn't demote a BigInt column).
///   5. DateTime — every value parses as ISO-8601 date/datetime.
///   6. NVarchar — fallback.
pub fn infer_column_types(headers: &[String], rows: &[Vec<String>]) -> Vec<InferredColumn> {
    let mut out: Vec<InferredColumn> = Vec::with_capacity(headers.len());
    for (col_idx, name) in headers.iter().enumerate() {
        let mut all_int = true;
        let mut all_bigint = true;
        let mut all_float = true;
        let mut has_float_marker = false; // `.` or `e`/`E`
        let mut has_non_i32 = false;
        let mut all_bool = true;
        let mut has_true_false_literal = false;
        let mut all_datetime = true;
        let mut non_empty_count: usize = 0;
        let mut had_null: bool = false;

        for row in rows.iter() {
            let cell = row.get(col_idx).map(String::as_str).unwrap_or("");
            if cell.is_empty() {
                had_null = true;
                continue;
            }
            non_empty_count += 1;

            // Integer classification. i64 first so we can flag "exceeds i32".
            let as_i64 = cell.parse::<i64>().ok();
            match as_i64 {
                Some(v) => {
                    if v < i32::MIN as i64 || v > i32::MAX as i64 {
                        has_non_i32 = true;
                        all_int = false;
                    }
                }
                None => {
                    all_int = false;
                    all_bigint = false;
                }
            }

            let as_f64 = cell.parse::<f64>().ok();
            if as_f64.is_none() {
                all_float = false;
            } else if cell.contains('.') || cell.contains('e') || cell.contains('E') {
                has_float_marker = true;
            }

            let lower = cell.to_ascii_lowercase();
            let is_bool_literal = matches!(lower.as_str(), "true" | "false" | "0" | "1");
            if !is_bool_literal {
                all_bool = false;
            } else if lower == "true" || lower == "false" {
                has_true_false_literal = true;
            }

            if !looks_like_datetime(cell) {
                all_datetime = false;
            }
        }

        let inferred = if non_empty_count == 0 {
            InferredType::NVarchar
        } else if all_bool && has_true_false_literal {
            InferredType::Bool
        } else if all_int {
            InferredType::Int
        } else if all_bigint && !has_float_marker {
            InferredType::BigInt
        } else if all_float && has_float_marker {
            InferredType::Float
        } else if all_datetime {
            InferredType::DateTime
        } else {
            InferredType::NVarchar
        };

        out.push(InferredColumn {
            name: name.clone(),
            inferred_type: inferred,
            nullable: had_null,
        });
    }
    out
}

/// Extremely narrow ISO-8601 sniff — enough to catch the shapes tiberius
/// hands us on the way out (`YYYY-MM-DD`, `YYYY-MM-DDTHH:MM:SS[.fff]`,
/// `YYYY-MM-DD HH:MM:SS`, with optional trailing `Z` / `+HH:MM`).
/// Chrono round-trips are the source of truth for parsing at insert time;
/// here we only need to decide "this column looks like a date".
fn looks_like_datetime(s: &str) -> bool {
    let s = s.trim();
    if s.len() < 8 {
        return false;
    }
    // Try a few chrono formats. Any one that parses wins.
    let formats: &[&str] = &[
        "%Y-%m-%d",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%SZ",
        "%Y-%m-%dT%H:%M:%S%.fZ",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M:%S%.f",
    ];
    for fmt in formats {
        if chrono::NaiveDateTime::parse_from_str(s, fmt).is_ok() {
            return true;
        }
        if chrono::NaiveDate::parse_from_str(s, fmt).is_ok() {
            return true;
        }
    }
    // RFC 3339 with timezone offset — chrono has a dedicated parser.
    chrono::DateTime::parse_from_rfc3339(s).is_ok()
}

// ---- Registry ----------------------------------------------------------

pub type ImporterRegistry = HashMap<ImportFormat, Box<dyn DataImporter>>;

pub fn default_registry() -> ImporterRegistry {
    let mut map: ImporterRegistry = HashMap::new();
    map.insert(ImportFormat::Csv, Box::new(CsvImporter::default()));
    map.insert(ImportFormat::Json, Box::new(JsonImporter::default()));
    map
}

/// Build a registry from a live `ImportConfig`. Lets the caller push the
/// user's `csvDelimiter` / `csvHeader` / `sampleRowsForInference` into the
/// per-format impl without cloning the config into every module.
pub fn registry_from_config(cfg: &crate::adapters::import_config::ImportConfig) -> ImporterRegistry {
    let mut map: ImporterRegistry = HashMap::new();
    let delim = cfg.csv_delimiter.bytes().next().unwrap_or(b',');
    map.insert(
        ImportFormat::Csv,
        Box::new(CsvImporter {
            delimiter: delim,
            has_header: cfg.csv_header,
            sample_rows_for_inference: cfg.sample_rows_for_inference as usize,
        }),
    );
    map.insert(
        ImportFormat::Json,
        Box::new(JsonImporter {
            sample_rows_for_inference: cfg.sample_rows_for_inference as usize,
        }),
    );
    map
}
