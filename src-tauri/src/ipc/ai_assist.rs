//! AI query-assistant commands. Thin wrappers over the `AiProvider` trait
//! that also stitch in the active connection's schema as system context so
//! generated SQL references real tables instead of hallucinated ones.

use std::fmt::Write as _;

use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

use crate::core::schema::SchemaInfo;
use crate::error::AppError;
use crate::adapters::ai_config::AiConfig;
use crate::adapters::ai_provider::{build_provider, SessionContext};
use crate::state::AppState;

/// Build a compact schema summary for the system prompt. Format is one line
/// per column (`schema.table.column: type`) because token cost per row is
/// low and the model needs the types to pick sensible casts.
fn format_schema(schema: &SchemaInfo, max_chars: usize) -> String {
    let mut out = String::new();
    out.push_str("Active database schema:\n");
    'outer: for node in &schema.schemas {
        for table in &node.tables {
            let cols = table
                .column_count
                .map(|c| format!(" ({c} cols)"))
                .unwrap_or_default();
            let rows = table
                .row_count
                .map(|r| format!(" ~{r} rows"))
                .unwrap_or_default();
            let line = format!("- {}.{}{}{}\n", node.name, table.name, cols, rows);
            if out.len() + line.len() > max_chars {
                let _ = writeln!(out, "... (truncated at {max_chars} chars)");
                break 'outer;
            }
            out.push_str(&line);
        }
        for view in &node.views {
            let line = format!("- {}.{} (view)\n", node.name, view.name);
            if out.len() + line.len() > max_chars {
                let _ = writeln!(out, "... (truncated at {max_chars} chars)");
                break 'outer;
            }
            out.push_str(&line);
        }
    }
    out
}

const SQL_SYSTEM_PREAMBLE: &str = "You are a SQL assistant for Microsoft SQL Server / Azure SQL. \
When the user asks a natural-language question about their data, respond with a SQL query in a ```sql code block, \
followed by one short sentence explaining what it does. Use only the tables listed below. \
If the request is ambiguous, ask a single clarifying question instead of guessing.\n\n";

#[tauri::command]
#[specta::specta]
pub async fn ai_new_session(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: Uuid,
) -> Result<String, AppError> {
    tracing::info!(target: "queryben::ai", %connection_id, "ai_new_session");

    let cfg = match app.path().app_data_dir() {
        Ok(dir) => AiConfig::load_or_default(&dir),
        Err(_) => AiConfig::default(),
    };

    let mut system = String::from(SQL_SYSTEM_PREAMBLE);
    if cfg.include_schema_in_context {
        // Best-effort: if introspection fails (bad connection, dead socket)
        // we still open a session — the assistant just won't have schema
        // grounding for this turn. The user gets an obvious "no tables"
        // response from the model instead of a hard error dead-end.
        match crate::ipc::query::get_schema(state, connection_id).await {
            Ok(schema) => {
                system.push_str(&format_schema(&schema, cfg.max_schema_chars));
            }
            Err(err) => {
                tracing::warn!(
                    target: "queryben::ai",
                    %connection_id,
                    error = %err,
                    "schema introspection failed; opening session without schema context"
                );
                system.push_str("(schema unavailable — connection could not be introspected)\n");
            }
        }
    }

    let provider = build_provider(&cfg);
    provider
        .new_session(SessionContext { system_prompt: system })
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn ai_complete(
    app: AppHandle,
    session_id: String,
    prompt: String,
) -> Result<String, AppError> {
    tracing::info!(
        target: "queryben::ai",
        session_id = %session_id,
        prompt_len = prompt.len(),
        "ai_complete"
    );

    let cfg = match app.path().app_data_dir() {
        Ok(dir) => AiConfig::load_or_default(&dir),
        Err(_) => AiConfig::default(),
    };

    let provider = build_provider(&cfg);
    provider.complete(&session_id, &prompt).await
}
