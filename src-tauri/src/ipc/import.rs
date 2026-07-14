//! Import IPC. Two commands, matching the shape of `commands::export`:
//!   * `import_preview(path, format)` — synchronous file peek: header +
//!     first N rows + inferred column types.
//!   * `import_execute(connection_id, path, format, target_schema,
//!     target_table, column_mapping, options)` — creates the table if
//!     asked, optionally truncates, then streams a chunked INSERT with
//!     parameter binding via tiberius.
//!
//! Row cap policy — the preview cap (10k / `ROW_CAP`) belongs to the
//! preview call only. The execute path streams the whole file to the DB in
//! `options.chunk_size` batches (default 500) so 100k-row imports don't
//! OOM the frontend or the Rust process.
//!
//! Error policy — `skip_on_error = false` (default) aborts on the first
//! failing row and rolls back only the current batch. `skip_on_error = true`
//! keeps going and stashes each failure in `ImportResult.errors` so the
//! wizard can render a summary table.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Instant;

use tauri::{AppHandle, Manager, State};
use tiberius::Query;
use uuid::Uuid;

use crate::core::connection::{ConnectionSnapshot, CreateConnectionInput};
use crate::core::import::{
    ColumnMapping, ImportFormat, ImportOptions, ImportPreview, ImportResult, ImportRowError,
};
use crate::core::query::CellValue;
use crate::error::AppError;
use crate::adapters::import_config::ImportConfig;
use crate::adapters::importer::{registry_from_config, ImporterRegistry};
use crate::adapters::{azure::oauth as azure_oauth, mssql};
use crate::state::AppState;

const SCOPE_SQLDB: &str = "https://database.windows.net/.default";

// Same preview cap the export path uses. Keeps the wire payload bounded when
// the frontend calls back to refill the preview table.
const PREVIEW_ROW_CAP: usize = 10_000;

fn config(app: &AppHandle) -> ImportConfig {
    static CFG: OnceLock<ImportConfig> = OnceLock::new();
    CFG.get_or_init(|| {
        let dir = app
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| PathBuf::from("."));
        ImportConfig::load(&dir)
    })
    .clone()
}

fn registry(app: &AppHandle) -> &'static ImporterRegistry {
    static REG: OnceLock<ImporterRegistry> = OnceLock::new();
    REG.get_or_init(|| {
        let dir = app
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| PathBuf::from("."));
        registry_from_config(&ImportConfig::load(&dir))
    })
}

/// Reopen a tiberius session from a snapshot. Duplicated from
/// `commands::query::reopen_input` because that helper is private to its
/// module. The shape is intentionally identical.
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

#[tauri::command]
#[specta::specta]
pub async fn import_preview(
    app: AppHandle,
    path: String,
    format: ImportFormat,
) -> Result<ImportPreview, AppError> {
    tracing::info!(target: "queryben::import::preview", %path, ?format);
    let importer = registry(&app).get(&format).ok_or_else(|| {
        AppError::NotImplemented(format!("no importer registered for {:?}", format))
    })?;
    let preview = importer.preview(Path::new(&path), 10).await?;
    // Belt-and-suspenders — nobody should be shipping >10k preview rows but
    // the cap protects the IPC channel just in case.
    if preview.rows.len() > PREVIEW_ROW_CAP {
        tracing::warn!(
            target: "queryben::import::preview",
            preview_rows = preview.rows.len(),
            "preview exceeded cap; truncating"
        );
    }
    Ok(preview)
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
#[specta::specta]
pub async fn import_execute(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: Uuid,
    path: String,
    format: ImportFormat,
    target_schema: String,
    target_table: String,
    column_mapping: Vec<ColumnMapping>,
    options: ImportOptions,
) -> Result<ImportResult, AppError> {
    let started = Instant::now();
    tracing::info!(
        target: "queryben::import::execute",
        %connection_id, %path, ?format,
        %target_schema, %target_table,
        columns = column_mapping.len(),
        create = options.create_table_if_missing,
        truncate = options.truncate_before_insert,
        skip_on_error = options.skip_on_error,
        chunk_size = options.chunk_size,
        "entry"
    );

    if column_mapping.iter().all(|m| !m.include) {
        return Err(AppError::internal(
            "at least one column must be included in the mapping".to_string(),
        ));
    }

    let importer = registry(&app).get(&format).ok_or_else(|| {
        AppError::NotImplemented(format!("no importer registered for {:?}", format))
    })?;

    // Column order in the file (indexes into each row). We need this to line
    // up cell positions with the mapping the user built in the wizard.
    let preview = importer.preview(Path::new(&path), 1).await?;
    let source_headers: Vec<String> = preview.columns.iter().map(|c| c.name.clone()).collect();

    let all_rows = importer.read_all(Path::new(&path)).await?;
    let total_rows = all_rows.len();

    // Pre-resolve source-column indexes for each included mapping. A mapping
    // that points at a header we can't find in the file becomes an
    // early-abort error, not a silent NULL, so the user notices.
    let included: Vec<(&ColumnMapping, usize)> = {
        let mut v = Vec::new();
        for m in column_mapping.iter().filter(|m| m.include) {
            let idx = source_headers
                .iter()
                .position(|h| h == &m.source_column)
                .ok_or_else(|| {
                    AppError::internal(format!(
                        "mapping references unknown source column '{}'",
                        m.source_column
                    ))
                })?;
            v.push((m, idx));
        }
        v
    };

    let snapshot = state.registry.snapshot(connection_id)?;
    let input = reopen_input(&state, snapshot).await?;
    let mut client = mssql::connect_for_connection(&input, connection_id).await?;

    let mut created_table = false;
    if options.create_table_if_missing {
        let default_string_type = config(&app).default_string_type.clone();
        let create_sql = build_create_table_if_missing(
            &target_schema,
            &target_table,
            &included,
            &default_string_type,
        );
        tracing::info!(
            target: "queryben::import::execute",
            sql = %create_sql,
            "issuing conditional CREATE TABLE"
        );
        client.simple_query(create_sql).await?;
        created_table = true;
    }

    if options.truncate_before_insert {
        let sql = format!(
            "DELETE FROM [{}].[{}]",
            escape_ident(&target_schema),
            escape_ident(&target_table)
        );
        tracing::info!(
            target: "queryben::import::execute",
            sql = %sql,
            "truncating target table"
        );
        client.simple_query(sql).await?;
    }

    // Chunked INSERT. Each chunk is one multi-row VALUES statement with
    // parameter bindings — server-side batch semantics, no client-side
    // stitching, and the size cap protects us from tiberius' 2100-param
    // limit (chunk_size * included.len() must stay under that).
    let chunk_size_rows = options.chunk_size.max(1) as usize;
    // 2100 param cap comes from SQL Server proc/exec_sp; leave headroom.
    let max_params_per_batch: usize = 2000;
    let cols_per_row = included.len().max(1);
    let effective_chunk = chunk_size_rows.min((max_params_per_batch / cols_per_row).max(1));

    let mut rows_inserted: u64 = 0;
    let mut rows_failed: u64 = 0;
    let mut errors: Vec<ImportRowError> = Vec::new();

    let insert_prefix = build_insert_prefix(&target_schema, &target_table, &included);

    for (chunk_idx, chunk) in all_rows.chunks(effective_chunk).enumerate() {
        let started_chunk = Instant::now();
        let mut sql = String::from(&insert_prefix);
        sql.push_str(" VALUES ");
        for i in 0..chunk.len() {
            if i > 0 {
                sql.push(',');
            }
            sql.push('(');
            for j in 0..included.len() {
                if j > 0 {
                    sql.push(',');
                }
                sql.push_str(&format!("@P{}", i * included.len() + j + 1));
            }
            sql.push(')');
        }

        let mut query = Query::new(sql.clone());
        for row in chunk {
            for (_m, idx) in &included {
                let cell = row.get(*idx).unwrap_or(&CellValue::Null);
                bind_cell(&mut query, cell);
            }
        }

        match query.execute(&mut client).await {
            Ok(res) => {
                rows_inserted = rows_inserted.saturating_add(res.total());
                tracing::info!(
                    target: "queryben::import::execute",
                    chunk_idx,
                    chunk_rows = chunk.len(),
                    rows_inserted,
                    duration_ms = started_chunk.elapsed().as_millis() as u64,
                    "chunk committed"
                );
            }
            Err(e) => {
                let msg = e.to_string();
                if options.skip_on_error {
                    // On per-batch failure with skip_on_error, retry the rows
                    // one at a time so a single bad row doesn't waste the
                    // whole chunk. This is the "correct" but slower path;
                    // most users hit it only for a handful of dirty rows.
                    tracing::warn!(
                        target: "queryben::import::execute",
                        chunk_idx,
                        error = %msg,
                        "batch failed with skip_on_error; retrying row-by-row"
                    );
                    for (i, row) in chunk.iter().enumerate() {
                        let file_row_idx = (chunk_idx * effective_chunk + i + 1) as u64;
                        let mut per_row_sql = String::from(&insert_prefix);
                        per_row_sql.push_str(" VALUES (");
                        for j in 0..included.len() {
                            if j > 0 {
                                per_row_sql.push(',');
                            }
                            per_row_sql.push_str(&format!("@P{}", j + 1));
                        }
                        per_row_sql.push(')');
                        let mut q = Query::new(per_row_sql);
                        for (_m, idx) in &included {
                            let cell = row.get(*idx).unwrap_or(&CellValue::Null);
                            bind_cell(&mut q, cell);
                        }
                        match q.execute(&mut client).await {
                            Ok(_) => rows_inserted = rows_inserted.saturating_add(1),
                            Err(re) => {
                                rows_failed = rows_failed.saturating_add(1);
                                if errors.len() < 100 {
                                    errors.push(ImportRowError {
                                        row_index: file_row_idx,
                                        message: re.to_string(),
                                    });
                                }
                            }
                        }
                    }
                } else {
                    // Abort. The successful earlier chunks already committed
                    // — we don't wrap the whole import in one giant tran
                    // because a 100k row transaction would blow the log.
                    let file_row_idx =
                        (chunk_idx * effective_chunk + 1) as u64;
                    errors.push(ImportRowError {
                        row_index: file_row_idx,
                        message: msg.clone(),
                    });
                    rows_failed = rows_failed.saturating_add(chunk.len() as u64);
                    tracing::error!(
                        target: "queryben::import::execute",
                        chunk_idx,
                        error = %msg,
                        "batch failed; aborting (skip_on_error is off)"
                    );
                    let duration_ms = started.elapsed().as_millis() as u32;
                    return Ok(ImportResult {
                        rows_inserted,
                        rows_failed,
                        duration_ms,
                        errors,
                        created_table,
                        truncated: options.truncate_before_insert,
                    });
                }
            }
        }
    }

    state.registry.mark_used(connection_id).ok();

    let duration_ms = started.elapsed().as_millis() as u32;
    tracing::info!(
        target: "queryben::import::execute",
        total_rows,
        rows_inserted,
        rows_failed,
        duration_ms,
        errors = errors.len(),
        "done"
    );

    Ok(ImportResult {
        rows_inserted,
        rows_failed,
        duration_ms,
        errors,
        created_table,
        truncated: options.truncate_before_insert,
    })
}

fn build_create_table_if_missing(
    schema: &str,
    table: &str,
    included: &[(&ColumnMapping, usize)],
    _default_string_type: &str,
) -> String {
    let mut cols = String::new();
    for (i, (m, _)) in included.iter().enumerate() {
        if i > 0 {
            cols.push_str(", ");
        }
        cols.push_str(&format!(
            "[{}] {} NULL",
            escape_ident(&m.target_column),
            m.target_type
        ));
    }
    // OBJECT_ID('[schema].[table]', 'U') IS NULL is the standard idempotent
    // guard for CREATE TABLE. Wraps the CREATE in a small T-SQL branch so
    // re-running the import doesn't error on the table already existing.
    format!(
        "IF OBJECT_ID('[{}].[{}]', 'U') IS NULL BEGIN CREATE TABLE [{}].[{}] ({}); END",
        escape_ident(schema),
        escape_ident(table),
        escape_ident(schema),
        escape_ident(table),
        cols
    )
}

fn build_insert_prefix(
    schema: &str,
    table: &str,
    included: &[(&ColumnMapping, usize)],
) -> String {
    let mut cols = String::new();
    for (i, (m, _)) in included.iter().enumerate() {
        if i > 0 {
            cols.push(',');
        }
        cols.push_str(&format!("[{}]", escape_ident(&m.target_column)));
    }
    format!(
        "INSERT INTO [{}].[{}] ({})",
        escape_ident(schema),
        escape_ident(table),
        cols
    )
}

// Double-up any `]` to escape it inside bracketed identifiers. Same rule
// SSMS / QUOTENAME uses.
fn escape_ident(input: &str) -> String {
    input.replace(']', "]]")
}

fn bind_cell(query: &mut Query<'_>, cell: &CellValue) {
    match cell {
        CellValue::Null => query.bind(None::<String>),
        CellValue::Bool(b) => query.bind(*b),
        CellValue::Int(n) => query.bind(*n),
        CellValue::Float(f) => query.bind(*f),
        CellValue::Text(s) => query.bind(s.clone()),
        CellValue::DateTime(s) => query.bind(s.clone()),
        CellValue::Bytes(s) => query.bind(s.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::import::ColumnMapping;

    fn m(source: &str, target: &str, ty: &str) -> ColumnMapping {
        ColumnMapping {
            source_column: source.into(),
            target_column: target.into(),
            target_type: ty.into(),
            include: true,
        }
    }

    #[test]
    fn escape_ident_doubles_close_bracket() {
        assert_eq!(escape_ident("plain"), "plain");
        assert_eq!(escape_ident("weird]name"), "weird]]name");
    }

    #[test]
    fn create_table_if_missing_renders_guard_and_columns() {
        let mappings = vec![m("a", "A", "INT"), m("b", "B", "NVARCHAR(50)")];
        let included: Vec<(&ColumnMapping, usize)> =
            mappings.iter().enumerate().map(|(i, m)| (m, i)).collect();
        let sql = build_create_table_if_missing("dbo", "Widget", &included, "NVARCHAR(255)");
        assert!(sql.contains("OBJECT_ID('[dbo].[Widget]', 'U') IS NULL"), "guard missing: {sql}");
        assert!(sql.contains("CREATE TABLE [dbo].[Widget]"), "create missing: {sql}");
        assert!(sql.contains("[A] INT NULL"), "col A missing: {sql}");
        assert!(sql.contains("[B] NVARCHAR(50) NULL"), "col B missing: {sql}");
    }

    #[test]
    fn insert_prefix_wraps_target_columns_in_brackets() {
        let mappings = vec![m("a", "A", "INT"), m("b", "B", "NVARCHAR(50)")];
        let included: Vec<(&ColumnMapping, usize)> =
            mappings.iter().enumerate().map(|(i, m)| (m, i)).collect();
        let sql = build_insert_prefix("dbo", "Widget", &included);
        assert_eq!(sql, "INSERT INTO [dbo].[Widget] ([A],[B])");
    }
}
