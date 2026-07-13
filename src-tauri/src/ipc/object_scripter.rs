//! Object-scripter IPC. Single command `script_object`.
//!
//! Right-click any object → "Script as →" submenu → this command runs on the
//! backend, resolves credentials, opens a tiberius session, and returns a SQL
//! string the frontend drops into a fresh query tab.
//!
//! `SELECT TOP` and `DROP` are pure renderers (no DB call). Everything else
//! (`CREATE`, `ALTER`, `INSERT template`) opens a live connection because it
//! needs sys.* introspection or OBJECT_DEFINITION.

use std::path::PathBuf;

use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

use crate::core::connection::{ConnectionSnapshot, CreateConnectionInput};
use crate::core::object_script::{ObjectKind, ScriptAction, SchemaObjectRef};
use crate::error::AppError;
use crate::adapters::object_scripter::{
    render_drop_and_create, render_select_top, ObjectScripter, ScripterOptions,
    SqlServerObjectScripter,
};
use crate::adapters::{azure::oauth as azure_oauth, mssql};
use crate::state::AppState;

// Mirrors commands::query::SCOPE_SQLDB. Duplicated per the convention set by
// other command modules (table_designer, schema_compare) to keep this file
// standalone.
const SCOPE_SQLDB: &str = "https://database.windows.net/.default";

// SELECT TOP N default. Users on ADS get 1000; SSMS defaults to 1000 too. We
// stay at 100 for parity with the existing double-click browse flow — the row
// count is what the user is asking to iterate on, not the schema.
const SELECT_TOP_N: u32 = 100;

fn app_data_dir(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from("."))
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
pub async fn script_object(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: Uuid,
    kind: ObjectKind,
    schema: String,
    name: String,
    // Only meaningful for Index (parent table). Frontend passes null otherwise.
    table: Option<String>,
    action: ScriptAction,
) -> Result<String, AppError> {
    tracing::info!(
        target: "queryben::scripter",
        %connection_id,
        ?kind,
        %schema,
        %name,
        ?action,
        "entry"
    );

    let opts = ScripterOptions::load(&app_data_dir(&app));
    let obj = SchemaObjectRef {
        kind,
        schema,
        name,
        table,
    };
    let scripter = SqlServerObjectScripter::new(opts.clone());

    // Pure paths never open a client.
    match action {
        ScriptAction::Drop => return Ok(scripter.script_drop(&obj)),
        ScriptAction::SelectTop => {
            if !matches!(kind, ObjectKind::Table | ObjectKind::View) {
                return Err(AppError::internal(
                    "SELECT TOP is only supported for tables and views",
                ));
            }
            return Ok(render_select_top(&obj, &opts, SELECT_TOP_N));
        }
        _ => {}
    }

    // Live-DB paths.
    let snapshot = state.registry.snapshot(connection_id)?;
    let input = reopen_input(&state, snapshot).await?;
    let mut client = mssql::connect_for_connection(&input, connection_id).await?;

    let result = match action {
        ScriptAction::Create => scripter.script_create(&mut client, &obj).await?,
        ScriptAction::Alter => scripter.script_alter(&mut client, &obj).await?,
        ScriptAction::DropAndCreate => {
            // Compose: drop is pure; create requires the same client. Fetch
            // create first so a broken introspection doesn't leak a lone DROP
            // into the user's tab.
            let create = scripter.script_create(&mut client, &obj).await?;
            let drop = scripter.script_drop(&obj);
            render_drop_and_create(&drop, &create)
        }
        ScriptAction::InsertTemplate => {
            scripter.script_insert_template(&mut client, &obj).await?
        }
        ScriptAction::Drop | ScriptAction::SelectTop => {
            // Already handled above; unreachable but exhaustive-match-safe.
            unreachable!()
        }
    };

    state.registry.mark_used(connection_id).ok();
    Ok(result)
}
