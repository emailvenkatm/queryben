//! Azure sign-in, browse, and AAD-token connect. Bearers stay Rust-side.

use chrono::Utc;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

use crate::core::azure::{AzureConnectInput, AzureSqlDatabase, AzureSqlServer, AzureSubscription};
use crate::core::connection::{
    AuthMode, Connection, ConnectionEntry, CreateConnectionInput,
};
use crate::error::AppError;
use crate::adapters::{azure_oauth, azure_rest, mssql};
use crate::adapters::azure_oauth::AzureAccount;
use crate::state::AppState;

// Firewall-rule branch also emits progress in the same shape as the mssql
// retry loop. Keeps the UI's listener single-purpose.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FirewallProgress {
    stage: &'static str,
    ip: String,
}

// Passed to /token as `<resource>/.default`. Azure mints an access_token
// whose `aud` matches, which is what tiberius and ARM want.
const SCOPE_MANAGEMENT: &str = "https://management.azure.com/.default";
const SCOPE_SQLDB: &str = "https://database.windows.net/.default";

// ---- sign-in / sign-out / account -------------------------------------------

#[tauri::command]
#[specta::specta]
pub async fn azure_sign_in(
    app: AppHandle,
    state: State<'_, AppState>,
    tenant_id: String,
    client_id: String,
) -> Result<AzureAccount, AppError> {
    tracing::info!(target: "queryben::azure::sign-in", %tenant_id, "starting");
    azure_oauth::sign_in(&app, &state.azure_tokens, &tenant_id, &client_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn azure_sign_out(state: State<'_, AppState>) -> Result<(), AppError> {
    tracing::info!(target: "queryben::azure::sign-out", "clearing tokens");
    azure_oauth::sign_out(&state.azure_tokens)
}

#[tauri::command]
#[specta::specta]
pub async fn azure_sign_out_account(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<(), AppError> {
    tracing::info!(target: "queryben::azure::sign-out-account", %account_id, "clearing one account");
    azure_oauth::sign_out_account(&state.azure_tokens, &account_id)
}

#[tauri::command]
#[specta::specta]
pub async fn azure_current_account(
    state: State<'_, AppState>,
) -> Result<Option<AzureAccount>, AppError> {
    azure_oauth::current_account(&state.azure_tokens)
}

#[tauri::command]
#[specta::specta]
pub async fn azure_list_accounts()
-> Result<Vec<crate::adapters::azure_accounts::AccountRegistryEntry>, AppError> {
    Ok(azure_oauth::list_accounts())
}

// ---- ARM browse --------------------------------------------------------------

#[tauri::command]
#[specta::specta]
pub async fn list_azure_subscriptions(
    state: State<'_, AppState>,
    tenant_id: String,
    client_id: String,
    account_id: Option<String>,
) -> Result<Vec<AzureSubscription>, AppError> {
    tracing::info!(target: "queryben::azure::list-subscriptions", ?account_id, "listing");
    let bearer = azure_oauth::acquire_token(
        &state.azure_tokens,
        &tenant_id,
        &client_id,
        SCOPE_MANAGEMENT,
        account_id.as_deref(),
    )
    .await?;
    azure_rest::list_subscriptions(&bearer).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_azure_sql_servers(
    state: State<'_, AppState>,
    tenant_id: String,
    client_id: String,
    subscription_id: String,
    account_id: Option<String>,
) -> Result<Vec<AzureSqlServer>, AppError> {
    tracing::info!(target: "queryben::azure::list-servers", %subscription_id, ?account_id);
    let bearer = azure_oauth::acquire_token(
        &state.azure_tokens,
        &tenant_id,
        &client_id,
        SCOPE_MANAGEMENT,
        account_id.as_deref(),
    )
    .await?;
    azure_rest::list_sql_servers(&bearer, &subscription_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_azure_sql_databases(
    state: State<'_, AppState>,
    tenant_id: String,
    client_id: String,
    server_id: String,
    account_id: Option<String>,
) -> Result<Vec<AzureSqlDatabase>, AppError> {
    tracing::info!(target: "queryben::azure::list-databases", %server_id, ?account_id);
    let bearer = azure_oauth::acquire_token(
        &state.azure_tokens,
        &tenant_id,
        &client_id,
        SCOPE_MANAGEMENT,
        account_id.as_deref(),
    )
    .await?;
    azure_rest::list_databases(&bearer, &server_id).await
}

// ---- connect ----------------------------------------------------------------

#[tauri::command]
#[specta::specta]
pub async fn connect_azure_sql(
    app: AppHandle,
    state: State<'_, AppState>,
    tenant_id: String,
    client_id: String,
    input: AzureConnectInput,
    account_id: Option<String>,
) -> Result<Connection, AppError> {
    tracing::info!(
        target: "queryben::azure::connect",
        server = %input.server_fqdn,
        database = %input.database,
        ?account_id,
    );

    // Mint the sqldb bearer right before opening the tiberius connection so
    // we're not racing expiry through the TDS handshake.
    let sql_bearer = azure_oauth::acquire_token(
        &state.azure_tokens,
        &tenant_id,
        &client_id,
        SCOPE_SQLDB,
        account_id.as_deref(),
    )
    .await?;

    let create = CreateConnectionInput {
        name: input.display_name.clone(),
        server: input.server_fqdn.clone(),
        database: input.database.clone(),
        port: None,
        username: None,
        password: None,
        auth_mode: AuthMode::AadToken,
        trust_server_certificate: false,
        aad_bearer: Some(sql_bearer),
        nickname: input.nickname.clone(),
        color: input.color,
    };

    // Open the connection to prove the bearer works. On 40615 (client IP not
    // on allowlist), mssql surfaces `AppError::FirewallBlocked { ip, .. }`;
    // pattern-match on it, PUT a firewall rule via ARM, retry once. Same
    // trick SSMS and ADS use.
    let _client = match mssql::connect_with_progress(&create, &app).await {
        Ok(c) => c,
        Err(AppError::FirewallBlocked { ip: client_ip, .. }) => {
            tracing::warn!(
                target: "queryben::azure::connect",
                %client_ip,
                "firewall block, adding ARM rule and retrying"
            );
            let payload = FirewallProgress {
                stage: "adding-firewall-rule",
                ip: client_ip.clone(),
            };
            if let Err(err) = app.emit(mssql::CONNECT_PROGRESS_EVENT, payload) {
                tracing::warn!(
                    target: "queryben::azure::connect",
                    %err,
                    "failed to emit connect-progress (firewall)"
                );
            }
            let mgmt_bearer = azure_oauth::acquire_token(
                &state.azure_tokens,
                &tenant_id,
                &client_id,
                SCOPE_MANAGEMENT,
                account_id.as_deref(),
            )
            .await?;
            let rule_name = format!(
                "QueryBen_ClientIP_{}",
                Utc::now().format("%Y-%m-%d_%H-%M-%S")
            );
            azure_rest::add_firewall_rule(
                &mgmt_bearer,
                &input.server_id,
                &rule_name,
                &client_ip,
                &client_ip,
            )
            .await?;
            // Firewall rule needs a beat to propagate to the SQL gateway.
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            mssql::connect_with_progress(&create, &app).await?
        }
        Err(e) => return Err(e),
    };

    // Fall back to the current signed-in account when the caller didn't
    // explicitly pin one. This keeps the wizard from silently binding a
    // connection to whatever the registry happens to sort first.
    let resolved_account_id = match account_id {
        Some(a) => Some(a),
        None => azure_oauth::current_account(&state.azure_tokens)?
            .map(|a| a.home_account_id),
    };

    let nickname = crate::core::connection::normalize_nickname(input.nickname)?;
    let connection = Connection {
        id: Uuid::new_v4(),
        name: input.display_name,
        server: input.server_fqdn,
        database: input.database,
        port: None,
        username: None,
        auth_mode: AuthMode::AadToken,
        created_at: Utc::now(),
        last_used: None,
        account_id: resolved_account_id,
        nickname,
        color: input.color,
    };
    let entry = ConnectionEntry {
        connection,
        // Bearer is never persisted; azure_oauth reacquires on next connect
        // using the tenant/client IDs stashed here.
        password: None,
        trust_server_certificate: false,
        tenant_id: Some(tenant_id),
        client_id: Some(client_id),
        // Wizard already gave us the ARM ID, so cache it now — the auto-
        // firewall path on subsequent query executions will skip subscription
        // discovery entirely.
        server_arm_id: Some(input.server_id.clone()),
    };
    state.registry.insert(entry)
}

