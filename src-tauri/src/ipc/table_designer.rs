//! Table Designer IPC.
//!
//! Three commands:
//! - `load_table_design(connection_id, schema, name)` — pulls the current
//!   shape from the DB for the editor to seed from.
//! - `generate_table_ddl(connection_id, current, next)` — pure diff, no IO.
//!   `current = None` on the new-table flow so the provider emits a single
//!   CREATE.
//! - `apply_table_ddl(connection_id, statements)` — wraps the reviewed
//!   statements in a transaction via `execute_transaction`. Destructive; the
//!   user is expected to eyeball the preview first.

use tauri::State;
use uuid::Uuid;

use crate::core::connection::{ConnectionSnapshot, CreateConnectionInput};
use crate::core::schema::TransactionResult;
use crate::core::table_design::{DdlStatement, TableDesign};
use crate::error::AppError;
use crate::adapters::table_designer_provider::{
    SqlServerTableDesignerProvider, TableDesignerProvider,
};
use crate::adapters::{azure_oauth, mssql};
use crate::state::AppState;

// Mirrors commands::query::SCOPE_SQLDB. Duplicated here so the designer path
// doesn't reach across module boundaries for a const.
const SCOPE_SQLDB: &str = "https://database.windows.net/.default";

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
pub async fn load_table_design(
    state: State<'_, AppState>,
    connection_id: Uuid,
    schema: String,
    name: String,
) -> Result<TableDesign, AppError> {
    tracing::info!(
        target: "queryben::table-designer::load",
        %connection_id, %schema, %name, "entry"
    );
    let snapshot = state.registry.snapshot(connection_id)?;
    let input = reopen_input(&state, snapshot).await?;
    let mut client = mssql::connect_for_connection(&input, connection_id).await?;
    let provider = SqlServerTableDesignerProvider;
    let design = provider.load(&mut client, &schema, &name).await?;
    state.registry.mark_used(connection_id).ok();
    Ok(design)
}

#[tauri::command]
#[specta::specta]
pub async fn generate_table_ddl(
    _state: State<'_, AppState>,
    _connection_id: Uuid,
    current: Option<TableDesign>,
    next: TableDesign,
) -> Result<Vec<DdlStatement>, AppError> {
    tracing::info!(
        target: "queryben::table-designer::ddl",
        has_current = current.is_some(),
        table = %next.name,
        cols = next.columns.len(),
        "entry"
    );
    let provider = SqlServerTableDesignerProvider;
    Ok(provider.generate_ddl(current.as_ref(), &next))
}

#[tauri::command]
#[specta::specta]
pub async fn apply_table_ddl(
    state: State<'_, AppState>,
    connection_id: Uuid,
    statements: Vec<String>,
) -> Result<TransactionResult, AppError> {
    tracing::info!(
        target: "queryben::table-designer::apply",
        %connection_id,
        statement_count = statements.len(),
        "entry"
    );
    // Delegate to the shared transaction runner in commands::query. It handles
    // BEGIN / ROLLBACK / COMMIT identically to browse-mode edits, so a failing
    // ALTER unwinds cleanly instead of leaving a half-migrated table.
    crate::ipc::query::execute_transaction(state, connection_id, statements).await
}
