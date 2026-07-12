use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, thiserror::Error, Serialize, specta::Type)]
#[serde(tag = "kind", content = "message")]
pub enum AppError {
    #[error("unauthorized")]
    Unauthorized,

    #[error("not found: {0}")]
    NotFound(String),

    #[error("validation: {0}")]
    Validation(String),

    #[error("io: {0}")]
    Io(String),

    #[error("provider: {0}")]
    Provider(String),

    #[error("other: {0}")]
    Other(String),

    #[error("connection failed: {0}")]
    ConnectionFailed(String),

    #[error("authentication failed: {0}")]
    AuthFailed(String),

    #[error("query failed: {message} at line {line:?}:{column:?}")]
    QueryFailed {
        message: String,
        line: Option<u32>,
        column: Option<u32>,
    },

    // Azure SQL 40615: client IP not on the server firewall allowlist. `ip`
    // parses out of the tiberius token stream; `server` is the target FQDN;
    // `connection_id` lets the frontend pin the auto-fix dialog to the row.
    #[error("firewall blocked: {ip} not on {server} allowlist")]
    FirewallBlocked {
        ip: String,
        server: String,
        connection_id: Option<Uuid>,
    },

    #[error("cancelled")]
    Cancelled,

    #[error("not implemented: {0}")]
    NotImplemented(String),

    #[error("rate limited (retry after {retry_after_seconds:?}s)")]
    RateLimited { retry_after_seconds: Option<u32> },

    // Backend per-call timeout. Usually a dead TCP after a network switch
    // that tokio hasn't noticed yet. Frontend renders as "connection may
    // be stale" instead of a raw QueryFailed.
    #[error("timed out: {0}")]
    Timeout(String),

    #[error("internal: {0}")]
    Internal(String),
}

impl AppError {
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }

    // Convert tiberius errors while retaining server context. Token 40615
    // bubbles as FirewallBlocked so the frontend can offer auto-fix instead
    // of a raw "Query failed" banner.
    pub fn from_tiberius_with_server(
        err: tiberius::error::Error,
        server: &str,
        connection_id: Option<Uuid>,
    ) -> Self {
        use tiberius::error::Error as TE;
        if let TE::Server(token) = &err {
            if token.code() == 40615 {
                if let Some(ip) = parse_client_ip(token.message()) {
                    return AppError::FirewallBlocked {
                        ip,
                        server: server.to_string(),
                        connection_id,
                    };
                }
            }
        }
        Self::from(err)
    }
}

fn parse_client_ip(msg: &str) -> Option<String> {
    let marker = "IP address '";
    let start = msg.find(marker)? + marker.len();
    let end = start + msg[start..].find('\'')?;
    let ip = &msg[start..end];
    if ip.split('.').filter(|s| s.parse::<u8>().is_ok()).count() == 4 {
        Some(ip.to_string())
    } else {
        None
    }
}

impl From<tiberius::error::Error> for AppError {
    fn from(err: tiberius::error::Error) -> Self {
        use tiberius::error::Error as TE;
        match err {
            TE::Server(token) => AppError::QueryFailed {
                message: token.message().to_string(),
                line: Some(token.line()),
                column: None,
            },
            TE::Io { kind, message } => {
                AppError::ConnectionFailed(format!("io ({kind:?}): {message}"))
            }
            TE::Tls(msg) => AppError::ConnectionFailed(format!("tls: {msg}")),
            TE::Protocol(msg) => AppError::ConnectionFailed(format!("protocol: {msg}")),
            TE::Encoding(msg) => AppError::Internal(format!("encoding: {msg}")),
            TE::Conversion(msg) => AppError::Internal(format!("conversion: {msg}")),
            TE::Utf8 => AppError::Internal("utf8 decode failure".into()),
            TE::Utf16 => AppError::Internal("utf16 decode failure".into()),
            TE::ParseInt(e) => AppError::Internal(format!("parse int: {e}")),
            other => AppError::Internal(format!("tiberius: {other}")),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::ConnectionFailed(format!("io: {err}"))
    }
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        AppError::ConnectionFailed(format!("http: {err}"))
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
