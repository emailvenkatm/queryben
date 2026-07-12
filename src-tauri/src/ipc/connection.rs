//! Connection CRUD. Passwords go in but never come back out.

use std::time::Instant;

use chrono::Utc;
use tauri::State;
use uuid::Uuid;

use crate::core::connection::{
    normalize_nickname, Connection, ConnectionEntry, CreateConnectionInput, TestResult,
    UpdateConnectionInput,
};
use crate::error::AppError;
use crate::adapters::mssql;
use crate::state::AppState;

#[tauri::command]
#[specta::specta]
pub async fn create_connection(
    state: State<'_, AppState>,
    input: CreateConnectionInput,
) -> Result<Connection, AppError> {
    let nickname = normalize_nickname(input.nickname.clone())?;
    let connection = Connection {
        id: Uuid::new_v4(),
        name: input.name.clone(),
        server: input.server.clone(),
        database: input.database.clone(),
        port: input.port,
        username: input.username.clone(),
        auth_mode: input.auth_mode.clone(),
        created_at: Utc::now(),
        last_used: None,
        account_id: None,
        nickname,
        color: input.color,
    };
    let entry = ConnectionEntry {
        connection,
        password: input.password.clone(),
        trust_server_certificate: input.trust_server_certificate,
        // SQL-auth path doesn't need AAD context.
        tenant_id: None,
        client_id: None,
        // Unknown until an AAD sign-in resolves it; SQL-auth connections
        // can't call ARM at all, so leaving None is correct forever.
        server_arm_id: None,
    };
    state.registry.insert(entry)
}

#[tauri::command]
#[specta::specta]
pub async fn list_connections(
    state: State<'_, AppState>,
) -> Result<Vec<Connection>, AppError> {
    state.registry.list()
}

#[tauri::command]
#[specta::specta]
pub async fn delete_connection(
    state: State<'_, AppState>,
    id: Uuid,
) -> Result<(), AppError> {
    state.registry.remove(id)
}

#[tauri::command]
#[specta::specta]
pub async fn update_connection(
    state: State<'_, AppState>,
    input: UpdateConnectionInput,
) -> Result<Connection, AppError> {
    state.registry.update_labels(input)
}

#[tauri::command]
#[specta::specta]
pub async fn test_connection(
    _state: State<'_, AppState>,
    input: CreateConnectionInput,
) -> Result<TestResult, AppError> {
    let start = Instant::now();
    match mssql::connect(&input).await {
        Ok(_client) => Ok(TestResult {
            ok: true,
            message: None,
            latency_ms: Some(start.elapsed().as_millis() as u32),
        }),
        Err(err) => Ok(TestResult {
            ok: false,
            message: Some(err.to_string()),
            latency_ms: None,
        }),
    }
}
