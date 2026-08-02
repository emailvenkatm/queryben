//! Introspection SQL constants. Each pair (`_ALL` / `_ONE`) exists because
//! batching them collapsed empty result sets on fresh Azure SQL DBs (tiberius
//! returned <5 vecs, non-deterministic which one dropped) which misaligned
//! parsing and turned table names into "null". The ~5x roundtrip cost is fine
//! for a 5-min-cached schema refresh.

use std::time::Instant;

use tiberius::{Query, Row};

use crate::adapters::mssql;
use crate::error::AppError;

pub(super) const SCHEMAS_SQL_ALL: &str = "SELECT s.name AS schema_name
       FROM sys.schemas AS s
      WHERE s.name NOT LIKE 'db\\_%' ESCAPE '\\'
        AND s.name NOT IN ('sys', 'INFORMATION_SCHEMA', 'guest')";

pub(super) const SCHEMAS_SQL_ONE: &str = "SELECT s.name AS schema_name
       FROM sys.schemas AS s
      WHERE s.name NOT LIKE 'db\\_%' ESCAPE '\\'
        AND s.name NOT IN ('sys', 'INFORMATION_SCHEMA', 'guest')
        AND s.name = @P1";

pub(super) const OBJECTS_SQL_ALL: &str = "SELECT t.TABLE_SCHEMA, t.TABLE_NAME, t.TABLE_TYPE
       FROM INFORMATION_SCHEMA.TABLES AS t
      WHERE t.TABLE_TYPE IN ('BASE TABLE', 'VIEW')";

pub(super) const OBJECTS_SQL_ONE: &str = "SELECT t.TABLE_SCHEMA, t.TABLE_NAME, t.TABLE_TYPE
       FROM INFORMATION_SCHEMA.TABLES AS t
      WHERE t.TABLE_TYPE IN ('BASE TABLE', 'VIEW')
        AND t.TABLE_SCHEMA = @P1";

pub(super) const PROCS_SQL_ALL: &str = "SELECT s.name AS schema_name, p.name AS proc_name
       FROM sys.procedures AS p
       JOIN sys.schemas    AS s ON s.schema_id = p.schema_id";

pub(super) const PROCS_SQL_ONE: &str = "SELECT s.name AS schema_name, p.name AS proc_name
       FROM sys.procedures AS p
       JOIN sys.schemas    AS s ON s.schema_id = p.schema_id
      WHERE s.name = @P1";

pub(super) const FNS_SQL_ALL: &str = "SELECT s.name AS schema_name, o.name AS fn_name
       FROM sys.objects AS o
       JOIN sys.schemas AS s ON s.schema_id = o.schema_id
      WHERE o.type IN ('FN', 'IF', 'TF')";

pub(super) const FNS_SQL_ONE: &str = "SELECT s.name AS schema_name, o.name AS fn_name
       FROM sys.objects AS o
       JOIN sys.schemas AS s ON s.schema_id = o.schema_id
      WHERE o.type IN ('FN', 'IF', 'TF')
        AND s.name = @P1";

pub(super) const STATS_SQL_ALL: &str = "SELECT SCHEMA_NAME(t.schema_id) AS schema_name,
            t.name                   AS table_name,
            ISNULL((SELECT SUM(p.rows)
                      FROM sys.partitions AS p
                     WHERE p.object_id = t.object_id
                       AND p.index_id IN (0, 1)), 0) AS row_estimate,
            (SELECT COUNT(*)
               FROM sys.columns AS c
              WHERE c.object_id = t.object_id) AS col_count
       FROM sys.tables AS t";

pub(super) const STATS_SQL_ONE: &str = "SELECT SCHEMA_NAME(t.schema_id) AS schema_name,
            t.name                   AS table_name,
            ISNULL((SELECT SUM(p.rows)
                      FROM sys.partitions AS p
                     WHERE p.object_id = t.object_id
                       AND p.index_id IN (0, 1)), 0) AS row_estimate,
            (SELECT COUNT(*)
               FROM sys.columns AS c
              WHERE c.object_id = t.object_id) AS col_count
       FROM sys.tables AS t
      WHERE SCHEMA_NAME(t.schema_id) = @P1";

// Column list for a single table. Combines INFORMATION_SCHEMA.COLUMNS (portable
// bits) with COLUMNPROPERTY / sys.computed_columns for the IDENTITY / computed
// flags that INFORMATION_SCHEMA doesn't expose. `is_rowversion` matches both
// the modern `rowversion` alias and the legacy `timestamp` synonym; either one
// is server-maintained and can't be written from a client statement.
pub(super) const COLUMNS_SQL: &str = "SELECT c.COLUMN_NAME,
       c.DATA_TYPE,
       c.CHARACTER_MAXIMUM_LENGTH,
       c.NUMERIC_PRECISION,
       c.NUMERIC_SCALE,
       CASE WHEN c.IS_NULLABLE = 'YES' THEN 1 ELSE 0 END AS is_nullable,
       COLUMNPROPERTY(OBJECT_ID(QUOTENAME(c.TABLE_SCHEMA) + '.' + QUOTENAME(c.TABLE_NAME)),
                      c.COLUMN_NAME, 'IsIdentity') AS is_identity,
       COLUMNPROPERTY(OBJECT_ID(QUOTENAME(c.TABLE_SCHEMA) + '.' + QUOTENAME(c.TABLE_NAME)),
                      c.COLUMN_NAME, 'IsComputed') AS is_computed,
       CASE WHEN LOWER(c.DATA_TYPE) IN ('timestamp', 'rowversion') THEN 1 ELSE 0 END AS is_rowversion,
       c.COLUMN_DEFAULT,
       c.ORDINAL_POSITION
  FROM INFORMATION_SCHEMA.COLUMNS AS c
 WHERE c.TABLE_SCHEMA = @P1
   AND c.TABLE_NAME = @P2
 ORDER BY c.ORDINAL_POSITION";

// Primary-key columns in key order. sys.indexes.is_primary_key filters to the
// clustered/nonclustered PK; key_ordinal preserves the composite order.
pub(super) const PK_SQL: &str = "SELECT c.name
  FROM sys.indexes AS i
  JOIN sys.index_columns AS ic
    ON ic.object_id = i.object_id AND ic.index_id = i.index_id
  JOIN sys.columns AS c
    ON c.object_id = ic.object_id AND c.column_id = ic.column_id
  JOIN sys.tables AS t
    ON t.object_id = i.object_id
  JOIN sys.schemas AS s
    ON s.schema_id = t.schema_id
 WHERE i.is_primary_key = 1
   AND s.name = @P1
   AND t.name = @P2
 ORDER BY ic.key_ordinal";

// Runs one introspection SELECT. When `only_schema` is Some, binds it as @P1
// and uses the ONE variant; otherwise runs the ALL variant with no bindings.
pub(super) async fn run_query(
    client: &mut mssql::MssqlClient,
    label: &'static str,
    sql_all: &'static str,
    sql_one: &'static str,
    only_schema: Option<&str>,
) -> Result<Vec<Row>, AppError> {
    let started = Instant::now();
    let rows = match only_schema {
        Some(name) => {
            let mut q = Query::new(sql_one);
            q.bind(name.to_string());
            q.query(client).await?.into_first_result().await?
        }
        None => {
            Query::new(sql_all)
                .query(client)
                .await?
                .into_first_result()
                .await?
        }
    };
    tracing::info!(
        target: "queryben::introspect",
        query = label,
        rows = rows.len(),
        duration_ms = started.elapsed().as_millis() as u64,
        "introspection query complete"
    );
    Ok(rows)
}

// Renders "nvarchar(50)" / "decimal(18,4)" / "int" from the raw DATA_TYPE +
// length/precision/scale INFORMATION_SCHEMA hands back. Length -1 = MAX.
pub(super) fn format_sql_type(
    data_type: &str,
    char_len: Option<i32>,
    numeric_precision: Option<i32>,
    numeric_scale: Option<i32>,
) -> String {
    let dt = data_type.to_ascii_lowercase();
    match dt.as_str() {
        "char" | "varchar" | "nchar" | "nvarchar" | "binary" | "varbinary" => match char_len {
            Some(-1) => format!("{dt}(max)"),
            Some(n) if n > 0 => format!("{dt}({n})"),
            _ => dt,
        },
        "decimal" | "numeric" => match (numeric_precision, numeric_scale) {
            (Some(p), Some(s)) => format!("{dt}({p},{s})"),
            (Some(p), None) => format!("{dt}({p})"),
            _ => dt,
        },
        _ => dt,
    }
}

// System schemas we hide from the object explorer. Any DB role starting with
// `db_` is also skipped programmatically because MSSQL keeps adding them.
const SYSTEM_SCHEMAS: &[&str] = &["sys", "INFORMATION_SCHEMA", "guest"];

pub(super) fn is_system_schema(name: &str) -> bool {
    SYSTEM_SCHEMAS.iter().any(|s| s.eq_ignore_ascii_case(name)) || name.starts_with("db_")
}
