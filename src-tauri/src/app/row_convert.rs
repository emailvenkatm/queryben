//! Convert tiberius `Row` cells into the wire-shape `CellValue` the frontend
//! consumes, plus the `ColumnKind` classifier that keys the grid's type badge.

use tiberius::{ColumnData, Row};

use crate::adapters::base64;
use crate::core::query::{CellValue, ColumnKind};

pub(crate) fn row_to_cells(row: Row) -> Vec<CellValue> {
    row.into_iter().map(convert_cell).collect()
}

/// Map the tiberius `column_type()` debug label (e.g. "Intn", "Int4",
/// "BigVarChar", "NVarcharMax", "Datetimen", "Datetime2", "Bitn", "Floatn",
/// "Guid") onto the JS-facing `ColumnKind`. The browse grid keys its type-
/// badge palette off this enum directly; when the field is missing or
/// unrecognised the badge falls through to "unknown" and renders as "?".
pub(crate) fn classify_column_type(raw: &str) -> ColumnKind {
    // Tiberius names are PascalCase without underscores; case-insensitive
    // contains-checks are enough to survive minor version renames.
    let t = raw.to_ascii_lowercase();
    // Order matters: check bit/bool before "int" (Bitn contains no 'int',
    // but stay defensive).
    if t == "bit" || t.starts_with("bitn") || t == "bool" {
        return ColumnKind::Boolean;
    }
    if t.contains("int")
        || t.contains("float")
        || t.contains("real")
        || t.contains("money")
        || t.contains("decimal")
        || t.contains("numeric")
    {
        return ColumnKind::Number;
    }
    if t.contains("date") || t.contains("time") {
        return ColumnKind::Datetime;
    }
    if t.contains("char")
        || t.contains("text")
        || t.contains("xml")
        || t.contains("guid")
        || t.contains("uuid")
        || t.contains("varchar")
        || t.contains("string")
    {
        return ColumnKind::String;
    }
    ColumnKind::Unknown
}

// Row::into_iter yields `ColumnData<'static>`, and tiberius' `FromSql` requires
// the `'static` lifetime, so we bind it explicitly rather than eliding to
// `'_`. Row-owned string / binary variants keep their heap Cows; we just move
// them out with `.into_owned()`.
fn convert_cell(data: ColumnData<'static>) -> CellValue {
    use ColumnData::*;
    // Date-family branches use tiberius' own chrono `FromSql` impls (enabled by
    // the `chrono` + `tds73` features on the tiberius dep). We funnel every
    // temporal variant through `CellValue::DateTime(iso_string)` so the browse
    // grid's date-input renderers get a single canonical wire shape.
    match data {
        Bit(None) | U8(None) | I16(None) | I32(None) | I64(None) | F32(None) | F64(None)
        | String(None) | Guid(None) | Binary(None) | Numeric(None) | Xml(None)
        | DateTime(None) | SmallDateTime(None) | Time(None) | Date(None) | DateTime2(None)
        | DateTimeOffset(None) => CellValue::Null,
        Bit(Some(v)) => CellValue::Bool(v),
        U8(Some(v)) => CellValue::Int(v as i64),
        I16(Some(v)) => CellValue::Int(v as i64),
        I32(Some(v)) => CellValue::Int(v as i64),
        I64(Some(v)) => CellValue::Int(v),
        F32(Some(v)) => CellValue::Float(v as f64),
        F64(Some(v)) => CellValue::Float(v),
        String(Some(s)) => CellValue::Text(s.into_owned()),
        Guid(Some(g)) => CellValue::Text(g.to_string()),
        Binary(Some(b)) => CellValue::Bytes(base64::encode(&b)),
        // DECIMAL / NUMERIC / MONEY / SMALLMONEY: tiberius `Money`/`SmallMoney`
        // fixed-len types decode to `F64` (already handled above); the variable-
        // len `Decimaln`/`Numericn` land here. `Numeric::to_string()` renders as
        // e.g. "1200000.00" — precision-preserving string, mirrors how the
        // datetime branch avoids f64 lossiness.
        Numeric(Some(n)) => CellValue::Text(n.to_string()),
        Xml(Some(x)) => CellValue::Text(x.to_string()),
        v @ (Date(Some(_)) | Time(Some(_)) | SmallDateTime(Some(_)) | DateTime(Some(_))
        | DateTime2(Some(_)) | DateTimeOffset(Some(_))) => datetime_cell(v),
    }
}

// Format any of the six TDS temporal `ColumnData` variants as an ISO-8601
// string. Uses tiberius' chrono `FromSql` impls, which the `chrono` + `tds73`
// features on the tiberius dep provide. A conversion miss (should not happen
// for the variants the caller filters to) falls back to `{:?}` so the cell
// still renders something instead of blowing up the whole result set.
fn datetime_cell(data: ColumnData<'static>) -> CellValue {
    use tiberius::FromSql;
    let debug_fallback = || CellValue::Text(format!("{:?}", &data));
    let iso: Option<String> = match &data {
        ColumnData::Date(_) => chrono::NaiveDate::from_sql(&data)
            .ok()
            .flatten()
            .map(|d| d.format("%Y-%m-%d").to_string()),
        ColumnData::Time(_) => chrono::NaiveTime::from_sql(&data)
            .ok()
            .flatten()
            .map(|t| t.format("%H:%M:%S%.f").to_string()),
        ColumnData::SmallDateTime(_) | ColumnData::DateTime(_) | ColumnData::DateTime2(_) => {
            chrono::NaiveDateTime::from_sql(&data)
                .ok()
                .flatten()
                .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S%.f").to_string())
        }
        ColumnData::DateTimeOffset(_) => {
            // FromSql impl targeting FixedOffset preserves the source zone; the
            // Utc impl would silently normalize and drop the offset the user
            // stored.
            <chrono::DateTime<chrono::FixedOffset> as FromSql>::from_sql(&data)
                .ok()
                .flatten()
                .map(|dt| dt.to_rfc3339())
        }
        _ => None,
    };
    match iso {
        Some(s) => CellValue::DateTime(s),
        None => debug_fallback(),
    }
}
