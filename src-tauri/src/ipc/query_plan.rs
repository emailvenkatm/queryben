//! IPC entry for the query plan visualizer. Dispatches on the connection's
//! auth mode / driver kind to pick the right provider, reads the on-disk
//! capture-options file, and returns a parsed tree the frontend can render.

use std::fs;
use std::path::PathBuf;

use serde::Deserialize;
use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

use crate::core::connection::{ConnectionSnapshot, CreateConnectionInput};
use crate::core::query_plan::QueryPlan;
use crate::error::AppError;
use crate::adapters::{azure::oauth as azure_oauth, mssql};
use crate::adapters::query_plan_provider::{
    PlanCaptureOptions, QueryPlanProvider, SqlServerQueryPlanProvider,
};
use crate::state::AppState;

// Bearer scope for tiberius (mirrors commands::query). Duplicated here rather
// than punching a pub API through query.rs so the two command modules stay
// independently editable.
const SCOPE_SQLDB: &str = "https://database.windows.net/.default";

async fn reopen_input(
    state: &AppState,
    s: ConnectionSnapshot,
) -> Result<CreateConnectionInput, AppError> {
    let bearer = if s.connection.auth_mode.uses_aad_bearer() {
        let tenant = s.tenant_id.as_deref().ok_or_else(|| {
            AppError::AuthFailed("AAD connection missing tenant_id; reconnect to repair".into())
        })?;
        let client = s.client_id.as_deref().ok_or_else(|| {
            AppError::AuthFailed("AAD connection missing client_id; reconnect to repair".into())
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

const OPTIONS_FILE: &str = "queryplan.config.json";

#[derive(Debug, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct FileOptions {
    show_estimated_only: bool,
    warn_on_scan_rows_above: f64,
    warn_on_missing_index: bool,
}

impl Default for FileOptions {
    fn default() -> Self {
        Self {
            show_estimated_only: true,
            warn_on_scan_rows_above: 100_000.0,
            warn_on_missing_index: true,
        }
    }
}

impl From<FileOptions> for PlanCaptureOptions {
    fn from(f: FileOptions) -> Self {
        Self {
            show_estimated_only: f.show_estimated_only,
            warn_on_scan_rows_above: f.warn_on_scan_rows_above,
            warn_on_missing_index: f.warn_on_missing_index,
        }
    }
}

fn load_options(app: &AppHandle) -> PlanCaptureOptions {
    let Ok(dir) = app.path().app_data_dir() else {
        return PlanCaptureOptions::default();
    };
    let path: PathBuf = dir.join(OPTIONS_FILE);
    let Ok(raw) = fs::read_to_string(&path) else {
        return PlanCaptureOptions::default();
    };
    match serde_json::from_str::<FileOptions>(&raw) {
        Ok(f) => f.into(),
        Err(err) => {
            tracing::warn!(
                target: "queryben::query-plan",
                path = %path.display(),
                "queryplan.config.json parse failed, using defaults: {err}"
            );
            PlanCaptureOptions::default()
        }
    }
}

#[tauri::command]
#[specta::specta]
pub async fn get_query_plan(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: Uuid,
    sql: String,
) -> Result<QueryPlan, AppError> {
    tracing::info!(
        target: "queryben::query-plan",
        %connection_id,
        sql_len = sql.len(),
        "entry"
    );

    let opts = load_options(&app);

    let snapshot = state.registry.snapshot(connection_id)?;
    let input = reopen_input(&state, snapshot).await?;
    let mut client = mssql::connect_for_connection(&input, connection_id).await?;

    // v1 = SQL Server only. MySQL / Postgres providers exist as unimplemented
    // impls; the match here is where they slot in once their drivers land.
    let provider: Box<dyn QueryPlanProvider> = Box::new(SqlServerQueryPlanProvider);
    let plan = provider.capture_plan(&mut client, &sql, &opts).await?;

    state.registry.mark_used(connection_id).ok();
    tracing::info!(
        target: "queryben::query-plan",
        %connection_id,
        is_actual = plan.is_actual,
        warnings = plan.warnings.len(),
        "done"
    );
    Ok(plan)
}
