//! Saved queries + query history domain types.
//!
//! Two shapes ship over the wire (both serde `camelCase` so the TS invoke
//! wrappers land on the fields the React screens want):
//!   - `SavedQuery` — user-authored, named, foldered, editable.
//!   - `HistoryEntry` — auto-logged execution record (SQL + connection + time +
//!     row count + duration + optional error).
//!
//! Filter types are separate structs (not enums) so the frontend can send
//! partial JSON — every field is `Option<T>` — and the backend fills in the
//! default when a filter isn't set. No `tauri` / `rusqlite` imports here; the
//! infra layer maps these to/from rows.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---- Saved queries ---------------------------------------------------------

/// One saved query row.
///
/// `folder` is a plain string (not a nested tree) so we can render a virtual
/// folder tree by grouping in the UI without maintaining a second table. The
/// default folder comes from `queries.config.json > savedQueriesDefaultFolder`
/// (falls back to "General" if the file is missing).
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SavedQuery {
    pub id: Uuid,
    pub name: String,
    pub folder: String,
    pub sql: String,
    pub connection_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Filter passed to `list_saved_queries`. All fields optional — an empty
/// filter returns everything sorted by (folder ASC, name ASC).
#[derive(Debug, Clone, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SavedQueryFilter {
    /// Substring match against `name` (case-insensitive). Empty string is
    /// treated as "no filter", not "match empty name".
    #[serde(default)]
    pub search: Option<String>,
    /// Exact folder match. `None` includes every folder.
    #[serde(default)]
    pub folder: Option<String>,
    /// Restrict to queries bound to this connection.
    #[serde(default)]
    pub connection_id: Option<Uuid>,
}

// ---- Query history ---------------------------------------------------------

/// One row in the query-history table. Written on every executed query when
/// `queries.config.json > autoLogHistory` is true (default).
///
/// `error` is `Some` iff the execution surfaced a backend `AppError`. We store
/// the human-readable message only, not the tag — the frontend just renders
/// the message next to a red dot; a future "re-diagnose" flow can look up
/// details from the connection.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: Uuid,
    pub sql: String,
    pub connection_id: Option<Uuid>,
    pub executed_at: DateTime<Utc>,
    /// Total rows across all result sets. `None` when the query errored before
    /// the first row was produced.
    pub row_count: Option<u64>,
    /// Wall-clock time as observed by the frontend (`Date.now()` delta around
    /// the `executeQuery` invoke). Milliseconds, `None` when the frontend
    /// couldn't measure it (very rare, but keep the column nullable so we
    /// can't crash on a missing value).
    pub duration_ms: Option<u64>,
    /// `Some` iff the execute call rejected. Human-readable one-liner.
    pub error: Option<String>,
}

/// Filter for `list_query_history`. Default returns the newest
/// `HistoryConfig::max_rows` rows, ordered `executed_at DESC`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct HistoryFilter {
    /// Substring match against `sql` (case-insensitive). Empty = no filter.
    #[serde(default)]
    pub search: Option<String>,
    /// Restrict to executions on this connection.
    #[serde(default)]
    pub connection_id: Option<Uuid>,
    /// Cap the returned rows. Falls back to config `historyMaxRows` when
    /// `None`.
    #[serde(default)]
    pub limit: Option<u32>,
}
