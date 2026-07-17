//! Per-table drill-in: column list + primary key. Editable when at least
//! one PK column exists.

use tiberius::Query;
use uuid::Uuid;

use crate::adapters::mssql;
use crate::core::schema::{TableColumn, TableMetadata};
use crate::error::AppError;
use crate::state::AppState;

use super::rows::{row_get_i32, row_get_string};
use super::sql::{format_sql_type, COLUMNS_SQL, PK_SQL};
use crate::app::session::reopen_input;

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

    let columns = load_columns(&mut client, &schema, &name).await?;
    let primary_key = load_primary_key(&mut client, &schema, &name).await?;

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

async fn load_columns(
    client: &mut mssql::MssqlClient,
    schema: &str,
    name: &str,
) -> Result<Vec<TableColumn>, AppError> {
    let mut q = Query::new(COLUMNS_SQL);
    q.bind(schema.to_string());
    q.bind(name.to_string());
    let rows = q.query(client).await?.into_first_result().await?;

    let mut out: Vec<TableColumn> = Vec::with_capacity(rows.len());
    for row in rows {
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

        out.push(TableColumn {
            name: col_name,
            sql_type: format_sql_type(&data_type, char_len, numeric_precision, numeric_scale),
            is_nullable,
            is_identity,
            is_computed,
            default_expression,
            ordinal,
        });
    }
    Ok(out)
}

async fn load_primary_key(
    client: &mut mssql::MssqlClient,
    schema: &str,
    name: &str,
) -> Result<Vec<String>, AppError> {
    let mut q = Query::new(PK_SQL);
    q.bind(schema.to_string());
    q.bind(name.to_string());
    let rows = q.query(client).await?.into_first_result().await?;
    let mut out: Vec<String> = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(row_get_string(&row, 0, "name")?);
    }
    Ok(out)
}
