//! Schema-compare IPC. Three commands:
//!
//! - `schema_snapshot(connection_id)`  -> pulls the current shape from the DB
//! - `schema_diff(source, target)`     -> pure diff, no IO
//! - `schema_diff_ddl(diff)`           -> generates migration SQL
//!
//! The diff and DDL commands accept round-tripped snapshots so a stale
//! snapshot doesn't force another introspection query on every user click.

use std::time::Instant;

use chrono::Utc;
use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

use crate::core::connection::{ConnectionSnapshot, CreateConnectionInput};
use crate::core::schema_diff::{
    DdlStatement, SchemaCompareOptions, SchemaDiff, SchemaSnapshot,
};
use crate::error::AppError;
use crate::adapters::schema_provider::{compute_diff, SchemaProvider, SqlServerSchemaProvider};
use crate::adapters::{azure::oauth as azure_oauth, mssql};
use crate::state::AppState;

// Matches the tiberius scope used in commands/query.rs. Duplicated here so the
// schema-compare command doesn't reach across module boundaries for a const.
const SCOPE_SQLDB: &str = "https://database.windows.net/.default";

// File name for the persisted options. Sits next to theme.json / connections.json
// in the app-data dir. The task doc references
// `~/Library/Application Support/QueryBen/schema-compare.config.json`; on macOS
// Tauri resolves app_data_dir to
// `~/Library/Application Support/com.queryben.app/`, which is where every other
// QueryBen config already lives.
const OPTIONS_FILE: &str = "schema-compare.config.json";

fn load_options(app: &AppHandle) -> SchemaCompareOptions {
    let dir = match app.path().app_data_dir() {
        Ok(d) => d,
        Err(_) => return SchemaCompareOptions::default(),
    };
    let raw = match std::fs::read(dir.join(OPTIONS_FILE)) {
        Ok(b) => b,
        Err(_) => return SchemaCompareOptions::default(),
    };
    match serde_json::from_slice::<SchemaCompareOptions>(&raw) {
        Ok(o) => o,
        Err(err) => {
            tracing::warn!(
                target: "queryben::schema-compare",
                "schema-compare.config.json parse failed, using defaults: {err}"
            );
            SchemaCompareOptions::default()
        }
    }
}

async fn reopen_input(
    state: &AppState,
    s: ConnectionSnapshot,
) -> Result<CreateConnectionInput, AppError> {
    let bearer = if s.connection.auth_mode.uses_aad_bearer() {
        let tenant = s.tenant_id.as_deref().ok_or_else(|| {
            AppError::AuthFailed(
                "AAD connection missing tenant_id; reconnect to repair".into(),
            )
        })?;
        let client = s.client_id.as_deref().ok_or_else(|| {
            AppError::AuthFailed(
                "AAD connection missing client_id; reconnect to repair".into(),
            )
        })?;
        Some(
            azure_oauth::acquire_token(
                &state.azure_tokens,
                tenant,
                client,
                SCOPE_SQLDB,
                s.connection.account_id.as_deref(),
            )
            .await?,
        )
    } else {
        None
    };
    let c = s.connection;
    Ok(CreateConnectionInput {
        name: c.name,
        server: c.server,
        database: c.database,
        port: c.port,
        username: c.username,
        password: s.password,
        auth_mode: c.auth_mode,
        trust_server_certificate: s.trust_server_certificate,
        aad_bearer: bearer,
        nickname: c.nickname,
        color: c.color,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn schema_snapshot(
    state: State<'_, AppState>,
    connection_id: Uuid,
) -> Result<SchemaSnapshot, AppError> {
    tracing::info!(target: "queryben::schema-compare::snapshot", %connection_id, "entry");
    let started = Instant::now();
    let snapshot = state.registry.snapshot(connection_id)?;
    let label = format!(
        "{} · {}",
        snapshot.connection.name, snapshot.connection.database
    );
    let input = reopen_input(&state, snapshot).await?;
    let mut client = mssql::connect_for_connection(&input, connection_id).await?;

    // v1 = MSSQL only. When the connection struct grows an `engine` field the
    // dispatch swaps in here.
    let provider = SqlServerSchemaProvider;
    let objects = provider.snapshot(&mut client).await?;

    state.registry.mark_used(connection_id).ok();
    tracing::info!(
        target: "queryben::schema-compare::snapshot",
        %connection_id,
        object_count = objects.len(),
        duration_ms = started.elapsed().as_millis() as u64,
        "done"
    );

    Ok(SchemaSnapshot {
        label,
        captured_at: Utc::now().to_rfc3339(),
        connection_id: connection_id.to_string(),
        engine: "mssql".into(),
        objects,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn schema_diff(
    app: AppHandle,
    source: SchemaSnapshot,
    target: SchemaSnapshot,
) -> Result<SchemaDiff, AppError> {
    let options = load_options(&app);
    tracing::info!(
        target: "queryben::schema-compare::diff",
        source_objects = source.objects.len(),
        target_objects = target.objects.len(),
        "entry"
    );
    Ok(compute_diff(&source, &target, &options))
}

#[tauri::command]
#[specta::specta]
pub async fn schema_diff_ddl(diff: SchemaDiff) -> Result<Vec<DdlStatement>, AppError> {
    tracing::info!(
        target: "queryben::schema-compare::ddl",
        added = diff.added.len(),
        dropped = diff.dropped.len(),
        changed = diff.changed.len(),
        "entry"
    );
    let provider = SqlServerSchemaProvider;
    Ok(provider.generate_ddl(&diff))
}
