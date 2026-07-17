//! Object-explorer schema introspection: schemas, tables, views, procs, fns,
//! plus a fast row/column estimate per table.

use std::collections::BTreeMap;
use std::time::Instant;

use tiberius::{ColumnData, Query, Row};
use uuid::Uuid;

use crate::adapters::mssql;
use crate::core::schema::{
    RoutineInfo, SchemaInfo, SchemaNode, TableColumn, TableInfo, TableMetadata,
};
use crate::error::AppError;
use crate::state::AppState;

use super::session::reopen_input;

// System schemas we hide from the object explorer. Any DB role starting with
// `db_` is also skipped programmatically because MSSQL keeps adding them.
const SYSTEM_SCHEMAS: &[&str] = &["sys", "INFORMATION_SCHEMA", "guest"];

fn is_system_schema(name: &str) -> bool {
    SYSTEM_SCHEMAS.iter().any(|s| s.eq_ignore_ascii_case(name)) || name.starts_with("db_")
}

// The five introspection SQL statements. Each is executed as its own tiberius
// query in `introspect_all` — batching them collapsed empty result sets on
// fresh Azure SQL DBs (tiberius returned <5 vecs, non-deterministic which one
// dropped) which misaligned parsing and turned table names into "null".
// The ~5x roundtrip cost is fine for a 5-min-cached schema refresh.
const SCHEMAS_SQL_ALL: &str = "SELECT s.name AS schema_name
       FROM sys.schemas AS s
      WHERE s.name NOT LIKE 'db\\_%' ESCAPE '\\'
        AND s.name NOT IN ('sys', 'INFORMATION_SCHEMA', 'guest')";

const SCHEMAS_SQL_ONE: &str = "SELECT s.name AS schema_name
       FROM sys.schemas AS s
      WHERE s.name NOT LIKE 'db\\_%' ESCAPE '\\'
        AND s.name NOT IN ('sys', 'INFORMATION_SCHEMA', 'guest')
        AND s.name = @P1";

const OBJECTS_SQL_ALL: &str = "SELECT t.TABLE_SCHEMA, t.TABLE_NAME, t.TABLE_TYPE
       FROM INFORMATION_SCHEMA.TABLES AS t
      WHERE t.TABLE_TYPE IN ('BASE TABLE', 'VIEW')";

const OBJECTS_SQL_ONE: &str = "SELECT t.TABLE_SCHEMA, t.TABLE_NAME, t.TABLE_TYPE
       FROM INFORMATION_SCHEMA.TABLES AS t
      WHERE t.TABLE_TYPE IN ('BASE TABLE', 'VIEW')
        AND t.TABLE_SCHEMA = @P1";

const PROCS_SQL_ALL: &str = "SELECT s.name AS schema_name, p.name AS proc_name
       FROM sys.procedures AS p
       JOIN sys.schemas    AS s ON s.schema_id = p.schema_id";

const PROCS_SQL_ONE: &str = "SELECT s.name AS schema_name, p.name AS proc_name
       FROM sys.procedures AS p
       JOIN sys.schemas    AS s ON s.schema_id = p.schema_id
      WHERE s.name = @P1";

const FNS_SQL_ALL: &str = "SELECT s.name AS schema_name, o.name AS fn_name
       FROM sys.objects AS o
       JOIN sys.schemas AS s ON s.schema_id = o.schema_id
      WHERE o.type IN ('FN', 'IF', 'TF')";

const FNS_SQL_ONE: &str = "SELECT s.name AS schema_name, o.name AS fn_name
       FROM sys.objects AS o
       JOIN sys.schemas AS s ON s.schema_id = o.schema_id
      WHERE o.type IN ('FN', 'IF', 'TF')
        AND s.name = @P1";

const STATS_SQL_ALL: &str = "SELECT SCHEMA_NAME(t.schema_id) AS schema_name,
            t.name                   AS table_name,
            ISNULL((SELECT SUM(p.rows)
                      FROM sys.partitions AS p
                     WHERE p.object_id = t.object_id
                       AND p.index_id IN (0, 1)), 0) AS row_estimate,
            (SELECT COUNT(*)
               FROM sys.columns AS c
              WHERE c.object_id = t.object_id) AS col_count
       FROM sys.tables AS t";

const STATS_SQL_ONE: &str = "SELECT SCHEMA_NAME(t.schema_id) AS schema_name,
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
// flags that INFORMATION_SCHEMA doesn't expose.
const COLUMNS_SQL: &str = "SELECT c.COLUMN_NAME,
       c.DATA_TYPE,
       c.CHARACTER_MAXIMUM_LENGTH,
       c.NUMERIC_PRECISION,
       c.NUMERIC_SCALE,
       CASE WHEN c.IS_NULLABLE = 'YES' THEN 1 ELSE 0 END AS is_nullable,
       COLUMNPROPERTY(OBJECT_ID(QUOTENAME(c.TABLE_SCHEMA) + '.' + QUOTENAME(c.TABLE_NAME)),
                      c.COLUMN_NAME, 'IsIdentity') AS is_identity,
       COLUMNPROPERTY(OBJECT_ID(QUOTENAME(c.TABLE_SCHEMA) + '.' + QUOTENAME(c.TABLE_NAME)),
                      c.COLUMN_NAME, 'IsComputed') AS is_computed,
       c.COLUMN_DEFAULT,
       c.ORDINAL_POSITION
  FROM INFORMATION_SCHEMA.COLUMNS AS c
 WHERE c.TABLE_SCHEMA = @P1
   AND c.TABLE_NAME = @P2
 ORDER BY c.ORDINAL_POSITION";

// Primary-key columns in key order. sys.indexes.is_primary_key filters to the
// clustered/nonclustered PK; key_ordinal preserves the composite order.
const PK_SQL: &str = "SELECT c.name
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
async fn run_query(
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

// Pulled into a helper so both `get_schema` and any future per-schema drill-in
// share the same INFORMATION_SCHEMA / sys.* pass.
async fn introspect_all(
    client: &mut mssql::MssqlClient,
    only_schema: Option<&str>,
) -> Result<Vec<SchemaNode>, AppError> {
    tracing::info!(target: "queryben::introspect", only_schema = ?only_schema, "issuing introspection queries");
    let schema_rows = run_query(client, "schemas", SCHEMAS_SQL_ALL, SCHEMAS_SQL_ONE, only_schema).await?;
    let object_rows = run_query(client, "objects", OBJECTS_SQL_ALL, OBJECTS_SQL_ONE, only_schema).await?;
    let proc_rows   = run_query(client, "procs",   PROCS_SQL_ALL,   PROCS_SQL_ONE,   only_schema).await?;
    let fn_rows     = run_query(client, "fns",     FNS_SQL_ALL,     FNS_SQL_ONE,     only_schema).await?;
    let stats_rows  = run_query(client, "stats",   STATS_SQL_ALL,   STATS_SQL_ONE,   only_schema).await?;
    tracing::info!(
        target: "queryben::introspect",
        schemas = schema_rows.len(),
        objects = object_rows.len(),
        procs = proc_rows.len(),
        fns = fn_rows.len(),
        stats = stats_rows.len(),
        "parsed row counts"
    );

    let mut nodes: BTreeMap<String, SchemaNode> = BTreeMap::new();
    for row in schema_rows {
        let name = row_get_string(&row, 0, "schema_name")?;
        tracing::info!(target: "queryben::introspect", loop_ = "schemas", %name, "row");
        if is_system_schema(&name) {
            continue;
        }
        nodes.entry(name.clone()).or_insert_with(|| SchemaNode {
            name: name.clone(),
            tables: Vec::new(),
            views: Vec::new(),
            procedures: Vec::new(),
            functions: Vec::new(),
        });
    }

    // Row / column counts keyed by (schema, table). Missing means view or
    // no partition rows.
    let mut stats: BTreeMap<(String, String), (u64, u32)> = BTreeMap::new();
    for row in stats_rows {
        let schema = row_get_string(&row, 0, "schema_name")?;
        let table = row_get_string(&row, 1, "table_name")?;
        // SUM over BIGINT rows -> tiberius returns Numeric. Fall back to 0 if
        // the DB is empty or the column type surprises us.
        let rows_est = row_get_i64(&row, 2).unwrap_or(0).max(0) as u64;
        let col_count = row_get_i32(&row, 3).unwrap_or(0).max(0) as u32;
        tracing::info!(
            target: "queryben::introspect",
            loop_ = "stats",
            %schema,
            %table,
            rows_est,
            col_count,
            "row"
        );
        stats.insert((schema, table), (rows_est, col_count));
    }

    for row in object_rows {
        let schema = row_get_string(&row, 0, "TABLE_SCHEMA")?;
        let name = row_get_string(&row, 1, "TABLE_NAME")?;
        let kind = row_get_string(&row, 2, "TABLE_TYPE")?;
        tracing::info!(
            target: "queryben::introspect",
            loop_ = "objects",
            %schema,
            %name,
            %kind,
            "row"
        );
        if is_system_schema(&schema) {
            continue;
        }
        let node = nodes.entry(schema.clone()).or_insert_with(|| SchemaNode {
            name: schema.clone(),
            tables: Vec::new(),
            views: Vec::new(),
            procedures: Vec::new(),
            functions: Vec::new(),
        });
        let (row_count, column_count) = stats
            .get(&(schema.clone(), name.clone()))
            .map(|(r, c)| (Some(*r), Some(*c)))
            .unwrap_or((None, None));
        let table = TableInfo {
            schema: schema.clone(),
            name: name.clone(),
            row_count,
            column_count,
        };
        if kind == "VIEW" {
            node.views.push(table);
        } else {
            node.tables.push(table);
        }
    }

    for row in proc_rows {
        let schema = row_get_string(&row, 0, "schema_name")?;
        let name = row_get_string(&row, 1, "proc_name")?;
        tracing::info!(target: "queryben::introspect", loop_ = "procs", %schema, %name, "row");
        if is_system_schema(&schema) {
            continue;
        }
        let node = nodes.entry(schema.clone()).or_insert_with(|| SchemaNode {
            name: schema.clone(),
            tables: Vec::new(),
            views: Vec::new(),
            procedures: Vec::new(),
            functions: Vec::new(),
        });
        node.procedures.push(RoutineInfo {
            schema,
            name,
        });
    }

    for row in fn_rows {
        let schema = row_get_string(&row, 0, "schema_name")?;
        let name = row_get_string(&row, 1, "fn_name")?;
        tracing::info!(target: "queryben::introspect", loop_ = "fns", %schema, %name, "row");
        if is_system_schema(&schema) {
            continue;
        }
        let node = nodes.entry(schema.clone()).or_insert_with(|| SchemaNode {
            name: schema.clone(),
            tables: Vec::new(),
            views: Vec::new(),
            procedures: Vec::new(),
            functions: Vec::new(),
        });
        node.functions.push(RoutineInfo {
            schema,
            name,
        });
    }

    // Deterministic ordering makes the tree stable between refreshes.
    let mut out: Vec<SchemaNode> = nodes.into_values().collect();
    for node in &mut out {
        node.tables.sort_by(|a, b| a.name.cmp(&b.name));
        node.views.sort_by(|a, b| a.name.cmp(&b.name));
        node.procedures.sort_by(|a, b| a.name.cmp(&b.name));
        node.functions.sort_by(|a, b| a.name.cmp(&b.name));
    }
    Ok(out)
}

pub async fn get_schema(
    state: &AppState,
    connection_id: Uuid,
) -> Result<SchemaInfo, AppError> {
    tracing::info!(target: "queryben::get-schema", %connection_id, "entry");
    let snapshot = state.registry.snapshot(connection_id)?;
    let input = reopen_input(state, snapshot).await?;
    tracing::info!(target: "queryben::get-schema", %connection_id, "connecting");
    let mut client = mssql::connect_for_connection(&input, connection_id).await?;
    tracing::info!(target: "queryben::get-schema", %connection_id, "connected, introspecting");
    let schemas = introspect_all(&mut client, None).await.map_err(|e| {
        tracing::error!(target: "queryben::get-schema", %connection_id, error = %e, "introspection failed");
        e
    })?;
    tracing::info!(target: "queryben::get-schema", %connection_id, count = schemas.len(), "done");
    state.registry.mark_used(connection_id).ok();
    Ok(SchemaInfo {
        connection_id,
        schemas,
    })
}

pub async fn list_tables(
    state: &AppState,
    connection_id: Uuid,
    schema: String,
) -> Result<Vec<TableInfo>, AppError> {
    tracing::info!(target: "queryben::list-tables", %connection_id, %schema);
    let snapshot = state.registry.snapshot(connection_id)?;
    let input = reopen_input(state, snapshot).await?;
    let mut client = mssql::connect_for_connection(&input, connection_id).await?;
    let nodes = introspect_all(&mut client, Some(&schema)).await?;
    state.registry.mark_used(connection_id).ok();
    // Frontend wants just this schema's tables (not views/procs/fns).
    Ok(nodes
        .into_iter()
        .find(|n| n.name.eq_ignore_ascii_case(&schema))
        .map(|n| n.tables)
        .unwrap_or_default())
}

pub async fn get_table_metadata(
    state: &AppState,
    connection_id: Uuid,
    schema: String,
    name: String,
) -> Result<TableMetadata, AppError> {
    tracing::info!(
        target: "queryben::get-table-metadata",
        %connection_id,
        %schema,
        %name,
        "entry"
    );

    let snapshot = state.registry.snapshot(connection_id)?;
    let input = reopen_input(state, snapshot).await?;
    let mut client = mssql::connect_for_connection(&input, connection_id).await?;

    // --- columns ---
    let mut col_q = Query::new(COLUMNS_SQL);
    col_q.bind(schema.clone());
    col_q.bind(name.clone());
    let col_rows = col_q.query(&mut client).await?.into_first_result().await?;

    let mut columns: Vec<TableColumn> = Vec::with_capacity(col_rows.len());
    for row in col_rows {
        let col_name = row_get_string(&row, 0, "COLUMN_NAME")?;
        let data_type = row_get_string(&row, 1, "DATA_TYPE")?;
        let char_len = row.try_get::<i32, _>(2).map_err(AppError::from)?;
        // NUMERIC_PRECISION comes back as u8, NUMERIC_SCALE as i32 in tiberius.
        let numeric_precision = row
            .try_get::<u8, _>(3)
            .map_err(AppError::from)?
            .map(i32::from);
        let numeric_scale = row.try_get::<i32, _>(4).map_err(AppError::from)?;
        let is_nullable = row_get_i32(&row, 5)? != 0;
        // COLUMNPROPERTY returns int; NULL possible if the object_id lookup
        // fails (shouldn't for a real table, but stay defensive).
        let is_identity = row.try_get::<i32, _>(6).map_err(AppError::from)?.unwrap_or(0) != 0;
        let is_computed = row.try_get::<i32, _>(7).map_err(AppError::from)?.unwrap_or(0) != 0;
        let default_expression = row
            .try_get::<&str, _>(8)
            .map_err(AppError::from)?
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty());
        let ordinal = row_get_i32(&row, 9)?.max(0) as u32;

        columns.push(TableColumn {
            name: col_name,
            sql_type: format_sql_type(&data_type, char_len, numeric_precision, numeric_scale),
            is_nullable,
            is_identity,
            is_computed,
            default_expression,
            ordinal,
        });
    }

    // --- primary key ---
    let mut pk_q = Query::new(PK_SQL);
    pk_q.bind(schema.clone());
    pk_q.bind(name.clone());
    let pk_rows = pk_q.query(&mut client).await?.into_first_result().await?;
    let mut primary_key: Vec<String> = Vec::with_capacity(pk_rows.len());
    for row in pk_rows {
        primary_key.push(row_get_string(&row, 0, "name")?);
    }

    state.registry.mark_used(connection_id).ok();

    let is_editable = !primary_key.is_empty();
    tracing::info!(
        target: "queryben::get-table-metadata",
        %connection_id,
        %schema,
        %name,
        columns = columns.len(),
        pk_cols = primary_key.len(),
        is_editable,
        "done"
    );

    Ok(TableMetadata {
        schema,
        name,
        is_editable,
        primary_key,
        columns,
    })
}

// Renders "nvarchar(50)" / "decimal(18,4)" / "int" from the raw DATA_TYPE +
// length/precision/scale INFORMATION_SCHEMA hands back. Length -1 = MAX.
fn format_sql_type(
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

// Small typed accessors so we don't repeat the `NotFound` boilerplate.
//
// tiberius has a footgun: `try_get::<&str, _>(idx)` can succeed with an empty
// borrow when the row's `sysname`/nvarchar column comes back under a different
// `ColumnData` variant than the `&str` impl matches (varies by tiberius
// version and column collation). Column-name lookup sometimes works when
// index lookup silently misfires, and if both come up empty we reach into the
// raw `ColumnData` for the column and pull the value out under any string-
// shaped variant. Whichever path succeeds we log at info level so this class
// of bug is obvious next time it shows up.
fn row_get_string(row: &Row, idx: usize, col_name: &str) -> Result<String, AppError> {
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

// Best-effort string extraction from tiberius' `ColumnData`. Covers the common
// string-shaped variants; anything else falls through to `None`.
fn column_data_as_string(data: &ColumnData<'_>) -> Option<String> {
    match data {
        ColumnData::String(Some(cow)) => Some(cow.to_string()),
        ColumnData::Xml(Some(x)) => Some(x.to_string()),
        ColumnData::Guid(Some(g)) => Some(g.to_string()),
        _ => None,
    }
}

fn row_get_i64(row: &Row, idx: usize) -> Result<i64, AppError> {
    Ok(row
        .try_get::<i64, _>(idx)
        .map_err(AppError::from)?
        .unwrap_or(0))
}

fn row_get_i32(row: &Row, idx: usize) -> Result<i32, AppError> {
    Ok(row
        .try_get::<i32, _>(idx)
        .map_err(AppError::from)?
        .unwrap_or(0))
}
