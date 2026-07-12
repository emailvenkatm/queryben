//! Export config loader. Reads `<app_data_dir>/export.config.json` once at
//! app start; any failure (missing file, malformed JSON, IO error) falls back
//! to the hard-coded defaults so the export button never dies on a config
//! error. Same pattern as `infra::notebook_config`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::core::export::{ExportFormat, ExportOptions};

const CONFIG_FILE: &str = "export.config.json";
const DEFAULT_DIR: &str = "~/Downloads";

/// On-disk shape of `export.config.json`. `defaultFormat` and `defaultDir`
/// power the export-dialog pre-fill; the format-specific fields are folded
/// into an `ExportOptions` and handed to the matching `RowExporter`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportConfig {
    pub default_format: ExportFormat,
    pub default_dir: String,
    pub csv_delimiter: String,
    pub csv_include_header: bool,
    pub json_pretty: bool,
    pub xlsx_sheet_name: String,
}

impl Default for ExportConfig {
    fn default() -> Self {
        Self {
            default_format: ExportFormat::Csv,
            default_dir: DEFAULT_DIR.into(),
            csv_delimiter: ",".into(),
            csv_include_header: true,
            json_pretty: true,
            xlsx_sheet_name: "Results".into(),
        }
    }
}

impl ExportConfig {
    /// Read `export.config.json` from `app_data_dir`. Any failure short-
    /// circuits to `Default` — export must not go dark because of a stray
    /// trailing comma in a config file.
    pub fn load(app_data_dir: &Path) -> Self {
        let path = app_data_dir.join(CONFIG_FILE);
        let Ok(bytes) = std::fs::read(&path) else {
            return Self::default();
        };
        match serde_json::from_slice::<ExportConfig>(&bytes) {
            Ok(cfg) => cfg,
            Err(err) => {
                tracing::warn!(
                    target: "queryben::export",
                    ?path,
                    error = %err,
                    "export.config.json malformed; using defaults"
                );
                Self::default()
            }
        }
    }

    /// Resolve `default_dir` to an absolute `PathBuf`, expanding `~`. Does
    /// NOT create the directory — the OS save dialog will surface an error
    /// itself if it doesn't exist, which is friendlier than silently making
    /// a directory the user didn't ask for.
    pub fn resolve_default_dir(&self) -> PathBuf {
        expand_tilde(&self.default_dir)
    }

    /// Fold the format-specific fields into the runtime `ExportOptions`
    /// handed to each `RowExporter`. Grabs the first character of
    /// `csv_delimiter` (comma if empty or multibyte — we don't try to be
    /// clever about tab-as-`\t`, that'd need a separate escape-parsing pass).
    pub fn to_options(&self) -> ExportOptions {
        let csv_delimiter = self.csv_delimiter.chars().next().unwrap_or(',');
        ExportOptions {
            csv_delimiter,
            csv_include_header: self.csv_include_header,
            json_pretty: self.json_pretty,
            xlsx_sheet_name: self.xlsx_sheet_name.clone(),
        }
    }
}

fn expand_tilde(input: &str) -> PathBuf {
    if let Some(stripped) = input.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(stripped);
        }
    }
    PathBuf::from(input)
}
