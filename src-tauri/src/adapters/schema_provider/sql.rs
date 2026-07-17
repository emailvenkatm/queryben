//! SQL Server introspection SQL + shared row helpers.
//!
//! The five statements below mirror the shape of the object-explorer
//! introspection in `ipc::query` but pull enough detail to diff two schemas.
//! Kept as local const strings (not shared) so the object-explorer path stays
//! independently readable.

use tiberius::{Query, Row};

use crate::adapters::mssql::MssqlClient;
use crate::error::AppError;

pub(super) const TABLES_SQL: &str = "SELECT s.name AS schema_name, t.name AS table_name
       FROM sys.tables AS t
       JOIN sys.schemas AS s ON s.schema_id = t.schema_id
      WHERE s.name NOT IN ('sys','INFORMATION_SCHEMA','guest')
        AND s.name NOT LIKE 'db\\_%' ESCAPE '\\'";

pub(super) const VIEWS_SQL: &str = "SELECT s.name AS schema_name, v.name AS view_name,
            OBJECT_DEFINITION(v.object_id) AS body
       FROM sys.views AS v
       JOIN sys.schemas AS s ON s.schema_id = v.schema_id
      WHERE s.name NOT IN ('sys','INFORMATION_SCHEMA','guest')
        AND s.name NOT LIKE 'db\\_%' ESCAPE '\\'";

pub(super) const PROCS_SQL: &str = "SELECT s.name AS schema_name, p.name AS proc_name,
            OBJECT_DEFINITION(p.object_id) AS body
       FROM sys.procedures AS p
       JOIN sys.schemas AS s ON s.schema_id = p.schema_id
      WHERE s.name NOT IN ('sys','INFORMATION_SCHEMA','guest')
        AND s.name NOT LIKE 'db\\_%' ESCAPE '\\'";

pub(super) const FNS_SQL: &str = "SELECT s.name AS schema_name, o.name AS fn_name,
            OBJECT_DEFINITION(o.object_id) AS body
       FROM sys.objects AS o
       JOIN sys.schemas AS s ON s.schema_id = o.schema_id
      WHERE o.type IN ('FN','IF','TF')
        AND s.name NOT IN ('sys','INFORMATION_SCHEMA','guest')
        AND s.name NOT LIKE 'db\\_%' ESCAPE '\\'";

// Column shape per table. Same joins the object-explorer's per-table drill-in
// uses, just yanked across every table in one pass.
pub(super) const COLUMNS_SQL: &str = "SELECT s.name AS schema_name,
            t.name AS table_name,
            c.name AS column_name,
            TYPE_NAME(c.user_type_id) AS data_type,
            c.max_length,
            c.precision,
            c.scale,
            c.is_nullable,
            c.is_identity,
            c.is_computed,
            OBJECT_DEFINITION(dc.object_id) AS default_expr,
            c.column_id
       FROM sys.columns AS c
       JOIN sys.tables AS t ON t.object_id = c.object_id
       JOIN sys.schemas AS s ON s.schema_id = t.schema_id
       LEFT JOIN sys.default_constraints AS dc
              ON dc.parent_object_id = c.object_id
             AND dc.parent_column_id = c.column_id
      WHERE s.name NOT IN ('sys','INFORMATION_SCHEMA','guest')
        AND s.name NOT LIKE 'db\\_%' ESCAPE '\\'
      ORDER BY s.name, t.name, c.column_id";

// Index shape per table, including PK. Excludes heaps (index_id = 0).
pub(super) const INDEXES_SQL: &str = "SELECT s.name AS schema_name,
            t.name AS table_name,
            i.name AS index_name,
            i.is_unique,
            i.is_primary_key,
            (SELECT STRING_AGG(c.name, ',') WITHIN GROUP (ORDER BY ic.key_ordinal)
               FROM sys.index_columns AS ic
               JOIN sys.columns AS c
                 ON c.object_id = ic.object_id AND c.column_id = ic.column_id
              WHERE ic.object_id = i.object_id
                AND ic.index_id = i.index_id
                AND ic.is_included_column = 0) AS key_columns
       FROM sys.indexes AS i
       JOIN sys.tables AS t ON t.object_id = i.object_id
       JOIN sys.schemas AS s ON s.schema_id = t.schema_id
      WHERE i.index_id > 0
        AND i.name IS NOT NULL
        AND s.name NOT IN ('sys','INFORMATION_SCHEMA','guest')
        AND s.name NOT LIKE 'db\\_%' ESCAPE '\\'";

pub(super) async fn fetch(
    client: &mut MssqlClient,
    sql: &'static str,
) -> Result<Vec<Row>, AppError> {
    Ok(Query::new(sql)
        .query(client)
        .await?
        .into_first_result()
        .await?)
}

pub(super) fn str_col(row: &Row, idx: usize, label: &str) -> Result<String, AppError> {
    match row.try_get::<&str, _>(idx).map_err(AppError::from)? {
        Some(s) => Ok(s.to_string()),
        None => Err(AppError::internal(format!(
            "schema-compare row column {label} (idx {idx}) was NULL"
        ))),
    }
}

pub(super) fn format_sql_type(data_type: &str, max_length: i32, precision: i32, scale: i32) -> String {
    let dt = data_type.to_ascii_lowercase();
    match dt.as_str() {
        "char" | "varchar" | "binary" | "varbinary" => match max_length {
            -1 => format!("{dt}(max)"),
            n if n > 0 => format!("{dt}({n})"),
            _ => dt,
        },
        "nchar" | "nvarchar" => match max_length {
            -1 => format!("{dt}(max)"),
            // sys.columns.max_length is byte-length; nvarchar chars = bytes/2.
            n if n > 0 => format!("{dt}({})", n / 2),
            _ => dt,
        },
        "decimal" | "numeric" => format!("{dt}({precision},{scale})"),
        _ => dt,
    }
}

// "schema.table.index" -> ("[schema].[table]", "index").
pub(super) fn split_index_qname(qname: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = qname.split('.').collect();
    if parts.len() < 3 {
        return None;
    }
    let idx = parts.last()?.to_string();
    let schema = parts[0];
    let table = parts[1..parts.len() - 1].join(".");
    Some((format!("[{schema}].[{table}]"), idx))
}
