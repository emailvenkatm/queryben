//! Full-schema pull for the object explorer: schemas, tables + views, procs,
//! functions, per-table row/column estimates. All five queries run
//! sequentially — see the SQL module docs for why batching bit us.

use std::collections::BTreeMap;

use uuid::Uuid;

use crate::adapters::mssql;
use crate::core::schema::{RoutineInfo, SchemaInfo, SchemaNode, TableInfo};
use crate::error::AppError;
use crate::state::AppState;

use super::rows::{row_get_i32, row_get_i64, row_get_string};
use super::sql::{
    is_system_schema, run_query, FNS_SQL_ALL, FNS_SQL_ONE, OBJECTS_SQL_ALL, OBJECTS_SQL_ONE,
    PROCS_SQL_ALL, PROCS_SQL_ONE, SCHEMAS_SQL_ALL, SCHEMAS_SQL_ONE, STATS_SQL_ALL, STATS_SQL_ONE,
};
use crate::app::session::reopen_input;

// Pulled into a helper so both `get_schema` and `list_tables` share the same
// INFORMATION_SCHEMA / sys.* pass.
pub(super) async fn introspect_all(
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
        nodes.entry(name.clone()).or_insert_with(|| empty_node(&name));
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
        let node = nodes.entry(schema.clone()).or_insert_with(|| empty_node(&schema));
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
        let node = nodes.entry(schema.clone()).or_insert_with(|| empty_node(&schema));
        node.procedures.push(RoutineInfo { schema, name });
    }

    for row in fn_rows {
        let schema = row_get_string(&row, 0, "schema_name")?;
        let name = row_get_string(&row, 1, "fn_name")?;
        tracing::info!(target: "queryben::introspect", loop_ = "fns", %schema, %name, "row");
        if is_system_schema(&schema) {
            continue;
        }
        let node = nodes.entry(schema.clone()).or_insert_with(|| empty_node(&schema));
        node.functions.push(RoutineInfo { schema, name });
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

fn empty_node(name: &str) -> SchemaNode {
    SchemaNode {
        name: name.to_string(),
        tables: Vec::new(),
        views: Vec::new(),
        procedures: Vec::new(),
        functions: Vec::new(),
    }
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
