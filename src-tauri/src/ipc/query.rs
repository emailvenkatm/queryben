//! Query execution + schema introspection commands. The bodies live in
//! `app::execute_query`, `app::execute_transaction`, and `app::introspect` —
//! this file only marshals args + delegates.

use tauri::State;
use uuid::Uuid;

use crate::app;
use crate::core::query::QueryOutcome;
use crate::core::schema::{SchemaInfo, TableInfo, TableMetadata, TransactionResult};
use crate::error::AppError;
use crate::state::AppState;

#[tauri::command]
#[specta::specta]
pub async fn execute_query(
    state: State<'_, AppState>,
    connection_id: Uuid,
    sql: String,
) -> Result<QueryOutcome, AppError> {
    app::execute_query::run(&state, connection_id, sql).await
}

// TODO: real cancel needs per-query handles + a tiberius attention-token
// escape hatch. For now this errors so the UI shows an honest "not wired yet".
#[tauri::command]
#[specta::specta]
pub async fn cancel_query(
    _state: State<'_, AppState>,
    query_id: Uuid,
) -> Result<(), AppError> {
    tracing::warn!(target: "queryben::cancel-query", %query_id, "cancel not wired");
    Err(AppError::NotImplemented(
        "query cancellation not wired yet".into(),
    ))
}

#[tauri::command]
#[specta::specta]
pub async fn get_schema(
    state: State<'_, AppState>,
    connection_id: Uuid,
) -> Result<SchemaInfo, AppError> {
    app::introspect::get_schema(&state, connection_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_tables(
    state: State<'_, AppState>,
    connection_id: Uuid,
    schema: String,
) -> Result<Vec<TableInfo>, AppError> {
    app::introspect::list_tables(&state, connection_id, schema).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_table_metadata(
    state: State<'_, AppState>,
    connection_id: Uuid,
    schema: String,
    name: String,
) -> Result<TableMetadata, AppError> {
    app::introspect::get_table_metadata(&state, connection_id, schema, name).await
}

#[tauri::command]
#[specta::specta]
pub async fn execute_transaction(
    state: State<'_, AppState>,
    connection_id: Uuid,
    statements: Vec<String>,
) -> Result<TransactionResult, AppError> {
    app::execute_transaction::run(&state, connection_id, statements).await
}
