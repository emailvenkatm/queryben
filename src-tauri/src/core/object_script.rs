//! Object-scripter domain types.
//!
//! Wire shape for the SSMS-parity "Script as" flow. `ObjectKind` names the
//! kind of schema object the user right-clicked; `ScriptAction` names the
//! variant of DDL/DML they picked from the submenu. Both round-trip through
//! specta so the frontend's discriminated union stays honest.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ObjectKind {
    Table,
    View,
    Procedure,
    Function,
    Index,
}

impl ObjectKind {
    /// SQL keyword for DROP / CREATE statements. Index is intentionally absent
    /// from this list — DROP INDEX uses a different shape (`ON [t]`) than the
    /// other object kinds, so callers must branch.
    pub fn ddl_keyword(&self) -> &'static str {
        match self {
            ObjectKind::Table => "TABLE",
            ObjectKind::View => "VIEW",
            ObjectKind::Procedure => "PROCEDURE",
            ObjectKind::Function => "FUNCTION",
            ObjectKind::Index => "INDEX",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ScriptAction {
    Create,
    Alter,
    Drop,
    DropAndCreate,
    SelectTop,
    InsertTemplate,
}

/// A qualified schema-object reference the user right-clicked on.
///
/// `table` is only meaningful when `kind = Index` — indexes are DROP'd via
/// `ON [schema].[table]`. Everything else ignores it.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SchemaObjectRef {
    pub kind: ObjectKind,
    pub schema: String,
    pub name: String,
    pub table: Option<String>,
}
