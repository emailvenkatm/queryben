//! Config loader for saved-queries + query-history features.
//!
//! Reads `<app_data_dir>/queries.config.json` once at app start; any error
//! (missing file, malformed JSON, IO failure) falls back to hard-coded
//! defaults. Same pattern as `infra::notebook_config` so the two feature
//! configs live in one place mentally.

use std::path::Path;

use serde::{Deserialize, Serialize};

const CONFIG_FILE: &str = "queries.config.json";

/// Default retention window for auto-logged history rows. 90 days matches
/// what most editors ship out of the box (DataGrip = 30d, DBeaver = "forever"
/// with a manual purge). 90d strikes a balance between usefulness ("show me
/// what I ran last quarter") and disk pressure.
const DEFAULT_RETENTION_DAYS: u32 = 90;

/// Hard cap on history rows, regardless of retention window. Prevents an
/// automated test suite from blowing the DB up with 100k rows in one day and
/// still surviving the window-based prune.
const DEFAULT_MAX_ROWS: u32 = 5_000;

/// Default folder assigned to saved queries when the "Save query" dialog is
/// submitted with no folder. "General" is the industry-standard bucket name.
const DEFAULT_FOLDER: &str = "General";

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct QueriesConfig {
    pub history_retention_days: u32,
    pub history_max_rows: u32,
    /// When false, `log_history` becomes a silent no-op. Users who don't want
    /// their SQL landing on disk (compliance, shared machine) flip this off.
    pub auto_log_history: bool,
    pub saved_queries_default_folder: String,
}

impl Default for QueriesConfig {
    fn default() -> Self {
        Self {
            history_retention_days: DEFAULT_RETENTION_DAYS,
            history_max_rows: DEFAULT_MAX_ROWS,
            auto_log_history: true,
            saved_queries_default_folder: DEFAULT_FOLDER.into(),
        }
    }
}

impl QueriesConfig {
    /// Read `queries.config.json` from `app_data_dir`. Any failure short-
    /// circuits to `Default` and logs at warn — same policy as
    /// `NotebookConfig::load`.
    pub fn load(app_data_dir: &Path) -> Self {
        let path = app_data_dir.join(CONFIG_FILE);
        let Ok(bytes) = std::fs::read(&path) else {
            return Self::default();
        };
        match serde_json::from_slice::<QueriesConfig>(&bytes) {
            Ok(cfg) => cfg,
            Err(err) => {
                tracing::warn!(
                    target: "queryben::queries::config",
                    ?path,
                    error = %err,
                    "queries.config.json malformed; using defaults"
                );
                Self::default()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn missing_file_yields_defaults() {
        let tmp = TempDir::new().expect("tempdir");
        let cfg = QueriesConfig::load(tmp.path());
        assert_eq!(cfg, QueriesConfig::default());
    }

    #[test]
    fn malformed_file_falls_back_to_defaults() {
        let tmp = TempDir::new().expect("tempdir");
        std::fs::write(tmp.path().join(CONFIG_FILE), b"{not json").expect("write");
        let cfg = QueriesConfig::load(tmp.path());
        assert_eq!(cfg, QueriesConfig::default());
    }

    #[test]
    fn well_formed_file_is_honored() {
        let tmp = TempDir::new().expect("tempdir");
        std::fs::write(
            tmp.path().join(CONFIG_FILE),
            br#"{"historyRetentionDays":30,"historyMaxRows":100,"autoLogHistory":false,"savedQueriesDefaultFolder":"Ad-Hoc"}"#,
        )
        .expect("write");
        let cfg = QueriesConfig::load(tmp.path());
        assert_eq!(cfg.history_retention_days, 30);
        assert_eq!(cfg.history_max_rows, 100);
        assert!(!cfg.auto_log_history);
        assert_eq!(cfg.saved_queries_default_folder, "Ad-Hoc");
    }
}
