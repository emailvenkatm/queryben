//! Load a table's current shape from the DB into a `TableDesign` the UI can
//! edit. Four introspection queries: columns, primary key, non-PK indexes,
//! foreign keys.

use tiberius::Query;

use crate::adapters::mssql::MssqlClient;
use crate::core::table_design::{DesignColumn, DesignForeignKey, DesignIndex, TableDesign};
use crate::error::AppError;

use super::sql::{
    format_sql_type, normalize_ref_action, split_csv, str_col, COLUMNS_SQL, FK_SQL, IDX_SQL, PK_SQL,
};

pub(super) async fn load(
    client: &mut MssqlClient,
    schema: &str,
    name: &str,
) -> Result<TableDesign, AppError> {
    let columns = load_columns(client, schema, name).await?;
    let (primary_key, pk_name) = load_primary_key(client, schema, name).await?;
    let indexes = load_indexes(client, schema, name).await?;
    let foreign_keys = load_foreign_keys(client, schema, name).await?;

    Ok(TableDesign {
        schema: schema.to_string(),
        name: name.to_string(),
        columns,
        primary_key,
        pk_name,
        indexes,
        foreign_keys,
    })
}

async fn load_columns(
    client: &mut MssqlClient,
    schema: &str,
    name: &str,
) -> Result<Vec<DesignColumn>, AppError> {
    let mut q = Query::new(COLUMNS_SQL);
    q.bind(schema.to_string());
    q.bind(name.to_string());
    let rows = q.query(client).await?.into_first_result().await?;

    let mut out: Vec<DesignColumn> = Vec::with_capacity(rows.len());
    for row in rows {
        let col_name = str_col(&row, 0, "column_name")?;
        let data_type = str_col(&row, 1, "data_type")?;
        let max_length = row
            .try_get::<i16, _>(2)
            .map_err(AppError::from)?
            .unwrap_or(0) as i32;
        let precision = row.try_get::<u8, _>(3).map_err(AppError::from)?.unwrap_or(0) as i32;
        let scale = row.try_get::<u8, _>(4).map_err(AppError::from)?.unwrap_or(0) as i32;
        let is_nullable = row
            .try_get::<bool, _>(5)
            .map_err(AppError::from)?
            .unwrap_or(true);
        let is_identity = row
            .try_get::<bool, _>(6)
            .map_err(AppError::from)?
            .unwrap_or(false);
        let is_computed = row
            .try_get::<bool, _>(7)
            .map_err(AppError::from)?
            .unwrap_or(false);
        let computed_expression = row
            .try_get::<&str, _>(8)
            .map_err(AppError::from)?
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let default_expression = row
            .try_get::<&str, _>(9)
            .map_err(AppError::from)?
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let ordinal = row
            .try_get::<i32, _>(10)
            .map_err(AppError::from)?
            .unwrap_or(0)
            .max(0) as u32;

        out.push(DesignColumn {
            name: col_name,
            sql_type: format_sql_type(&data_type, max_length, precision, scale),
            is_nullable,
            is_identity,
            is_computed,
            computed_expression,
            default_expression,
            ordinal,
        });
    }
    Ok(out)
}

async fn load_primary_key(
    client: &mut MssqlClient,
    schema: &str,
    name: &str,
) -> Result<(Vec<String>, Option<String>), AppError> {
    let mut q = Query::new(PK_SQL);
    q.bind(schema.to_string());
    q.bind(name.to_string());
    let rows = q.query(client).await?.into_first_result().await?;

    let mut primary_key: Vec<String> = Vec::new();
    let mut pk_name: Option<String> = None;
    for row in rows {
        if pk_name.is_none() {
            pk_name = row
                .try_get::<&str, _>(0)
                .map_err(AppError::from)?
                .map(|s| s.to_string());
        }
        let col = str_col(&row, 1, "column_name")?;
        primary_key.push(col);
    }
    Ok((primary_key, pk_name))
}

async fn load_indexes(
    client: &mut MssqlClient,
    schema: &str,
    name: &str,
) -> Result<Vec<DesignIndex>, AppError> {
    let mut q = Query::new(IDX_SQL);
    q.bind(schema.to_string());
    q.bind(name.to_string());
    let rows = q.query(client).await?.into_first_result().await?;

    let mut out: Vec<DesignIndex> = Vec::with_capacity(rows.len());
    for row in rows {
        let idx_name = str_col(&row, 0, "index_name")?;
        let is_unique = row
            .try_get::<bool, _>(1)
            .map_err(AppError::from)?
            .unwrap_or(false);
        let key_columns_str = row
            .try_get::<&str, _>(2)
            .map_err(AppError::from)?
            .unwrap_or("")
            .to_string();
        let cols: Vec<String> = key_columns_str
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| s.trim().to_string())
            .collect();
        out.push(DesignIndex {
            name: idx_name,
            is_unique,
            columns: cols,
        });
    }
    Ok(out)
}

async fn load_foreign_keys(
    client: &mut MssqlClient,
    schema: &str,
    name: &str,
) -> Result<Vec<DesignForeignKey>, AppError> {
    let mut q = Query::new(FK_SQL);
    q.bind(schema.to_string());
    q.bind(name.to_string());
    let rows = q.query(client).await?.into_first_result().await?;

    let mut out: Vec<DesignForeignKey> = Vec::with_capacity(rows.len());
    for row in rows {
        let fk_name = str_col(&row, 0, "fk_name")?;
        let ref_schema = str_col(&row, 1, "referenced_schema")?;
        let ref_table = str_col(&row, 2, "referenced_table")?;
        let parent_columns_str = row
            .try_get::<&str, _>(3)
            .map_err(AppError::from)?
            .unwrap_or("")
            .to_string();
        let referenced_columns_str = row
            .try_get::<&str, _>(4)
            .map_err(AppError::from)?
            .unwrap_or("")
            .to_string();
        let on_delete = row
            .try_get::<&str, _>(5)
            .map_err(AppError::from)?
            .map(|s| normalize_ref_action(s));
        let on_update = row
            .try_get::<&str, _>(6)
            .map_err(AppError::from)?
            .map(|s| normalize_ref_action(s));
        out.push(DesignForeignKey {
            name: fk_name,
            columns: split_csv(&parent_columns_str),
            referenced_schema: ref_schema,
            referenced_table: ref_table,
            referenced_columns: split_csv(&referenced_columns_str),
            on_delete,
            on_update,
        });
    }
    Ok(out)
}
