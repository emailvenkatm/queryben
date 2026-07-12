//! Tiberius (MSSQL/TDS) client. tokio TcpStream <-> tiberius async traits
//! via tokio-util's Compat shim.

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tiberius::{AuthMethod, Client, Config};
use tokio::net::TcpStream;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

use crate::core::connection::{AuthMode, CreateConnectionInput};
use crate::error::AppError;

pub type MssqlClient = Client<Compat<TcpStream>>;

// Event name and payload for connect-time progress. Emitted from the retry
// wrapper below so the UI can show "resuming database, attempt N of M" instead
// of a naked spinner during the 30-60s 40613 warm-up.
pub const CONNECT_PROGRESS_EVENT: &str = "queryben://connect-progress";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectProgress {
    pub stage: &'static str,
    pub attempt: u32,
    pub max_attempts: u32,
    pub waited_ms: u64,
}

// Thin wrapper for the connect-time path (commands/azure.rs). Delegates the
// actual TDS handshake + backoff to `connect_inner` and emits progress events
// on the 40613 resume branch. Non-connect callers (reopen_input in query.rs)
// keep using `connect` directly to stay AppHandle-free.
pub async fn connect_with_progress(
    input: &CreateConnectionInput,
    app: &AppHandle,
) -> Result<MssqlClient, AppError> {
    connect_inner(input, Some(app), None).await
}

pub async fn connect(input: &CreateConnectionInput) -> Result<MssqlClient, AppError> {
    connect_inner(input, None, None).await
}

/// Same as `connect` but tags any surfaced `FirewallBlocked` with the caller's
/// registry UUID so the frontend can offer "sign in & add rule against this
/// connection". Use this from query/schema paths that already know the id.
pub async fn connect_for_connection(
    input: &CreateConnectionInput,
    connection_id: uuid::Uuid,
) -> Result<MssqlClient, AppError> {
    connect_inner(input, None, Some(connection_id)).await
}

async fn connect_inner(
    input: &CreateConnectionInput,
    app: Option<&AppHandle>,
    connection_id: Option<uuid::Uuid>,
) -> Result<MssqlClient, AppError> {
    let mut config = Config::new();
    config.host(&input.server);
    if let Some(port) = input.port {
        config.port(port);
    }
    config.database(&input.database);

    if input.trust_server_certificate {
        config.trust_cert();
    }

    match input.auth_mode {
        AuthMode::SqlAuth => {
            let user = input.username.as_deref().ok_or_else(|| {
                AppError::AuthFailed("SQL auth requires a username".into())
            })?;
            let password = input.password.as_deref().unwrap_or("");
            config.authentication(AuthMethod::sql_server(user, password));
        }
        // AadToken and AadInteractive share the same runtime path: the reopen
        // helpers mint a bearer via azure_oauth::acquire_token (MSAL PKCE +
        // keychain-stored refresh token) and stash it in `aad_bearer` before
        // handing tiberius `AuthMethod::aad_token`. The label distinction is
        // purely for UI recognition (SSMS/ADS parity).
        AuthMode::AadToken | AuthMode::AadInteractive => {
            let bearer = input.aad_bearer.as_deref().ok_or_else(|| {
                AppError::AuthFailed("AAD bearer token missing".into())
            })?;
            config.authentication(AuthMethod::aad_token(bearer));
        }
        AuthMode::AadPassword => {
            return Err(AppError::NotImplemented(
                "AAD Password not wired yet".into(),
            ));
        }
        AuthMode::AadManagedIdentity => {
            return Err(AppError::NotImplemented(
                "AAD Managed Identity not wired yet".into(),
            ));
        }
    }

    // Azure SQL Serverless auto-pauses after ~1h idle. First connect returns
    // 40613 "Database is not currently available" and kicks off a resume that
    // takes 30-60s. Retry on that exact error for up to ~90s. Everything else
    // (auth, bad server) fails immediately.
    const MAX_ATTEMPTS: u32 = 10; // attempts 0..=9 -> 10 tries
    let mut attempt: u32 = 0;
    let mut waited_ms: u64 = 0;
    loop {
        let tcp = TcpStream::connect(config.get_addr()).await?;
        tcp.set_nodelay(true).ok();

        match Client::connect(config.clone(), tcp.compat_write()).await {
            Ok(client) => return Ok(client),
            Err(e) => {
                let msg = e.to_string();
                let is_resume =
                    msg.contains("is not currently available") || msg.contains("40613");
                if !is_resume || attempt >= 9 {
                    // Server context turns the raw tiberius error into a
                    // typed FirewallBlocked when it's 40615, so downstream
                    // commands can pattern-match the variant.
                    return Err(AppError::from_tiberius_with_server(
                        e,
                        &input.server,
                        connection_id,
                    ));
                }
                let delay = std::time::Duration::from_secs(match attempt {
                    0 | 1 => 3,
                    2 | 3 => 5,
                    _ => 10,
                });
                tracing::info!(
                    target: "queryben::mssql::connect",
                    attempt,
                    delay_secs = delay.as_secs(),
                    "database resuming, retry after backoff"
                );
                if let Some(app) = app {
                    let payload = ConnectProgress {
                        stage: "resuming",
                        attempt: attempt + 1,
                        max_attempts: MAX_ATTEMPTS,
                        waited_ms,
                    };
                    if let Err(err) = app.emit(CONNECT_PROGRESS_EVENT, payload) {
                        tracing::warn!(
                            target: "queryben::mssql::connect",
                            %err,
                            "failed to emit connect-progress"
                        );
                    }
                }
                tokio::time::sleep(delay).await;
                waited_ms += delay.as_millis() as u64;
                attempt += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input_with(auth_mode: AuthMode, bearer: Option<String>) -> CreateConnectionInput {
        CreateConnectionInput {
            name: "n".into(),
            server: "127.0.0.1".into(),
            database: "db".into(),
            port: Some(1),
            username: None,
            password: None,
            auth_mode,
            trust_server_certificate: false,
            aad_bearer: bearer,
            nickname: None,
            color: None,
        }
    }

    // Regression for "AAD Interactive not wired yet": both AadToken and
    // AadInteractive must route through the same bearer branch. Prior to the
    // fix, AadInteractive short-circuited with NotImplemented before the TCP
    // dial. Now the missing-bearer failure is the same AuthFailed both variants
    // hit when a caller forgot to pre-mint the token — proof they share code.
    #[tokio::test]
    async fn aad_interactive_and_aad_token_share_bearer_branch() {
        let no_bearer_token = connect(&input_with(AuthMode::AadToken, None)).await;
        let no_bearer_interactive =
            connect(&input_with(AuthMode::AadInteractive, None)).await;
        assert!(
            matches!(no_bearer_token, Err(AppError::AuthFailed(_))),
            "expected AadToken missing-bearer to be AuthFailed, got {no_bearer_token:?}"
        );
        assert!(
            matches!(no_bearer_interactive, Err(AppError::AuthFailed(_))),
            "expected AadInteractive missing-bearer to be AuthFailed (not NotImplemented), got {no_bearer_interactive:?}"
        );
    }
}
