//! Cell-execution abstraction. `NotebookCellExecutor` is the seam where future
//! kernels (Python via jupyter_client, Kusto, Postgres) plug in without a
//! rewrite. Today we ship two impls: `SqlServerCellExecutor` (delegates to the
//! existing MSSQL `execute_query` pipeline) and `MarkdownCellExecutor` (no-op —
//! rendering happens in React).
//!
//! Executors register into a `HashMap<CellKind, Arc<dyn NotebookCellExecutor>>`
//! at app startup via `default_registry()`.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde::Serialize;
use tauri::State;
use uuid::Uuid;

use crate::core::notebook::CellKind;
use crate::core::query::QueryOutcome;
use crate::error::AppError;
use crate::state::AppState;

/// Payload returned by a single-cell run. `Sql` carries a full `QueryOutcome`
/// so the frontend can hand it to the same grid the query editor uses;
/// `Markdown` is a no-op ack so the UI can flip a spinner off.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CellRunResult {
    Sql { outcome: QueryOutcome },
    Markdown,
}

/// Context handed to every executor. Non-SQL executors can ignore fields they
/// don't need.
pub struct CellRunContext<'a> {
    pub state: State<'a, AppState>,
    pub connection_id: Option<Uuid>,
    pub source: String,
    pub max_rows: usize,
}

#[async_trait]
pub trait NotebookCellExecutor: Send + Sync {
    async fn run<'a>(&self, ctx: CellRunContext<'a>) -> Result<CellRunResult, AppError>;
}

pub struct SqlServerCellExecutor;

#[async_trait]
impl NotebookCellExecutor for SqlServerCellExecutor {
    async fn run<'a>(&self, ctx: CellRunContext<'a>) -> Result<CellRunResult, AppError> {
        let connection_id = ctx.connection_id.ok_or_else(|| {
            AppError::NotFound(
                "notebook has no active connection — pick one from the notebook toolbar"
                    .into(),
            )
        })?;
        let mut outcome =
            crate::ipc::query::execute_query(ctx.state, connection_id, ctx.source).await?;
        cap_rows(&mut outcome, ctx.max_rows);
        Ok(CellRunResult::Sql { outcome })
    }
}

pub struct MarkdownCellExecutor;

#[async_trait]
impl NotebookCellExecutor for MarkdownCellExecutor {
    async fn run<'a>(&self, _ctx: CellRunContext<'a>) -> Result<CellRunResult, AppError> {
        Ok(CellRunResult::Markdown)
    }
}

pub type CellRegistry = HashMap<CellKind, Arc<dyn NotebookCellExecutor>>;

/// Build the default registry at app startup. Adding a new kernel is a
/// one-liner: implement `NotebookCellExecutor`, then insert here.
pub fn default_registry() -> CellRegistry {
    let mut map: CellRegistry = HashMap::new();
    map.insert(CellKind::Sql, Arc::new(SqlServerCellExecutor));
    map.insert(CellKind::Markdown, Arc::new(MarkdownCellExecutor));
    map
}

// Truncate each set to `max_rows` client-side; the underlying MSSQL executor
// applies its own ROW_CAP (10k) which is well above the per-cell budget users
// care about in a notebook (500 by default).
fn cap_rows(outcome: &mut QueryOutcome, max: usize) {
    for rs in outcome.result_sets.iter_mut() {
        if rs.rows.len() > max {
            rs.rows.truncate(max);
            rs.truncated = true;
        }
    }
}
