//! Notebook config loader. Reads `<app_data_dir>/notebook.config.json` once at
//! app start; on any error (missing file, malformed JSON, IO failure) we fall
//! back to a hard-coded default. Hot reload is out of scope — the user restarts
//! the app if they want to swap notebook directories.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const CONFIG_FILE: &str = "notebook.config.json";
const DEFAULT_STORAGE_DIR: &str = "~/QueryBen Notebooks";
const DEFAULT_KERNEL: &str = "sql";
const DEFAULT_MAX_ROWS: usize = 500;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NotebookConfig {
    /// Directory holding `.ipynb` files. `~` is expanded to $HOME. On macOS
    /// the default lands notebooks in `~/QueryBen Notebooks` so users can
    /// browse them in Finder alongside anything else they save.
    pub storage_dir: String,
    pub default_kernel: String,
    pub max_rows_per_cell: usize,
}

impl Default for NotebookConfig {
    fn default() -> Self {
        Self {
            storage_dir: DEFAULT_STORAGE_DIR.into(),
            default_kernel: DEFAULT_KERNEL.into(),
            max_rows_per_cell: DEFAULT_MAX_ROWS,
        }
    }
}

impl NotebookConfig {
    /// Read `notebook.config.json` from `app_data_dir`. Any failure short-
    /// circuits to `Default`.
    pub fn load(app_data_dir: &Path) -> Self {
        let path = app_data_dir.join(CONFIG_FILE);
        let Ok(bytes) = std::fs::read(&path) else {
            return Self::default();
        };
        match serde_json::from_slice::<NotebookConfig>(&bytes) {
            Ok(cfg) => cfg,
            Err(err) => {
                tracing::warn!(
                    target: "queryben::notebook",
                    ?path,
                    error = %err,
                    "notebook.config.json malformed; using defaults"
                );
                Self::default()
            }
        }
    }

    /// Resolve `storage_dir` to an absolute path, expanding `~` and creating
    /// the directory tree if it doesn't exist yet. Returns the directory the
    /// caller should read/write notebooks in.
    pub fn resolve_storage_dir(&self) -> Result<PathBuf, std::io::Error> {
        let expanded = expand_tilde(&self.storage_dir);
        if !expanded.exists() {
            std::fs::create_dir_all(&expanded)?;
        }
        Ok(expanded)
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
