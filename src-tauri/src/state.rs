// AppState registered via tauri::Builder::manage(). All fields must be Send + Sync.

use std::path::Path;
use std::sync::Arc;

use crate::adapters::azure::oauth::TokenCache;
use crate::core::connection::ConnectionRegistry;
use crate::error::AppError;

pub struct AppState {
    pub registry: Arc<ConnectionRegistry>,
    // In-memory cache of access tokens keyed by scope. Refresh token lives in
    // the OS keychain, not here.
    pub azure_tokens: Arc<TokenCache>,
}

impl AppState {
    pub fn new(app_data_dir: &Path) -> Result<Self, AppError> {
        let registry = Arc::new(ConnectionRegistry::new(app_data_dir)?);

        match crate::adapters::azure::oauth::migrate_legacy_account_if_needed() {
            Ok(Some(account_id)) => match registry.backfill_missing_account_id(&account_id) {
                Ok(count) if count > 0 => tracing::info!(
                    target: "queryben::state::migration",
                    %account_id,
                    count,
                    "backfilled account_id on legacy AAD connections"
                ),
                Ok(_) => {}
                Err(err) => tracing::warn!(
                    target: "queryben::state::migration",
                    %err,
                    "account_id backfill failed"
                ),
            },
            Ok(None) => {}
            Err(err) => tracing::warn!(
                target: "queryben::state::migration",
                %err,
                "legacy-account migration probe failed"
            ),
        }

        Ok(Self {
            registry,
            azure_tokens: Arc::new(TokenCache::new()),
        })
    }
}
