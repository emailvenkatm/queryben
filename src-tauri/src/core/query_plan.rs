//! Query plan tree returned to the frontend visualizer.

use serde::Serialize;

/// Root wrapper — carries the tree plus any batch-level warnings the parser
/// couldn't attach to a specific node (missing indexes surfaced at the batch
/// level by SQL Server, etc.).
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct QueryPlan {
    pub statement_text: Option<String>,
    pub root: PlanNode,
    pub warnings: Vec<PlanWarning>,
    pub is_actual: bool,
}

/// One operator in the execution tree. Recursive via `children`.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanNode {
    pub id: u32,
    pub name: String,
    pub op_kind: OpKind,
    pub estimated_rows: Option<f64>,
    pub actual_rows: Option<f64>,
    pub estimated_cost: Option<f64>,
    pub warnings: Vec<PlanWarning>,
    pub object: Option<String>,
    pub children: Vec<PlanNode>,
}

/// Broad operator category so the frontend icon picker doesn't have to string-
/// match dozens of showplan names. Unknown falls through to a generic icon.
#[derive(Debug, Clone, Copy, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum OpKind {
    TableScan,
    IndexScan,
    IndexSeek,
    Sort,
    HashMatch,
    NestedLoops,
    MergeJoin,
    ComputeScalar,
    Filter,
    Aggregate,
    Parallelism,
    Spool,
    Unknown,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanWarning {
    pub kind: WarningKind,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum WarningKind {
    MissingIndex,
    LargeScan,
    ImplicitConversion,
    NoJoinPredicate,
    Other,
}
