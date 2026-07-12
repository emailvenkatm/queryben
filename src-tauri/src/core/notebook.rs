//! Notebook document + cell types. Wire shape is Jupyter-friendly so a `.ipynb`
//! written by this app also opens in JupyterLab (SQL-magic-flavored). The
//! `#[non_exhaustive]` on `CellKind` reserves room for future kernels
//! (Python, Kusto, Postgres) without breaking the serde contract.
//!
//! Outputs are not persisted to disk in this iteration — freshly loaded
//! notebooks always show empty results until the user re-runs each cell. This
//! avoids the specta/BigInt roundtripping constraint and matches how Jupyter
//! behaves when cleared before commit.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Notebook file format version. Bumped only when the on-disk shape changes in
/// a way older readers can't tolerate.
pub const NBFORMAT: u32 = 4;
pub const NBFORMAT_MINOR: u32 = 5;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Notebook {
    #[serde(default = "default_nbformat")]
    pub nbformat: u32,
    #[serde(default = "default_nbformat_minor", rename = "nbformat_minor")]
    pub nbformat_minor: u32,
    pub metadata: NotebookMeta,
    pub cells: Vec<Cell>,
}

fn default_nbformat() -> u32 { NBFORMAT }
fn default_nbformat_minor() -> u32 { NBFORMAT_MINOR }

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NotebookMeta {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub connection_id: Option<Uuid>,
    #[serde(default = "default_kernel")]
    pub kernel: String,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

fn default_kernel() -> String { "sql".into() }

impl Default for NotebookMeta {
    fn default() -> Self {
        Self {
            title: None,
            connection_id: None,
            kernel: default_kernel(),
            created_at: None,
            updated_at: None,
        }
    }
}

/// One cell in the notebook. `id` is stable across saves so the UI can key on
/// it without churning React state; `source` is a plain string (Jupyter also
/// allows a Vec<String>, but the string form round-trips cleanly and is what
/// JupyterLab writes for anything created after nbformat 4.5).
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Cell {
    pub id: Uuid,
    #[serde(rename = "cell_type")]
    pub kind: CellKind,
    #[serde(default)]
    pub source: String,
    #[serde(default, rename = "execution_count")]
    pub execution_count: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum CellKind {
    Sql,
    Markdown,
}

/// One entry in the `notebook_list` result. Mirrors what the sidebar renders:
/// display name (file stem), the full path, and a modified timestamp so cards
/// can sort recency-first.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NotebookSummary {
    pub id: String,
    pub name: String,
    pub path: String,
    pub modified_at: Option<String>,
}
