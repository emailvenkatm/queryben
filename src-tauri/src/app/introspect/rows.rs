//! Small typed accessors for tiberius rows plus a raw-`ColumnData` escape
//! hatch for the sysname/nvarchar case where `try_get::<&str>` silently
//! returns empty despite the row holding a value.

use tiberius::{ColumnData, Row};

use crate::error::AppError;

pub(super) fn row_get_string(row: &Row, idx: usize, col_name: &str) -> Result<String, AppError> {
    // Path 1: &str by index (fast path, no allocation on the tiberius side).
    if let Ok(Some(v)) = row.try_get::<&str, _>(idx) {
        if !v.is_empty() {
            return Ok(v.to_string());
        }
        tracing::info!(
            target: "queryben::introspect",
            idx,
            col_name,
            "&str-by-idx returned empty; trying column-name lookup"
        );
    }
    // Path 2: &str by column name.
    if let Ok(Some(v)) = row.try_get::<&str, _>(col_name) {
        if !v.is_empty() {
            return Ok(v.to_string());
        }
        tracing::info!(
            target: "queryben::introspect",
            idx,
            col_name,
            "&str-by-name returned empty; falling through to raw ColumnData"
        );
    }
    // Path 3: reach into the raw `ColumnData` for the column and pull whatever
    // string-shaped variant it happens to be under. This is the escape hatch
    // for the sysname/nvarchar case where both `try_get::<&str>` paths above
    // come back empty despite the row clearly holding a value.
    if let Some((_col, cell)) = row.cells().nth(idx) {
        if let Some(s) = column_data_as_string(cell) {
            if !s.is_empty() {
                tracing::info!(
                    target: "queryben::introspect",
                    idx,
                    col_name,
                    "recovered from raw ColumnData path"
                );
                return Ok(s);
            }
        }
        tracing::warn!(
            target: "queryben::introspect",
            idx,
            col_name,
            debug = ?cell,
            "column value could not be decoded as string"
        );
    }
    Err(AppError::internal(format!(
        "schema row column {col_name} (idx {idx}) was NULL or unreadable"
    )))
}

fn column_data_as_string(data: &ColumnData<'_>) -> Option<String> {
    match data {
        ColumnData::String(Some(cow)) => Some(cow.to_string()),
        ColumnData::Xml(Some(x)) => Some(x.to_string()),
        ColumnData::Guid(Some(g)) => Some(g.to_string()),
        _ => None,
    }
}

pub(super) fn row_get_i64(row: &Row, idx: usize) -> Result<i64, AppError> {
    Ok(row
        .try_get::<i64, _>(idx)
        .map_err(AppError::from)?
        .unwrap_or(0))
}

pub(super) fn row_get_i32(row: &Row, idx: usize) -> Result<i32, AppError> {
    Ok(row
        .try_get::<i32, _>(idx)
        .map_err(AppError::from)?
        .unwrap_or(0))
}
