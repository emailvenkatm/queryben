//! Import config loader. Reads `<app_data_dir>/import.config.json` once at
//! app start; any failure (missing file, malformed JSON) falls back to the
//! hard-coded defaults so the import wizard never dies on a config error.
//! Same pattern as `infra::export_config` and `infra::notebook_config`.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::core::import::ImportOptions;

const CONFIG_FILE: &str = "import.config.json";

/// On-disk shape of `import.config.json`. The `defaultStringType` field is
/// the fallback rendered for ambiguous / mixed-type columns when the wizard
/// is told to create the target table; typical values are `NVARCHAR(255)`
/// (default) or `NVARCHAR(MAX)` for very wide free-text columns.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportConfig {
    pub sample_rows_for_inference: u32,
    pub chunk_size: u32,
    pub default_string_type: String,
    pub csv_delimiter: String,
    pub csv_header: bool,
    pub create_table_if_missing: bool,
    pub truncate_before_insert: bool,
    pub skip_on_error: bool,
}

impl Default for ImportConfig {
    fn default() -> Self {
        Self {
            sample_rows_for_inference: 500,
            chunk_size: 500,
            default_string_type: "NVARCHAR(255)".into(),
            csv_delimiter: ",".into(),
            csv_header: true,
            create_table_if_missing: true,
            truncate_before_insert: false,
            skip_on_error: false,
        }
    }
}

impl ImportConfig {
    pub fn load(app_data_dir: &Path) -> Self {
        let path = app_data_dir.join(CONFIG_FILE);
        let Ok(bytes) = std::fs::read(&path) else {
            return Self::default();
        };
        match serde_json::from_slice::<ImportConfig>(&bytes) {
            Ok(cfg) => cfg,
            Err(err) => {
                tracing::warn!(
                    target: "queryben::import",
                    ?path,
                    error = %err,
                    "import.config.json malformed; using defaults"
                );
                Self::default()
            }
        }
    }

    pub fn to_options(&self) -> ImportOptions {
        ImportOptions {
            create_table_if_missing: self.create_table_if_missing,
            truncate_before_insert: self.truncate_before_insert,
            skip_on_error: self.skip_on_error,
            chunk_size: self.chunk_size,
        }
    }
}
