//! Azure SQL firewall auto-fix. Called from the frontend when an operation
//! (connect, execute_query, get_schema) surfaces `AppError::FirewallBlocked`.
//! Mirrors ADS's flow: acquire an ARM token, PUT a firewall rule pinning the
//! caller's client IP, done. Retry is the caller's job — Rust just says "rule
//! is now in place, try again".
//!
//! ARM auth is decoupled from SQL auth: an operator connecting with SQL
//! username+password can still sign in to Azure once to allow their IP. The
//! sign-in prompt is the same browser flow the AAD-token wizard uses; the
//! frontend calls `azure_sign_in` before hitting this command when the user
//! isn't signed in yet.
//!
//! Discovery cost: first call against a given connection scans every
//! subscription the signed-in account can see. The result is cached on the
//! ConnectionEntry (persisted to connections.json) so the next 40615 skips
//! straight to the ARM PUT.

use tauri::State;
use uuid::Uuid;

use crate::error::AppError;
use crate::adapters::{azure_oauth, azure_rest};
use crate::state::AppState;

const SCOPE_MANAGEMENT: &str = "https://management.azure.com/.default";
const SCOPE_SQLDB: &str = "https://database.windows.net/.default";

// Well-known public client ID for the Azure CLI. Microsoft explicitly documents
// this as the recommended public client for admin/management operations from
// third-party tools — it's what `az login`, PowerShell's `Connect-AzAccount`,
// and every Terraform provider use. Safe to hard-code.
// Docs: https://learn.microsoft.com/en-us/azure/active-directory/develop/msal-net-migration-public-client
const AZURE_CLI_CLIENT_ID: &str = "04b07795-8ddb-461a-bbee-02f9e1bf7b46";

// Empty tenant string routes through /organizations in azure_oauth::authority,
// which is Microsoft's multi-tenant endpoint. Lets the user pick their Azure
// directory at sign-in when the SQL connection didn't cache one.
const COMMON_TENANT: &str = "";

/// Add a firewall rule spanning `start_ip`..=`end_ip` on the Azure SQL server
/// backing `connection_id`. Works for SQL-auth and AAD-token connections alike
/// — creating a firewall rule is an Azure Resource Manager operation and only
/// needs an ARM bearer, independent of how the SQL connection authenticates.
/// The caller must have already signed in to Azure (via `azure_sign_in`); the
/// frontend gates this by triggering sign-in first when `azure_current_account`
/// returns None.
///
/// When `start_ip == end_ip` this is a single-IP rule (what ADS's "Add my
/// client IP" radio produces). When they differ it's a range — typically a
/// /24 covering the caller's ISP block, matching ADS's "Add my subnet IP
/// range" radio. The /24 form is what most engineers pick because ISPs shift
/// the last octet within the same block, so a single-IP rule re-triggers
/// 40615 after a few days.
#[tauri::command]
#[specta::specta]
pub async fn add_firewall_rule(
    state: State<'_, AppState>,
    connection_id: Uuid,
    start_ip: String,
    end_ip: String,
    rule_name: String,
) -> Result<(), AppError> {
    tracing::info!(
        target: "queryben::firewall::add",
        %connection_id,
        %start_ip,
        %end_ip,
        %rule_name,
        "entry"
    );

    let snapshot = state.registry.snapshot(connection_id)?;

    // Prefer the connection's own tenant/client (AAD-token flow) so the ARM
    // token audience matches the directory that owns the SQL server. Fall back
    // to the Azure CLI public client + /organizations for SQL-auth connections
    // where we never cached a tenant. Same trick every Microsoft admin tool
    // uses when it doesn't have a bespoke app registration.
    let tenant_id = snapshot.tenant_id.as_deref().unwrap_or(COMMON_TENANT);
    let client_id = snapshot
        .client_id
        .as_deref()
        .unwrap_or(AZURE_CLI_CLIENT_ID);

    let mgmt_bearer = azure_oauth::acquire_token(
        &state.azure_tokens,
        tenant_id,
        client_id,
        SCOPE_MANAGEMENT,
        snapshot.connection.account_id.as_deref(),
    )
    .await?;

    // Cache-first: reuse the ARM ID the wizard stashed on the entry. If the
    // connection predates the arm-id cache, fall back to discovery and
    // write the result back so we don't pay the cost again.
    let arm_id = match snapshot.server_arm_id.clone() {
        Some(id) => id,
        None => {
            tracing::info!(
                target: "queryben::firewall::add",
                server = %snapshot.connection.server,
                "no cached ARM id; discovering"
            );
            let discovered =
                azure_rest::discover_sql_server(&mgmt_bearer, &snapshot.connection.server).await?;
            // Best-effort cache write; if this fails (e.g. disk full) we'd
            // rather still add the firewall rule than block the user.
            if let Err(err) = state
                .registry
                .set_server_arm_id(connection_id, discovered.server_arm_id.clone())
            {
                tracing::warn!(
                    target: "queryben::firewall::add",
                    %connection_id,
                    %err,
                    "failed to cache ARM id on connection"
                );
            }
            discovered.server_arm_id
        }
    };

    tracing::info!(
        target: "queryben::firewall::add",
        %connection_id,
        %arm_id,
        %rule_name,
        %start_ip,
        %end_ip,
        "PUT firewallRule"
    );
    azure_rest::add_firewall_rule(&mgmt_bearer, &arm_id, &rule_name, &start_ip, &end_ip).await?;

    // Azure's SQL gateway takes a beat to see the new rule. Blocking here
    // keeps the frontend contract simple: "when this promise resolves, retry."
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    Ok(())
}

/// Probe whether `add_firewall_rule` for this connection could run without any
/// user interaction. The frontend calls this the moment a `FirewallBlocked`
/// error is caught — if `true`, it skips the dialog and fires the add + retry
/// silently with a small toast; if `false`, it falls back to showing the
/// existing sign-in dialog.
///
/// Contract:
///   - Never opens a browser.
///   - Never emits events, never prompts.
///   - `Ok(true)` iff a mgmt access token is already in the in-memory cache
///     OR the refresh token in the keychain can be silently traded for one.
///   - `Ok(false)` iff sign-in would be needed (no refresh token, or refresh
///     rejected).
///   - `Err(_)` for genuine failures (network down, Azure 500s) so the caller
///     can distinguish "can't tell right now" from "definitely needs sign-in".
#[tauri::command]
#[specta::specta]
pub async fn can_add_rule_silently(
    state: State<'_, AppState>,
    connection_id: Uuid,
) -> Result<bool, AppError> {
    let snapshot = state.registry.snapshot(connection_id)?;
    let tenant_id = snapshot.tenant_id.as_deref().unwrap_or(COMMON_TENANT);
    let client_id = snapshot
        .client_id
        .as_deref()
        .unwrap_or(AZURE_CLI_CLIENT_ID);

    match azure_oauth::try_acquire_silent(
        &state.azure_tokens,
        tenant_id,
        client_id,
        SCOPE_MANAGEMENT,
        snapshot.connection.account_id.as_deref(),
    )
    .await
    {
        Ok(_) => Ok(true),
        // AuthFailed here means "sign-in needed" — refresh token missing or
        // rejected. Not an error from the caller's perspective; it's the
        // signal to fall back to the interactive dialog.
        Err(AppError::AuthFailed(_)) => Ok(false),
        // Network, DNS, TLS, or 5xx from AAD. Let the caller decide whether
        // to retry or fall back — silently swallowing would mask real outages.
        Err(other) => Err(other),
    }
}

/// Probe whether an AAD-token connection can mint a sqldb bearer without
/// prompting. Called by the ConnectionListScreen when it renders an AAD-token
/// card so the button copy can switch between "Connect" (cached) and
/// "Sign in & connect" (needs the browser dance).
///
/// SQL and mgmt scopes ride on the same refresh token, so in practice a
/// positive result here also unlocks the auto-firewall path. We keep the two
/// probes separate anyway so each caller asks about the scope it actually
/// needs — no cross-scope assumptions baked into the API.
///
/// Same contract as `can_add_rule_silently`: no browser, no prompts. AuthFailed
/// collapses to `Ok(false)`; network / 5xx bubbles as `Err`.
#[tauri::command]
#[specta::specta]
pub async fn has_cached_azure_token(
    state: State<'_, AppState>,
    connection_id: Uuid,
) -> Result<bool, AppError> {
    let snapshot = state.registry.snapshot(connection_id)?;
    let tenant_id = snapshot.tenant_id.as_deref().unwrap_or(COMMON_TENANT);
    let client_id = snapshot
        .client_id
        .as_deref()
        .unwrap_or(AZURE_CLI_CLIENT_ID);

    match azure_oauth::try_acquire_silent(
        &state.azure_tokens,
        tenant_id,
        client_id,
        SCOPE_SQLDB,
        snapshot.connection.account_id.as_deref(),
    )
    .await
    {
        Ok(_) => Ok(true),
        Err(AppError::AuthFailed(_)) => Ok(false),
        Err(other) => Err(other),
    }
}
