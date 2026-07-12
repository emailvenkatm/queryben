//! Schema-compare domain types.
//!
//! `SchemaSnapshot` is the engine-agnostic "here is everything the object
//! explorer would show, plus enough shape to diff". `SchemaDiff` is the result
//! of diffing two snapshots (source vs target). `DdlStatement` is one row in
//! the generated migration script.
//!
//! Snapshots are round-trippable JSON — the frontend hands them back to the
//! diff/DDL commands so we don't have to re-introspect on every user click.

use serde::{Deserialize, Serialize};

// String enum on the wire ("table", "view", "procedure", "function", "index"),
// matches the includeObjectKinds config filter values verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum ObjectKind {
    Table,
    View,
    Procedure,
    Function,
    Index,
}

impl ObjectKind {
    pub fn as_config_str(&self) -> &'static str {
        match self {
            ObjectKind::Table => "table",
            ObjectKind::View => "view",
            ObjectKind::Procedure => "procedure",
            ObjectKind::Function => "function",
            ObjectKind::Index => "index",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ColumnSpec {
    pub name: String,
    pub sql_type: String,
    pub is_nullable: bool,
    pub is_identity: bool,
    pub is_computed: bool,
    pub default_expression: Option<String>,
    pub ordinal: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct IndexSpec {
    pub name: String,
    pub is_unique: bool,
    pub is_primary_key: bool,
    pub columns: Vec<String>,
}

// One object in a schema snapshot. Tables carry their columns + indexes;
// views/procs/functions carry their body text so the DDL panel can render a
// side-by-side diff of the source. `body` is None for tables (their shape is
// the columns + indexes, not a single DDL string).
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SchemaObject {
    pub kind: ObjectKind,
    pub schema: String,
    pub name: String,
    // Full-qualified name "schema.name" — cached so the frontend doesn't have
    // to reconstruct it in every list render.
    pub qualified_name: String,
    #[serde(default)]
    pub columns: Vec<ColumnSpec>,
    #[serde(default)]
    pub indexes: Vec<IndexSpec>,
    #[serde(default)]
    pub body: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SchemaSnapshot {
    // Free-form label the UI shows above the tree ("prod-2024-06-15 14:30").
    pub label: String,
    pub captured_at: String, // ISO-8601
    // The connection UUID the snapshot was taken against, stringified so the
    // frontend can round-trip without care about UUID vs string.
    pub connection_id: String,
    // The engine used to introspect. Today always "mssql"; leaves room for
    // "mysql" / "postgres" without a struct rev.
    pub engine: String,
    pub objects: Vec<SchemaObject>,
}

// One difference between source and target for a single object identity. The
// `source` / `target` fields carry the full spec on each side — `added` has
// source=Some, target=None; `dropped` is the reverse; `changed` has both.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ObjectChange {
    pub kind: ObjectKind,
    pub qualified_name: String,
    pub source: Option<SchemaObject>,
    pub target: Option<SchemaObject>,
    // Human-readable reasons the diff engine flagged this as "changed" (empty
    // for added/dropped rows). One line per material difference: "column
    // 'foo' type: int -> bigint", "index 'ix_bar' dropped", etc.
    #[serde(default)]
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SchemaDiff {
    pub source_label: String,
    pub target_label: String,
    // Objects that exist on source but not target — target needs a CREATE.
    pub added: Vec<ObjectChange>,
    // Objects that exist on target but not source — target needs a DROP.
    pub dropped: Vec<ObjectChange>,
    // Objects that exist on both but have material differences.
    pub changed: Vec<ObjectChange>,
    // Objects present on both with no differences. Kept in the wire shape so
    // the UI can render a muted "identical (N)" row without a second call.
    pub unchanged_count: u32,
}

// One row in the generated migration script. `kind` is a string tag so the UI
// can badge each row without another type import.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DdlStatement {
    pub object_kind: ObjectKind,
    pub object_name: String,
    // "CREATE" | "ALTER" | "DROP"
    pub kind: String,
    pub sql: String,
}

// Options loaded from `<app_data_dir>/schema-compare.config.json`. All fields
// have defaults, so a missing file behaves the same as `{}`.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SchemaCompareOptions {
    #[serde(default = "default_true")]
    pub ignore_collation: bool,
    #[serde(default = "default_true")]
    pub ignore_fillfactor: bool,
    #[serde(default = "default_true")]
    pub ignore_whitespace: bool,
    #[serde(default = "default_object_kinds")]
    pub include_object_kinds: Vec<String>,
}

impl Default for SchemaCompareOptions {
    fn default() -> Self {
        Self {
            ignore_collation: true,
            ignore_fillfactor: true,
            ignore_whitespace: true,
            include_object_kinds: default_object_kinds(),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_object_kinds() -> Vec<String> {
    vec![
        "table".into(),
        "view".into(),
        "procedure".into(),
        "function".into(),
        "index".into(),
    ]
}

impl SchemaCompareOptions {
    pub fn includes(&self, kind: &ObjectKind) -> bool {
        let want = kind.as_config_str();
        self.include_object_kinds
            .iter()
            .any(|k| k.eq_ignore_ascii_case(want))
    }
}
