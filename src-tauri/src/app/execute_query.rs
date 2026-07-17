//! Run a user-authored batch and collect result sets.
//!
//! Per-call budget for tiberius stream reads/writes and the initial connect.
//! tokio TcpStream doesn't detect dead sockets (WiFi switch, VPN drop), so an
//! unwrapped `.await` on a stale connection hangs forever. 60s is long enough
//! that a legitimately slow query (e.g. cold Azure SQL page reads) still lands,
//! but short enough that the user gets a clean error instead of a permaspinner.

use std::time::{Duration, Instant};

use futures_util::TryStreamExt;
use tiberius::{Query, QueryItem, Row};

use crate::adapters::mssql;
use crate::core::ids::ConnectionId;
use crate::core::query::{ColumnMeta, QueryOutcome, ResultSet, ROW_CAP};
use crate::error::AppError;
use crate::state::AppState;

use super::row_convert::{classify_column_type, row_to_cells};
use super::session::reopen_input;

const QUERY_TIMEOUT_SECS: u64 = 60;

pub async fn run(
    state: &AppState,
    connection_id: ConnectionId,
    sql: String,
) -> Result<QueryOutcome, AppError> {
    let uuid = connection_id.as_uuid();
    tracing::info!(target: "queryben::execute-query", connection_id = %uuid, sql_len = sql.len());

    let snapshot = state.registry.snapshot(uuid)?;
    let input = reopen_input(state, snapshot).await?;

    // Timeout the initial connect too — a post-network-switch TLS handshake
    // can wedge just as hard as an active stream.
    let mut client = match tokio::time::timeout(
        Duration::from_secs(QUERY_TIMEOUT_SECS),
        mssql::connect_for_connection(&input, uuid),
    )
    .await
    {
        Ok(res) => res?,
        Err(_) => {
            tracing::error!(
                target: "queryben::execute-query",
                %connection_id,
                timeout_secs = QUERY_TIMEOUT_SECS,
                "connect timed out — likely dead socket after network change"
            );
            return Err(AppError::Timeout(format!(
                "connect exceeded {QUERY_TIMEOUT_SECS}s; the connection may be stale after a network change"
            )));
        }
    };
    tracing::info!(target: "queryben::execute-query", %connection_id, "connected");
    let batch_started = Instant::now();

    let query = Query::new(sql);
    // Send failure (invalid TDS packet, dead connection, etc.) can't be
    // partitioned across statements — no result sets exist yet, so bubble it
    // as a hard error like before.
    let mut stream = match tokio::time::timeout(
        Duration::from_secs(QUERY_TIMEOUT_SECS),
        query.query(&mut client),
    )
    .await
    {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => {
            tracing::error!(target: "queryben::execute-query", %connection_id, error = %e, "query send failed");
            return Err(e.into());
        }
        Err(_) => {
            tracing::error!(
                target: "queryben::execute-query",
                %connection_id,
                timeout_secs = QUERY_TIMEOUT_SECS,
                "query send timed out — likely dead socket after network change"
            );
            return Err(AppError::Timeout(format!(
                "query send exceeded {QUERY_TIMEOUT_SECS}s; the connection may be stale after a network change"
            )));
        }
    };
    tracing::info!(target: "queryben::execute-query", %connection_id, "stream opened");

    // Multi-result-set collector. Each `QueryItem::Metadata` opens a fresh
    // in-progress set; each `QueryItem::Row` appends to whatever set is
    // currently open. When the next Metadata (or stream end) arrives we flush
    // the in-progress set into `result_sets`. Timing is per-set: we snapshot
    // `Instant::now()` at Metadata and diff at flush time. Stream errors mid-
    // batch: keep every already-finished set, record `error = Some(msg)`, and
    // return normally so the frontend can render the successful ones + an
    // inline error where the failing set would go.
    let mut result_sets: Vec<ResultSet> = Vec::new();
    let mut cur_columns: Option<Vec<ColumnMeta>> = None;
    let mut cur_rows: Vec<Row> = Vec::new();
    let mut cur_started: Instant = Instant::now();
    let mut error: Option<String> = None;

    loop {
        let next = match tokio::time::timeout(
            Duration::from_secs(QUERY_TIMEOUT_SECS),
            stream.try_next(),
        )
        .await
        {
            Ok(res) => res,
            Err(_) => {
                tracing::error!(
                    target: "queryben::execute-query",
                    %connection_id,
                    timeout_secs = QUERY_TIMEOUT_SECS,
                    completed_sets = result_sets.len(),
                    "stream read timed out — likely dead socket after network change"
                );
                return Err(AppError::Timeout(format!(
                    "query read exceeded {QUERY_TIMEOUT_SECS}s; the connection may be stale after a network change"
                )));
            }
        };
        match next {
            Ok(Some(item)) => match item {
                QueryItem::Metadata(meta) => {
                    // New result set boundary. Flush whatever we had first.
                    flush(&mut result_sets, &mut cur_columns, &mut cur_rows, cur_started);
                    cur_started = Instant::now();
                    let cols: Vec<ColumnMeta> = meta
                        .columns()
                        .iter()
                        .map(|c| {
                            let sql_type = format!("{:?}", c.column_type());
                            ColumnMeta {
                                name: c.name().to_string(),
                                column_type: classify_column_type(&sql_type),
                                sql_type,
                                nullable: true,
                            }
                        })
                        .collect();
                    tracing::info!(
                        target: "queryben::execute-query",
                        %connection_id,
                        set_idx = result_sets.len(),
                        col_count = cols.len(),
                        "metadata frame — new result set opened"
                    );
                    cur_columns = Some(cols);
                }
                QueryItem::Row(row) => {
                    cur_rows.push(row);
                }
            },
            Ok(None) => break,
            Err(e) => {
                // Statement N blew up. Flush what we had for that set (may be
                // empty), record the error, stop iterating. Successful earlier
                // sets survive so the user isn't punished for a typo in the
                // last statement.
                let msg = e.to_string();
                tracing::error!(
                    target: "queryben::execute-query",
                    %connection_id,
                    completed_sets = result_sets.len(),
                    error = %msg,
                    "stream error mid-batch"
                );
                error = Some(msg);
                // Drop the in-progress set — its rows are undefined at the
                // point the server aborted, and the frontend renders the error
                // inline where this set would have gone.
                cur_columns = None;
                cur_rows.clear();
                break;
            }
        }
    }

    // Final flush for the tail set (Ok(None) branch).
    flush(&mut result_sets, &mut cur_columns, &mut cur_rows, cur_started);

    state.registry.mark_used(uuid).ok();

    let total_duration_ms = batch_started.elapsed().as_millis() as u32;
    tracing::info!(
        target: "queryben::execute-query",
        %connection_id,
        set_count = result_sets.len(),
        total_duration_ms,
        has_error = error.is_some(),
        "batch complete"
    );

    Ok(QueryOutcome {
        result_sets,
        total_duration_ms,
        error,
    })
}

// Flush the in-progress set (if any) into `result_sets`. Called on each fresh
// Metadata frame and once at stream end. Empty flushes (no columns yet) are
// skipped so a leading no-op DML doesn't emit a phantom grid.
fn flush(
    result_sets: &mut Vec<ResultSet>,
    cur_columns: &mut Option<Vec<ColumnMeta>>,
    cur_rows: &mut Vec<Row>,
    cur_started: Instant,
) {
    let Some(columns) = cur_columns.take() else {
        return;
    };
    let raw_rows: Vec<Row> = std::mem::take(cur_rows);
    let total = raw_rows.len();
    let truncated = total > ROW_CAP;
    let mut out_rows: Vec<Vec<crate::core::query::CellValue>> = Vec::with_capacity(total.min(ROW_CAP));
    for row in raw_rows.into_iter().take(ROW_CAP) {
        out_rows.push(row_to_cells(row));
    }
    let duration_ms = cur_started.elapsed().as_millis() as u32;
    result_sets.push(ResultSet {
        columns,
        rows: out_rows,
        row_count: total as u64,
        duration_ms,
        truncated,
    });
}
