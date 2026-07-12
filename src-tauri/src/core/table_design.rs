//! Table Designer domain types.
//!
//! Wire shape mirrors the frontend `TableDesign` in
//! `src/features/table-designer/`. `TableDesign` is the full editable shape:
//! columns, PK, non-PK indexes, and FKs. The provider consumes two of them —
//! `current` (what's on the server today, or `None` for a new table) and
//! `next` (what the user wants) — and returns a list of DDL statements the
//! user reviews before hitting Apply.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DesignColumn {
    pub name: String,
    // Full rendered type — "int", "nvarchar(255)", "decimal(18,4)".
    // Editor stores the raw string; provider re-emits it into the DDL.
    pub sql_type: String,
    pub is_nullable: bool,
    pub is_identity: bool,
    pub is_computed: bool,
    // For computed columns; ignored when is_computed = false.
    pub computed_expression: Option<String>,
    // For non-computed columns; ignored when is_computed = true.
    pub default_expression: Option<String>,
    pub ordinal: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DesignIndex {
    pub name: String,
    pub is_unique: bool,
    pub columns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DesignForeignKey {
    pub name: String,
    pub columns: Vec<String>,
    pub referenced_schema: String,
    pub referenced_table: String,
    pub referenced_columns: Vec<String>,
    // Optional ON DELETE / ON UPDATE actions. Provider skips the clause when
    // None so the server uses its default (NO ACTION).
    pub on_delete: Option<String>,
    pub on_update: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TableDesign {
    pub schema: String,
    pub name: String,
    pub columns: Vec<DesignColumn>,
    // Composite PK: columns listed in order. Empty = no PK. `pk_name` names
    // the constraint (server auto-generates one when None).
    pub primary_key: Vec<String>,
    pub pk_name: Option<String>,
    // Non-PK indexes only. PK is expressed via primary_key + pk_name.
    pub indexes: Vec<DesignIndex>,
    pub foreign_keys: Vec<DesignForeignKey>,
}

// One row in the generated DDL preview. Same shape as schema_diff::DdlStatement
// but scoped to the designer so we don't cross-import a compare-only type.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DdlStatement {
    // "CREATE" | "ALTER" | "DROP"
    pub kind: String,
    // Human label for the badge in the preview pane.
    pub label: String,
    pub sql: String,
}
