//! Run a batch of statements inside a single BEGIN/COMMIT. Any statement
//! failure rolls back and reports the failing index.

use std::time::Instant;

use crate::adapters::mssql;
use crate::core::ids::ConnectionId;
use crate::core::schema::TransactionResult;
use crate::error::AppError;
use crate::state::AppState;

use super::session::reopen_input;

// Hard cap so a runaway frontend can't ship a million-statement transaction
// and lock the connection. 500 is well above any reasonable batch of pending
// edits from a single grid session.
const MAX_TRANSACTION_STATEMENTS: usize = 500;

pub async fn run(
    state: &AppState,
    connection_id: ConnectionId,
    statements: Vec<String>,
) -> Result<TransactionResult, AppError> {
    let uuid = connection_id.as_uuid();
    let statement_count = statements.len();
    tracing::info!(
        target: "queryben::execute-transaction",
        connection_id = %uuid,
        statement_count,
        "entry"
    );

    // Empty batch is a no-op success. Saves a connect + BEGIN/COMMIT round trip
    // when the user hits "Commit" with nothing staged.
    if statements.is_empty() {
        return Ok(TransactionResult {
            committed: true,
            rows_affected: 0,
            statement_count: 0,
            duration_ms: 0,
            failed_statement_index: None,
            error_message: None,
        });
    }

    if statement_count > MAX_TRANSACTION_STATEMENTS {
        return Err(AppError::internal(format!(
            "transaction size {statement_count} exceeds cap of {MAX_TRANSACTION_STATEMENTS}"
        )));
    }

    let snapshot = state.registry.snapshot(uuid)?;
    let input = reopen_input(state, snapshot).await?;
    let mut client = mssql::connect_for_connection(&input, uuid).await?;

    let started = Instant::now();

    // BEGIN via simple_query — it doesn't return rows and tiberius rejects
    // multi-statement `execute` batches.
    if let Err(err) = client.simple_query("BEGIN TRANSACTION").await {
        tracing::error!(
            target: "queryben::execute-transaction",
            %connection_id,
            error = %err,
            "BEGIN failed"
        );
        return Err(err.into());
    }

    let mut rows_affected: u64 = 0;
    let mut failure: Option<(u32, String)> = None;

    for (idx, sql) in statements.iter().enumerate() {
        match client.execute(sql.as_str(), &[]).await {
            Ok(result) => {
                rows_affected = rows_affected.saturating_add(result.total());
            }
            Err(err) => {
                let msg = err.to_string();
                tracing::warn!(
                    target: "queryben::execute-transaction",
                    %connection_id,
                    statement_index = idx,
                    error = %msg,
                    "statement failed; rolling back"
                );
                failure = Some((idx as u32, msg));
                break;
            }
        }
    }

    let (committed, failed_statement_index, error_message) = match failure {
        None => {
            // All good — commit. If COMMIT itself fails we treat it as a
            // failure of the last statement (the whole batch didn't land).
            // Note: we .map(drop) the Ok arm so the QueryStream borrow releases
            // before the fallback ROLLBACK below re-borrows `client`.
            let commit_err = client.simple_query("COMMIT").await.err();
            match commit_err {
                None => (true, None, None),
                Some(err) => {
                    let msg = err.to_string();
                    tracing::error!(
                        target: "queryben::execute-transaction",
                        %connection_id,
                        error = %msg,
                        "COMMIT failed"
                    );
                    // Best-effort rollback; server may have already aborted.
                    if let Err(rb) = client.simple_query("ROLLBACK").await {
                        tracing::warn!(
                            target: "queryben::execute-transaction",
                            %connection_id,
                            error = %rb,
                            "ROLLBACK after COMMIT-failure also failed"
                        );
                    }
                    // rows_affected is meaningless when nothing committed.
                    rows_affected = 0;
                    (
                        false,
                        Some((statement_count.saturating_sub(1)) as u32),
                        Some(format!("commit failed: {msg}")),
                    )
                }
            }
        }
        Some((idx, msg)) => {
            if let Err(rb) = client.simple_query("ROLLBACK").await {
                tracing::warn!(
                    target: "queryben::execute-transaction",
                    %connection_id,
                    error = %rb,
                    "ROLLBACK failed after statement error"
                );
            }
            rows_affected = 0;
            (false, Some(idx), Some(msg))
        }
    };

    state.registry.mark_used(uuid).ok();

    let duration_ms = started.elapsed().as_millis() as u32;
    tracing::info!(
        target: "queryben::execute-transaction",
        %connection_id,
        committed,
        rows_affected,
        statement_count,
        duration_ms,
        ?failed_statement_index,
        "done"
    );

    Ok(TransactionResult {
        committed,
        rows_affected,
        statement_count: statement_count as u32,
        duration_ms,
        failed_statement_index,
        error_message,
    })
}
