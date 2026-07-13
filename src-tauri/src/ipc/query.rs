//! Query execution + schema introspection commands.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use futures_util::TryStreamExt;
use tauri::State;
use tiberius::{ColumnData, Query, QueryItem, Row};
use uuid::Uuid;

// Per-call budget for tiberius stream reads/writes and the initial connect.
// tokio TcpStream doesn't detect dead sockets (WiFi switch, VPN drop), so an
// unwrapped `.await` on a stale connection hangs forever. 60s is long enough
// that a legitimately slow query (e.g. cold Azure SQL page reads) still lands,
// but short enough that the user gets a clean error instead of a permaspinner.
const QUERY_TIMEOUT_SECS: u64 = 60;

use crate::core::connection::{ConnectionSnapshot, CreateConnectionInput};
use crate::core::query::{
    CellValue, ColumnKind, ColumnMeta, QueryOutcome, ResultSet, ROW_CAP,
};
use crate::core::schema::{
    RoutineInfo, SchemaInfo, SchemaNode, TableColumn, TableInfo, TableMetadata,
    TransactionResult,
};
use crate::error::AppError;
use crate::adapters::{azure::oauth as azure_oauth, base64, mssql};
use crate::state::AppState;

// tiberius wants the bearer scoped to database.windows.net.
const SCOPE_SQLDB: &str = "https://database.windows.net/.default";

// System schemas we hide from the object explorer. Any DB role starting with
// `db_` is also skipped programmatically because MSSQL keeps adding them.
const SYSTEM_SCHEMAS: &[&str] = &["sys", "INFORMATION_SCHEMA", "guest"];

#[tauri::command]
#[specta::specta]
pub async fn execute_query(
    state: State<'_, AppState>,
    connection_id: Uuid,
    sql: String,
) -> Result<QueryOutcome, AppError> {
    tracing::info!(target: "queryben::execute-query", %connection_id, sql_len = sql.len());

    let snapshot = state.registry.snapshot(connection_id)?;
    let input = reopen_input(&state, snapshot).await?;

    // Timeout the initial connect too — a post-network-switch TLS handshake
    // can wedge just as hard as an active stream.
    let mut client = match tokio::time::timeout(
        Duration::from_secs(QUERY_TIMEOUT_SECS),
        mssql::connect_for_connection(&input, connection_id),
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

    // Flush the in-progress set (if any) into `result_sets`. Called on each
    // fresh Metadata frame and once at stream end. Empty flushes (no columns
    // yet) are skipped so a leading no-op DML doesn't emit a phantom grid.
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
        let mut out_rows: Vec<Vec<CellValue>> = Vec::with_capacity(total.min(ROW_CAP));
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

    state.registry.mark_used(connection_id).ok();

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

// Reopen a tiberius session from a snapshot. For AAD-token entries we mint a
// fresh bearer via the keychain-stored refresh token; cache hit is instant.
async fn reopen_input(
    state: &AppState,
    s: ConnectionSnapshot,
) -> Result<CreateConnectionInput, AppError> {
    let bearer = if s.connection.auth_mode.uses_aad_bearer() {
        let tenant = s.tenant_id.as_deref().ok_or_else(|| {
            AppError::AuthFailed(
                "AAD connection missing tenant_id; reconnect to repair".into(),
            )
        })?;
        let client = s.client_id.as_deref().ok_or_else(|| {
            AppError::AuthFailed(
                "AAD connection missing client_id; reconnect to repair".into(),
            )
        })?;
        Some(
            azure_oauth::acquire_token(
                &state.azure_tokens,
                tenant,
                client,
                SCOPE_SQLDB,
                s.connection.account_id.as_deref(),
            )
            .await?,
        )
    } else {
        None
    };

    let c = s.connection;
    Ok(CreateConnectionInput {
        name: c.name,
        server: c.server,
        database: c.database,
        port: c.port,
        username: c.username,
        password: s.password,
        auth_mode: c.auth_mode,
        trust_server_certificate: s.trust_server_certificate,
        aad_bearer: bearer,
        nickname: c.nickname,
        color: c.color,
    })
}

fn row_to_cells(row: Row) -> Vec<CellValue> {
    row.into_iter().map(convert_cell).collect()
}

// Map the tiberius `column_type()` debug label (e.g. "Intn", "Int4",
// "BigVarChar", "NVarcharMax", "Datetimen", "Datetime2", "Bitn", "Floatn",
// "Guid") onto the JS-facing `ColumnKind`. The browse grid keys its type-
// badge palette off this enum directly; when the field is missing or
// unrecognised the badge falls through to "unknown" and renders as "?".
fn classify_column_type(raw: &str) -> ColumnKind {
    // Tiberius names are PascalCase without underscores; case-insensitive
    // contains-checks are enough to survive minor version renames.
    let t = raw.to_ascii_lowercase();
    // Order matters: check bit/bool before "int" (Bitn contains no 'int',
    // but stay defensive).
    if t == "bit" || t.starts_with("bitn") || t == "bool" {
        return ColumnKind::Boolean;
    }
    if t.contains("int")
        || t.contains("float")
        || t.contains("real")
        || t.contains("money")
        || t.contains("decimal")
        || t.contains("numeric")
    {
        return ColumnKind::Number;
    }
    if t.contains("date") || t.contains("time") {
        return ColumnKind::Datetime;
    }
    if t.contains("char")
        || t.contains("text")
        || t.contains("xml")
        || t.contains("guid")
        || t.contains("uuid")
        || t.contains("varchar")
        || t.contains("string")
    {
        return ColumnKind::String;
    }
    ColumnKind::Unknown
}

// Row::into_iter yields `ColumnData<'static>`, and tiberius' `FromSql` requires
// the `'static` lifetime, so we bind it explicitly rather than eliding to
// `'_`. Row-owned string / binary variants keep their heap Cows; we just move
// them out with `.into_owned()`.
fn convert_cell(data: ColumnData<'static>) -> CellValue {
    use ColumnData::*;
    // Date-family branches use tiberius' own chrono `FromSql` impls (enabled by
    // the `chrono` + `tds73` features on the tiberius dep). We funnel every
    // temporal variant through `CellValue::DateTime(iso_string)` so the browse
    // grid's date-input renderers get a single canonical wire shape.
    //
    // ISO-8601 formats:
    //   DATE           -> YYYY-MM-DD
    //   TIME           -> HH:MM:SS[.fff]         (fractional seconds preserved)
    //   SMALLDATETIME  -> YYYY-MM-DDTHH:MM:00    (minute resolution)
    //   DATETIME       -> YYYY-MM-DDTHH:MM:SS.fff
    //   DATETIME2      -> YYYY-MM-DDTHH:MM:SS[.fffffff]
    //   DATETIMEOFFSET -> YYYY-MM-DDTHH:MM:SS[.fffffff]+HH:MM  (fixed offset)
    match data {
        Bit(None) | U8(None) | I16(None) | I32(None) | I64(None) | F32(None) | F64(None)
        | String(None) | Guid(None) | Binary(None) | Numeric(None) | Xml(None)
        | DateTime(None) | SmallDateTime(None) | Time(None) | Date(None) | DateTime2(None)
        | DateTimeOffset(None) => CellValue::Null,
        Bit(Some(v)) => CellValue::Bool(v),
        U8(Some(v)) => CellValue::Int(v as i64),
        I16(Some(v)) => CellValue::Int(v as i64),
        I32(Some(v)) => CellValue::Int(v as i64),
        I64(Some(v)) => CellValue::Int(v),
        F32(Some(v)) => CellValue::Float(v as f64),
        F64(Some(v)) => CellValue::Float(v),
        String(Some(s)) => CellValue::Text(s.into_owned()),
        Guid(Some(g)) => CellValue::Text(g.to_string()),
        Binary(Some(b)) => CellValue::Bytes(base64::encode(&b)),
        // DECIMAL / NUMERIC / MONEY / SMALLMONEY: tiberius `Money`/`SmallMoney`
        // fixed-len types decode to `F64` (already handled above); the variable-
        // len `Decimaln`/`Numericn` land here. `Numeric::to_string()` renders as
        // e.g. "1200000.00" — precision-preserving string, mirrors how the
        // datetime branch avoids f64 lossiness.
        Numeric(Some(n)) => CellValue::Text(n.to_string()),
        // XML value; `XmlData` implements `Display`.
        Xml(Some(x)) => CellValue::Text(x.to_string()),
        // Date / Time / SmallDateTime / DateTime / DateTime2 / DateTimeOffset.
        // The chrono `FromSql` conversions borrow the ColumnData; wrap the
        // owned value back up so lifetimes line up, then format ISO-8601.
        v @ (Date(Some(_)) | Time(Some(_)) | SmallDateTime(Some(_)) | DateTime(Some(_))
        | DateTime2(Some(_)) | DateTimeOffset(Some(_))) => datetime_cell(v),
        // All tiberius 0.12 `ColumnData` variants are covered above
        // (Money/SmallMoney decode to F64, Image/Text/NText decode to
        // Binary/String, so they never reach here as their own variants).
    }
}

// Format any of the six TDS temporal `ColumnData` variants as an ISO-8601
// string. Uses tiberius' chrono `FromSql` impls, which the `chrono` + `tds73`
// features on the tiberius dep provide. A conversion miss (should not happen
// for the variants the caller filters to) falls back to `{:?}` so the cell
// still renders something instead of blowing up the whole result set.
fn datetime_cell(data: ColumnData<'static>) -> CellValue {
    use tiberius::FromSql;
    let debug_fallback = || CellValue::Text(format!("{:?}", &data));
    let iso: Option<String> = match &data {
        ColumnData::Date(_) => chrono::NaiveDate::from_sql(&data)
            .ok()
            .flatten()
            .map(|d| d.format("%Y-%m-%d").to_string()),
        ColumnData::Time(_) => chrono::NaiveTime::from_sql(&data)
            .ok()
            .flatten()
            .map(|t| t.format("%H:%M:%S%.f").to_string()),
        ColumnData::SmallDateTime(_) | ColumnData::DateTime(_) | ColumnData::DateTime2(_) => {
            chrono::NaiveDateTime::from_sql(&data)
                .ok()
                .flatten()
                .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S%.f").to_string())
        }
        ColumnData::DateTimeOffset(_) => {
            // FromSql impl targeting FixedOffset preserves the source zone; the
            // Utc impl would silently normalize and drop the offset the user
            // stored.
            <chrono::DateTime<chrono::FixedOffset> as FromSql>::from_sql(&data)
                .ok()
                .flatten()
                .map(|dt| dt.to_rfc3339())
        }
        _ => None,
    };
    match iso {
        Some(s) => CellValue::DateTime(s),
        None => debug_fallback(),
    }
}

// TODO: real cancel needs per-query handles + a tiberius attention-token
// escape hatch. For now this errors so the UI shows an honest "not wired yet".
#[tauri::command]
#[specta::specta]
pub async fn cancel_query(
    _state: State<'_, AppState>,
    query_id: Uuid,
) -> Result<(), AppError> {
    tracing::warn!(target: "queryben::cancel-query", %query_id, "cancel not wired");
    Err(AppError::NotImplemented(
        "query cancellation not wired yet".into(),
    ))
}

// Skip anything in the system list plus every db_* role (db_owner, db_datareader, ...).
fn is_system_schema(name: &str) -> bool {
    SYSTEM_SCHEMAS.iter().any(|s| s.eq_ignore_ascii_case(name)) || name.starts_with("db_")
}

// The five introspection SQL statements. Each is executed as its own tiberius
// query in `introspect_all` — batching them collapsed empty result sets on
// fresh Azure SQL DBs (tiberius returned <5 vecs, non-deterministic which one
// dropped) which misaligned parsing and turned table names into "null".
// The ~5x roundtrip cost is fine for a 5-min-cached schema refresh.
const SCHEMAS_SQL_ALL: &str = "SELECT s.name AS schema_name
       FROM sys.schemas AS s
      WHERE s.name NOT LIKE 'db\\_%' ESCAPE '\\'
        AND s.name NOT IN ('sys', 'INFORMATION_SCHEMA', 'guest')";

const SCHEMAS_SQL_ONE: &str = "SELECT s.name AS schema_name
       FROM sys.schemas AS s
      WHERE s.name NOT LIKE 'db\\_%' ESCAPE '\\'
        AND s.name NOT IN ('sys', 'INFORMATION_SCHEMA', 'guest')
        AND s.name = @P1";

const OBJECTS_SQL_ALL: &str = "SELECT t.TABLE_SCHEMA, t.TABLE_NAME, t.TABLE_TYPE
       FROM INFORMATION_SCHEMA.TABLES AS t
      WHERE t.TABLE_TYPE IN ('BASE TABLE', 'VIEW')";

const OBJECTS_SQL_ONE: &str = "SELECT t.TABLE_SCHEMA, t.TABLE_NAME, t.TABLE_TYPE
       FROM INFORMATION_SCHEMA.TABLES AS t
      WHERE t.TABLE_TYPE IN ('BASE TABLE', 'VIEW')
        AND t.TABLE_SCHEMA = @P1";

const PROCS_SQL_ALL: &str = "SELECT s.name AS schema_name, p.name AS proc_name
       FROM sys.procedures AS p
       JOIN sys.schemas    AS s ON s.schema_id = p.schema_id";

const PROCS_SQL_ONE: &str = "SELECT s.name AS schema_name, p.name AS proc_name
       FROM sys.procedures AS p
       JOIN sys.schemas    AS s ON s.schema_id = p.schema_id
      WHERE s.name = @P1";

const FNS_SQL_ALL: &str = "SELECT s.name AS schema_name, o.name AS fn_name
       FROM sys.objects AS o
       JOIN sys.schemas AS s ON s.schema_id = o.schema_id
      WHERE o.type IN ('FN', 'IF', 'TF')";

const FNS_SQL_ONE: &str = "SELECT s.name AS schema_name, o.name AS fn_name
       FROM sys.objects AS o
       JOIN sys.schemas AS s ON s.schema_id = o.schema_id
      WHERE o.type IN ('FN', 'IF', 'TF')
        AND s.name = @P1";

const STATS_SQL_ALL: &str = "SELECT SCHEMA_NAME(t.schema_id) AS schema_name,
            t.name                   AS table_name,
            ISNULL((SELECT SUM(p.rows)
                      FROM sys.partitions AS p
                     WHERE p.object_id = t.object_id
                       AND p.index_id IN (0, 1)), 0) AS row_estimate,
            (SELECT COUNT(*)
               FROM sys.columns AS c
              WHERE c.object_id = t.object_id) AS col_count
       FROM sys.tables AS t";

const STATS_SQL_ONE: &str = "SELECT SCHEMA_NAME(t.schema_id) AS schema_name,
            t.name                   AS table_name,
            ISNULL((SELECT SUM(p.rows)
                      FROM sys.partitions AS p
                     WHERE p.object_id = t.object_id
                       AND p.index_id IN (0, 1)), 0) AS row_estimate,
            (SELECT COUNT(*)
               FROM sys.columns AS c
              WHERE c.object_id = t.object_id) AS col_count
       FROM sys.tables AS t
      WHERE SCHEMA_NAME(t.schema_id) = @P1";

// Runs one introspection SELECT. When `only_schema` is Some, binds it as @P1
// and uses the ONE variant; otherwise runs the ALL variant with no bindings.
// Logs per-query duration + row count so misalignment is easy to spot.
async fn run_query(
    client: &mut mssql::MssqlClient,
    label: &'static str,
    sql_all: &'static str,
    sql_one: &'static str,
    only_schema: Option<&str>,
) -> Result<Vec<Row>, AppError> {
    let started = Instant::now();
    let rows = match only_schema {
        Some(name) => {
            let mut q = Query::new(sql_one);
            q.bind(name.to_string());
            q.query(client).await?.into_first_result().await?
        }
        None => {
            Query::new(sql_all)
                .query(client)
                .await?
                .into_first_result()
                .await?
        }
    };
    tracing::info!(
        target: "queryben::introspect",
        query = label,
        rows = rows.len(),
        duration_ms = started.elapsed().as_millis() as u64,
        "introspection query complete"
    );
    Ok(rows)
}

// Pulled into a helper so both `get_schema` and any future per-schema drill-in
// share the same INFORMATION_SCHEMA / sys.* pass.
async fn introspect_all(
    client: &mut mssql::MssqlClient,
    only_schema: Option<&str>,
) -> Result<Vec<SchemaNode>, AppError> {
    // Five sequential SELECTs: schemas, tables+views, procs, functions, table
    // stats. sys.partitions gives a fast row estimate that doesn't require
    // SELECT permission on every heap. index_id 0/1 = heap or CI.
    tracing::info!(target: "queryben::introspect", only_schema = ?only_schema, "issuing introspection queries");
    let schema_rows = run_query(client, "schemas", SCHEMAS_SQL_ALL, SCHEMAS_SQL_ONE, only_schema).await?;
    let object_rows = run_query(client, "objects", OBJECTS_SQL_ALL, OBJECTS_SQL_ONE, only_schema).await?;
    let proc_rows   = run_query(client, "procs",   PROCS_SQL_ALL,   PROCS_SQL_ONE,   only_schema).await?;
    let fn_rows     = run_query(client, "fns",     FNS_SQL_ALL,     FNS_SQL_ONE,     only_schema).await?;
    let stats_rows  = run_query(client, "stats",   STATS_SQL_ALL,   STATS_SQL_ONE,   only_schema).await?;
    tracing::info!(
        target: "queryben::introspect",
        schemas = schema_rows.len(),
        objects = object_rows.len(),
        procs = proc_rows.len(),
        fns = fn_rows.len(),
        stats = stats_rows.len(),
        "parsed row counts"
    );

    let mut nodes: BTreeMap<String, SchemaNode> = BTreeMap::new();
    for row in schema_rows {
        let name = row_get_string(&row, 0, "schema_name")?;
        tracing::info!(target: "queryben::introspect", loop_ = "schemas", %name, "row");
        if is_system_schema(&name) {
            continue;
        }
        nodes.entry(name.clone()).or_insert_with(|| SchemaNode {
            name: name.clone(),
            tables: Vec::new(),
            views: Vec::new(),
            procedures: Vec::new(),
            functions: Vec::new(),
        });
    }

    // Row / column counts keyed by (schema, table). Missing means view or
    // no partition rows.
    let mut stats: BTreeMap<(String, String), (u64, u32)> = BTreeMap::new();
    for row in stats_rows {
        let schema = row_get_string(&row, 0, "schema_name")?;
        let table = row_get_string(&row, 1, "table_name")?;
        // SUM over BIGINT rows -> tiberius returns Numeric. Fall back to 0 if
        // the DB is empty or the column type surprises us.
        let rows_est = row_get_i64(&row, 2).unwrap_or(0).max(0) as u64;
        let col_count = row_get_i32(&row, 3).unwrap_or(0).max(0) as u32;
        tracing::info!(
            target: "queryben::introspect",
            loop_ = "stats",
            %schema,
            %table,
            rows_est,
            col_count,
            "row"
        );
        stats.insert((schema, table), (rows_est, col_count));
    }

    for row in object_rows {
        let schema = row_get_string(&row, 0, "TABLE_SCHEMA")?;
        let name = row_get_string(&row, 1, "TABLE_NAME")?;
        let kind = row_get_string(&row, 2, "TABLE_TYPE")?;
        tracing::info!(
            target: "queryben::introspect",
            loop_ = "objects",
            %schema,
            %name,
            %kind,
            "row"
        );
        if is_system_schema(&schema) {
            continue;
        }
        let node = nodes.entry(schema.clone()).or_insert_with(|| SchemaNode {
            name: schema.clone(),
            tables: Vec::new(),
            views: Vec::new(),
            procedures: Vec::new(),
            functions: Vec::new(),
        });
        let (row_count, column_count) = stats
            .get(&(schema.clone(), name.clone()))
            .map(|(r, c)| (Some(*r), Some(*c)))
            .unwrap_or((None, None));
        let table = TableInfo {
            schema: schema.clone(),
            name: name.clone(),
            row_count,
            column_count,
        };
        if kind == "VIEW" {
            node.views.push(table);
        } else {
            node.tables.push(table);
        }
    }

    for row in proc_rows {
        let schema = row_get_string(&row, 0, "schema_name")?;
        let name = row_get_string(&row, 1, "proc_name")?;
        tracing::info!(target: "queryben::introspect", loop_ = "procs", %schema, %name, "row");
        if is_system_schema(&schema) {
            continue;
        }
        let node = nodes.entry(schema.clone()).or_insert_with(|| SchemaNode {
            name: schema.clone(),
            tables: Vec::new(),
            views: Vec::new(),
            procedures: Vec::new(),
            functions: Vec::new(),
        });
        node.procedures.push(RoutineInfo {
            schema,
            name,
        });
    }

    for row in fn_rows {
        let schema = row_get_string(&row, 0, "schema_name")?;
        let name = row_get_string(&row, 1, "fn_name")?;
        tracing::info!(target: "queryben::introspect", loop_ = "fns", %schema, %name, "row");
        if is_system_schema(&schema) {
            continue;
        }
        let node = nodes.entry(schema.clone()).or_insert_with(|| SchemaNode {
            name: schema.clone(),
            tables: Vec::new(),
            views: Vec::new(),
            procedures: Vec::new(),
            functions: Vec::new(),
        });
        node.functions.push(RoutineInfo {
            schema,
            name,
        });
    }

    // Deterministic ordering makes the tree stable between refreshes.
    let mut out: Vec<SchemaNode> = nodes.into_values().collect();
    for node in &mut out {
        node.tables.sort_by(|a, b| a.name.cmp(&b.name));
        node.views.sort_by(|a, b| a.name.cmp(&b.name));
        node.procedures.sort_by(|a, b| a.name.cmp(&b.name));
        node.functions.sort_by(|a, b| a.name.cmp(&b.name));
    }
    Ok(out)
}

// Small typed accessors so we don't repeat the `NotFound` boilerplate.
//
// tiberius has a footgun: `try_get::<&str, _>(idx)` can succeed with an empty
// borrow when the row's `sysname`/nvarchar column comes back under a different
// `ColumnData` variant than the `&str` impl matches (varies by tiberius
// version and column collation). Column-name lookup sometimes works when
// index lookup silently misfires, and if both come up empty we reach into the
// raw `ColumnData` for the column and pull the value out under any string-
// shaped variant. Whichever path succeeds we log at info level so this class
// of bug is obvious next time it shows up.
fn row_get_string(row: &Row, idx: usize, col_name: &str) -> Result<String, AppError> {
    // Path 1: &str by index (fast path, no allocation on the tiberius side).
    if let Ok(Some(v)) = row.try_get::<&str, _>(idx) {
        if !v.is_empty() {
            return Ok(v.to_string());
        }
        tracing::info!(
            target: "queryben::introspect",
            idx,
            col_name,
            "&str-by-idx returned empty; trying column-name lookup"
        );
    }
    // Path 2: &str by column name.
    if let Ok(Some(v)) = row.try_get::<&str, _>(col_name) {
        if !v.is_empty() {
            return Ok(v.to_string());
        }
        tracing::info!(
            target: "queryben::introspect",
            idx,
            col_name,
            "&str-by-name returned empty; falling through to raw ColumnData"
        );
    }
    // Path 3: reach into the raw `ColumnData` for the column and pull whatever
    // string-shaped variant it happens to be under. This is the escape hatch
    // for the sysname/nvarchar case where both `try_get::<&str>` paths above
    // come back empty despite the row clearly holding a value.
    if let Some((_col, cell)) = row.cells().nth(idx) {
        if let Some(s) = column_data_as_string(cell) {
            if !s.is_empty() {
                tracing::info!(
                    target: "queryben::introspect",
                    idx,
                    col_name,
                    "recovered from raw ColumnData path"
                );
                return Ok(s);
            }
        }
        tracing::warn!(
            target: "queryben::introspect",
            idx,
            col_name,
            debug = ?cell,
            "column value could not be decoded as string"
        );
    }
    Err(AppError::internal(format!(
        "schema row column {col_name} (idx {idx}) was NULL or unreadable"
    )))
}

// Best-effort string extraction from tiberius' `ColumnData`. Covers the common
// string-shaped variants; anything else falls through to `None`.
fn column_data_as_string(data: &ColumnData<'_>) -> Option<String> {
    match data {
        ColumnData::String(Some(cow)) => Some(cow.to_string()),
        ColumnData::Xml(Some(x)) => Some(x.to_string()),
        ColumnData::Guid(Some(g)) => Some(g.to_string()),
        _ => None,
    }
}

fn row_get_i64(row: &Row, idx: usize) -> Result<i64, AppError> {
    Ok(row
        .try_get::<i64, _>(idx)
        .map_err(AppError::from)?
        .unwrap_or(0))
}

fn row_get_i32(row: &Row, idx: usize) -> Result<i32, AppError> {
    Ok(row
        .try_get::<i32, _>(idx)
        .map_err(AppError::from)?
        .unwrap_or(0))
}

#[tauri::command]
#[specta::specta]
pub async fn get_schema(
    state: State<'_, AppState>,
    connection_id: Uuid,
) -> Result<SchemaInfo, AppError> {
    tracing::info!(target: "queryben::get-schema", %connection_id, "entry");
    let snapshot = state.registry.snapshot(connection_id)?;
    let input = reopen_input(&state, snapshot).await?;
    tracing::info!(target: "queryben::get-schema", %connection_id, "connecting");
    let mut client = mssql::connect_for_connection(&input, connection_id).await?;
    tracing::info!(target: "queryben::get-schema", %connection_id, "connected, introspecting");
    let schemas = introspect_all(&mut client, None).await.map_err(|e| {
        tracing::error!(target: "queryben::get-schema", %connection_id, error = %e, "introspection failed");
        e
    })?;
    tracing::info!(target: "queryben::get-schema", %connection_id, count = schemas.len(), "done");
    state.registry.mark_used(connection_id).ok();
    Ok(SchemaInfo {
        connection_id,
        schemas,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn list_tables(
    state: State<'_, AppState>,
    connection_id: Uuid,
    schema: String,
) -> Result<Vec<TableInfo>, AppError> {
    tracing::info!(target: "queryben::list-tables", %connection_id, %schema);
    let snapshot = state.registry.snapshot(connection_id)?;
    let input = reopen_input(&state, snapshot).await?;
    let mut client = mssql::connect_for_connection(&input, connection_id).await?;
    let nodes = introspect_all(&mut client, Some(&schema)).await?;
    state.registry.mark_used(connection_id).ok();
    // Frontend wants just this schema's tables (not views/procs/fns).
    Ok(nodes
        .into_iter()
        .find(|n| n.name.eq_ignore_ascii_case(&schema))
        .map(|n| n.tables)
        .unwrap_or_default())
}

// Column list for a single table. Combines INFORMATION_SCHEMA.COLUMNS (portable
// bits) with COLUMNPROPERTY / sys.computed_columns for the IDENTITY / computed
// flags that INFORMATION_SCHEMA doesn't expose.
const COLUMNS_SQL: &str = "SELECT c.COLUMN_NAME,
       c.DATA_TYPE,
       c.CHARACTER_MAXIMUM_LENGTH,
       c.NUMERIC_PRECISION,
       c.NUMERIC_SCALE,
       CASE WHEN c.IS_NULLABLE = 'YES' THEN 1 ELSE 0 END AS is_nullable,
       COLUMNPROPERTY(OBJECT_ID(QUOTENAME(c.TABLE_SCHEMA) + '.' + QUOTENAME(c.TABLE_NAME)),
                      c.COLUMN_NAME, 'IsIdentity') AS is_identity,
       COLUMNPROPERTY(OBJECT_ID(QUOTENAME(c.TABLE_SCHEMA) + '.' + QUOTENAME(c.TABLE_NAME)),
                      c.COLUMN_NAME, 'IsComputed') AS is_computed,
       c.COLUMN_DEFAULT,
       c.ORDINAL_POSITION
  FROM INFORMATION_SCHEMA.COLUMNS AS c
 WHERE c.TABLE_SCHEMA = @P1
   AND c.TABLE_NAME = @P2
 ORDER BY c.ORDINAL_POSITION";

// Primary-key columns in key order. sys.indexes.is_primary_key filters to the
// clustered/nonclustered PK; key_ordinal preserves the composite order.
const PK_SQL: &str = "SELECT c.name
  FROM sys.indexes AS i
  JOIN sys.index_columns AS ic
    ON ic.object_id = i.object_id AND ic.index_id = i.index_id
  JOIN sys.columns AS c
    ON c.object_id = ic.object_id AND c.column_id = ic.column_id
  JOIN sys.tables AS t
    ON t.object_id = i.object_id
  JOIN sys.schemas AS s
    ON s.schema_id = t.schema_id
 WHERE i.is_primary_key = 1
   AND s.name = @P1
   AND t.name = @P2
 ORDER BY ic.key_ordinal";

// Renders "nvarchar(50)" / "decimal(18,4)" / "int" from the raw DATA_TYPE +
// length/precision/scale INFORMATION_SCHEMA hands back. Length -1 = MAX.
fn format_sql_type(
    data_type: &str,
    char_len: Option<i32>,
    numeric_precision: Option<i32>,
    numeric_scale: Option<i32>,
) -> String {
    let dt = data_type.to_ascii_lowercase();
    match dt.as_str() {
        "char" | "varchar" | "nchar" | "nvarchar" | "binary" | "varbinary" => match char_len {
            Some(-1) => format!("{dt}(max)"),
            Some(n) if n > 0 => format!("{dt}({n})"),
            _ => dt,
        },
        "decimal" | "numeric" => match (numeric_precision, numeric_scale) {
            (Some(p), Some(s)) => format!("{dt}({p},{s})"),
            (Some(p), None) => format!("{dt}({p})"),
            _ => dt,
        },
        _ => dt,
    }
}

#[tauri::command]
#[specta::specta]
pub async fn get_table_metadata(
    state: State<'_, AppState>,
    connection_id: Uuid,
    schema: String,
    name: String,
) -> Result<TableMetadata, AppError> {
    tracing::info!(
        target: "queryben::get-table-metadata",
        %connection_id,
        %schema,
        %name,
        "entry"
    );

    let snapshot = state.registry.snapshot(connection_id)?;
    let input = reopen_input(&state, snapshot).await?;
    let mut client = mssql::connect_for_connection(&input, connection_id).await?;

    // --- columns ---
    let mut col_q = Query::new(COLUMNS_SQL);
    col_q.bind(schema.clone());
    col_q.bind(name.clone());
    let col_rows = col_q.query(&mut client).await?.into_first_result().await?;

    let mut columns: Vec<TableColumn> = Vec::with_capacity(col_rows.len());
    for row in col_rows {
        let col_name = row_get_string(&row, 0, "COLUMN_NAME")?;
        let data_type = row_get_string(&row, 1, "DATA_TYPE")?;
        let char_len = row.try_get::<i32, _>(2).map_err(AppError::from)?;
        // NUMERIC_PRECISION comes back as u8, NUMERIC_SCALE as i32 in tiberius.
        let numeric_precision = row
            .try_get::<u8, _>(3)
            .map_err(AppError::from)?
            .map(i32::from);
        let numeric_scale = row.try_get::<i32, _>(4).map_err(AppError::from)?;
        let is_nullable = row_get_i32(&row, 5)? != 0;
        // COLUMNPROPERTY returns int; NULL possible if the object_id lookup
        // fails (shouldn't for a real table, but stay defensive).
        let is_identity = row.try_get::<i32, _>(6).map_err(AppError::from)?.unwrap_or(0) != 0;
        let is_computed = row.try_get::<i32, _>(7).map_err(AppError::from)?.unwrap_or(0) != 0;
        let default_expression = row
            .try_get::<&str, _>(8)
            .map_err(AppError::from)?
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty());
        let ordinal = row_get_i32(&row, 9)?.max(0) as u32;

        columns.push(TableColumn {
            name: col_name,
            sql_type: format_sql_type(&data_type, char_len, numeric_precision, numeric_scale),
            is_nullable,
            is_identity,
            is_computed,
            default_expression,
            ordinal,
        });
    }

    // --- primary key ---
    let mut pk_q = Query::new(PK_SQL);
    pk_q.bind(schema.clone());
    pk_q.bind(name.clone());
    let pk_rows = pk_q.query(&mut client).await?.into_first_result().await?;
    let mut primary_key: Vec<String> = Vec::with_capacity(pk_rows.len());
    for row in pk_rows {
        primary_key.push(row_get_string(&row, 0, "name")?);
    }

    state.registry.mark_used(connection_id).ok();

    let is_editable = !primary_key.is_empty();
    tracing::info!(
        target: "queryben::get-table-metadata",
        %connection_id,
        %schema,
        %name,
        columns = columns.len(),
        pk_cols = primary_key.len(),
        is_editable,
        "done"
    );

    Ok(TableMetadata {
        schema,
        name,
        is_editable,
        primary_key,
        columns,
    })
}

// Hard cap so a runaway frontend can't ship a million-statement transaction
// and lock the connection. 500 is well above any reasonable batch of pending
// edits from a single grid session.
const MAX_TRANSACTION_STATEMENTS: usize = 500;

#[tauri::command]
#[specta::specta]
pub async fn execute_transaction(
    state: State<'_, AppState>,
    connection_id: Uuid,
    statements: Vec<String>,
) -> Result<TransactionResult, AppError> {
    let statement_count = statements.len();
    tracing::info!(
        target: "queryben::execute-transaction",
        %connection_id,
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

    let snapshot = state.registry.snapshot(connection_id)?;
    let input = reopen_input(&state, snapshot).await?;
    let mut client = mssql::connect_for_connection(&input, connection_id).await?;

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

    state.registry.mark_used(connection_id).ok();

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
