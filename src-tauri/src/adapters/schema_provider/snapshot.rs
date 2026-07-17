//! Full-schema snapshot for schema-compare. Runs five introspection queries,
//! stitches column + index rows into their owning tables, and emits a sorted
//! `Vec<SchemaObject>` the diff engine consumes.

use tiberius::Row;

use crate::adapters::mssql::MssqlClient;
use crate::core::schema_diff::{ColumnSpec, IndexSpec, ObjectKind, SchemaObject};
use crate::error::AppError;

use super::sql::{
    fetch, format_sql_type, str_col, COLUMNS_SQL, FNS_SQL, INDEXES_SQL, PROCS_SQL, TABLES_SQL,
    VIEWS_SQL,
};

pub(super) async fn snapshot_all(
    client: &mut MssqlClient,
) -> Result<Vec<SchemaObject>, AppError> {
    let table_rows = fetch(client, TABLES_SQL).await?;
    let view_rows = fetch(client, VIEWS_SQL).await?;
    let proc_rows = fetch(client, PROCS_SQL).await?;
    let fn_rows = fetch(client, FNS_SQL).await?;
    let col_rows = fetch(client, COLUMNS_SQL).await?;
    let idx_rows = fetch(client, INDEXES_SQL).await?;

    let columns_by_table = collect_columns(col_rows)?;
    let (indexes_by_table, standalone_indexes) = collect_indexes(idx_rows)?;

    let mut out: Vec<SchemaObject> = Vec::new();
    let mut columns_by_table = columns_by_table;
    let mut indexes_by_table = indexes_by_table;

    for row in table_rows {
        let schema = str_col(&row, 0, "schema_name")?;
        let name = str_col(&row, 1, "table_name")?;
        let qname = format!("{schema}.{name}");
        let cols = columns_by_table
            .remove(&(schema.clone(), name.clone()))
            .unwrap_or_default();
        let mut idxs = indexes_by_table
            .remove(&(schema.clone(), name.clone()))
            .unwrap_or_default();
        idxs.sort_by(|a, b| a.name.cmp(&b.name));
        out.push(SchemaObject {
            kind: ObjectKind::Table,
            schema,
            name,
            qualified_name: qname,
            columns: cols,
            indexes: idxs,
            body: None,
        });
    }

    out.extend(routine_rows_into_objects(view_rows, ObjectKind::View, "view_name")?);
    out.extend(routine_rows_into_objects(proc_rows, ObjectKind::Procedure, "proc_name")?);
    out.extend(routine_rows_into_objects(fn_rows, ObjectKind::Function, "fn_name")?);
    out.extend(standalone_indexes);

    out.sort_by(|a, b| {
        a.kind
            .as_config_str()
            .cmp(b.kind.as_config_str())
            .then_with(|| a.qualified_name.cmp(&b.qualified_name))
    });

    Ok(out)
}

type ColumnsByTable = std::collections::BTreeMap<(String, String), Vec<ColumnSpec>>;
type IndexesByTable = std::collections::BTreeMap<(String, String), Vec<IndexSpec>>;

fn collect_columns(rows: Vec<Row>) -> Result<ColumnsByTable, AppError> {
    let mut out: ColumnsByTable = ColumnsByTable::new();
    for row in rows {
        let schema = str_col(&row, 0, "schema_name")?;
        let table = str_col(&row, 1, "table_name")?;
        let name = str_col(&row, 2, "column_name")?;
        let data_type = str_col(&row, 3, "data_type")?;
        let max_length = row
            .try_get::<i16, _>(4)
            .map_err(AppError::from)?
            .unwrap_or(0) as i32;
        let precision = row.try_get::<u8, _>(5).map_err(AppError::from)?.unwrap_or(0) as i32;
        let scale = row.try_get::<u8, _>(6).map_err(AppError::from)?.unwrap_or(0) as i32;
        let is_nullable = row
            .try_get::<bool, _>(7)
            .map_err(AppError::from)?
            .unwrap_or(true);
        let is_identity = row
            .try_get::<bool, _>(8)
            .map_err(AppError::from)?
            .unwrap_or(false);
        let is_computed = row
            .try_get::<bool, _>(9)
            .map_err(AppError::from)?
            .unwrap_or(false);
        let default_expression = row
            .try_get::<&str, _>(10)
            .map_err(AppError::from)?
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty());
        let ordinal = row.try_get::<i32, _>(11).map_err(AppError::from)?.unwrap_or(0).max(0) as u32;

        out.entry((schema, table)).or_default().push(ColumnSpec {
            name,
            sql_type: format_sql_type(&data_type, max_length, precision, scale),
            is_nullable,
            is_identity,
            is_computed,
            default_expression,
            ordinal,
        });
    }
    Ok(out)
}

fn collect_indexes(
    rows: Vec<Row>,
) -> Result<(IndexesByTable, Vec<SchemaObject>), AppError> {
    let mut by_table: IndexesByTable = IndexesByTable::new();
    let mut standalone: Vec<SchemaObject> = Vec::new();
    for row in rows {
        let schema = str_col(&row, 0, "schema_name")?;
        let table = str_col(&row, 1, "table_name")?;
        let name = str_col(&row, 2, "index_name")?;
        let is_unique = row
            .try_get::<bool, _>(3)
            .map_err(AppError::from)?
            .unwrap_or(false);
        let is_primary_key = row
            .try_get::<bool, _>(4)
            .map_err(AppError::from)?
            .unwrap_or(false);
        let key_columns_str = row
            .try_get::<&str, _>(5)
            .map_err(AppError::from)?
            .unwrap_or("")
            .to_string();
        let columns: Vec<String> = key_columns_str
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| s.trim().to_string())
            .collect();

        let idx = IndexSpec {
            name: name.clone(),
            is_unique,
            is_primary_key,
            columns: columns.clone(),
        };
        by_table
            .entry((schema.clone(), table.clone()))
            .or_default()
            .push(idx.clone());

        // Also surface non-PK indexes as standalone Index objects so the
        // include-object-kinds filter can toggle them separately.
        if !is_primary_key {
            let qname = format!("{schema}.{table}.{name}");
            standalone.push(SchemaObject {
                kind: ObjectKind::Index,
                schema: schema.clone(),
                name: name.clone(),
                qualified_name: qname,
                columns: Vec::new(),
                indexes: vec![idx],
                body: None,
            });
        }
    }
    Ok((by_table, standalone))
}

fn routine_rows_into_objects(
    rows: Vec<Row>,
    kind: ObjectKind,
    name_label: &'static str,
) -> Result<Vec<SchemaObject>, AppError> {
    let mut out: Vec<SchemaObject> = Vec::with_capacity(rows.len());
    for row in rows {
        let schema = str_col(&row, 0, "schema_name")?;
        let name = str_col(&row, 1, name_label)?;
        let body = row
            .try_get::<&str, _>(2)
            .map_err(AppError::from)?
            .map(|s| s.to_string());
        let qname = format!("{schema}.{name}");
        out.push(SchemaObject {
            kind: kind.clone(),
            schema,
            name,
            qualified_name: qname,
            columns: Vec::new(),
            indexes: Vec::new(),
            body,
        });
    }
    Ok(out)
}
