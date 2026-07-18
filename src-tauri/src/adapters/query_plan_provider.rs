//! Per-engine query-plan capture. Only SQL Server is wired; MySQL and Postgres
//! keep unimplemented impls so the trait registry stays whole.

use async_trait::async_trait;
use futures_util::TryStreamExt;
use quick_xml::events::Event;
use quick_xml::Reader;
use tiberius::{Query, QueryItem};

use crate::core::query_plan::{OpKind, PlanNode, PlanWarning, QueryPlan, WarningKind};
use crate::error::AppError;
use crate::adapters::mssql::MssqlClient;

pub struct PlanCaptureOptions {
    pub show_estimated_only: bool,
    pub warn_on_scan_rows_above: f64,
    pub warn_on_missing_index: bool,
}

impl Default for PlanCaptureOptions {
    fn default() -> Self {
        Self {
            show_estimated_only: true,
            warn_on_scan_rows_above: 100_000.0,
            warn_on_missing_index: true,
        }
    }
}

#[async_trait]
pub trait QueryPlanProvider: Send + Sync {
    async fn capture_plan(
        &self,
        client: &mut MssqlClient,
        sql: &str,
        opts: &PlanCaptureOptions,
    ) -> Result<QueryPlan, AppError>;
}

pub struct SqlServerQueryPlanProvider;

#[async_trait]
impl QueryPlanProvider for SqlServerQueryPlanProvider {
    async fn capture_plan(
        &self,
        client: &mut MssqlClient,
        sql: &str,
        opts: &PlanCaptureOptions,
    ) -> Result<QueryPlan, AppError> {
        if opts.show_estimated_only {
            capture_estimated(client, sql, opts).await
        } else {
            capture_actual(client, sql, opts).await
        }
    }
}

// Estimated plan: SET SHOWPLAN_XML ON returns the plan as a single-cell result
// set instead of executing the user's statement. We wrap the toggle in a batch
// so it flips on, runs the parse-only compile, and flips back off.
async fn capture_estimated(
    client: &mut MssqlClient,
    sql: &str,
    opts: &PlanCaptureOptions,
) -> Result<QueryPlan, AppError> {
    client
        .simple_query("SET SHOWPLAN_XML ON")
        .await
        .map_err(AppError::from)?;

    let capture = run_and_collect_plan_xml(client, sql).await;

    // Always try to disable, even if the capture errored — otherwise the
    // session is stuck in showplan mode.
    if let Err(e) = client.simple_query("SET SHOWPLAN_XML OFF").await {
        tracing::warn!(target: "queryben::query-plan", "SET SHOWPLAN_XML OFF failed: {e}");
    }

    let xml = capture?;
    parse_plan_xml(&xml, opts, false)
}

// Actual plan: STATISTICS XML gives us the plan enriched with runtime row
// counts. Unlike SHOWPLAN_XML, this executes the statement, so it is opt-in.
async fn capture_actual(
    client: &mut MssqlClient,
    sql: &str,
    opts: &PlanCaptureOptions,
) -> Result<QueryPlan, AppError> {
    client
        .simple_query("SET STATISTICS XML ON")
        .await
        .map_err(AppError::from)?;

    let capture = run_and_collect_plan_xml(client, sql).await;

    if let Err(e) = client.simple_query("SET STATISTICS XML OFF").await {
        tracing::warn!(target: "queryben::query-plan", "SET STATISTICS XML OFF failed: {e}");
    }

    let xml = capture?;
    parse_plan_xml(&xml, opts, true)
}

// Grab the first XML-looking cell from the response stream. SHOWPLAN_XML emits
// it as a single-row/single-col result; STATISTICS XML interleaves plan cells
// with the query's own result sets — take the first plan cell we see.
async fn run_and_collect_plan_xml(
    client: &mut MssqlClient,
    sql: &str,
) -> Result<String, AppError> {
    let query = Query::new(sql);
    let mut stream = query.query(client).await.map_err(AppError::from)?;

    while let Some(item) = stream.try_next().await.map_err(AppError::from)? {
        if let QueryItem::Row(row) = item {
            if let Some((_, cell)) = row.cells().next() {
                if let Some(s) = cell_as_string(cell) {
                    if s.contains("ShowPlanXML") || s.contains("<ShowPlanXML") {
                        return Ok(s);
                    }
                }
            }
        }
    }

    Err(AppError::internal(
        "no query plan returned — server may not support SHOWPLAN_XML for this batch",
    ))
}

fn cell_as_string(cell: &tiberius::ColumnData<'_>) -> Option<String> {
    match cell {
        tiberius::ColumnData::String(Some(s)) => Some(s.to_string()),
        tiberius::ColumnData::Xml(Some(x)) => Some(x.to_string()),
        _ => None,
    }
}

// ---- XML parse -------------------------------------------------------------
//
// showplan_xml shape (relevant bits only):
//   <ShowPlanXML>
//     <BatchSequence>
//       <Batch>
//         <Statements>
//           <StmtSimple StatementText="...">
//             <QueryPlan>
//               <MissingIndexes>...</MissingIndexes>          (optional)
//               <RelOp NodeId=".." PhysicalOp=".." LogicalOp=".." EstimateRows="..">
//                 <RunTimeInformation>                        (only w/ STATISTICS XML)
//                   <RunTimeCountersPerThread ActualRows=".." />
//                 </RunTimeInformation>
//                 <IndexScan|Sort|...>
//                   <Object Table="..." Index="..." />
//                 </...>
//                 <RelOp ...>...</RelOp>                       (children)
//               </RelOp>
//             </QueryPlan>
//           </StmtSimple>
//         </Statements>
//       </Batch>
//     </BatchSequence>
//   </ShowPlanXML>
//
// We use quick-xml's pull parser and hand-build the tree so we don't pull in a
// full DOM crate. The parser is a small state machine: on <RelOp> push a new
// node; on </RelOp> pop it and attach it to its parent. Non-RelOp elements
// (Object, RunTimeInformation, MissingIndexes) enrich the current top-of-stack.

fn parse_plan_xml(
    xml: &str,
    opts: &PlanCaptureOptions,
    is_actual: bool,
) -> Result<QueryPlan, AppError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut stack: Vec<PlanNode> = Vec::new();
    let mut roots: Vec<PlanNode> = Vec::new();
    let mut statement_text: Option<String> = None;
    let mut plan_warnings: Vec<PlanWarning> = Vec::new();
    let mut missing_index_impact: Option<f64> = None;

    loop {
        let event = reader
            .read_event_into(&mut buf)
            .map_err(|e| AppError::internal(format!("showplan XML parse: {e}")))?;

        match event {
            Event::Start(ref e) => {
                handle_open(
                    e,
                    &mut stack,
                    &mut statement_text,
                    &mut missing_index_impact,
                    opts,
                );
            }
            Event::Empty(ref e) => {
                // Self-closing tags never contain nested RelOp children, so we
                // apply the same enrichment logic but skip anything that would
                // push onto the stack (there'd be no matching End to pop it).
                handle_open(
                    e,
                    &mut stack,
                    &mut statement_text,
                    &mut missing_index_impact,
                    opts,
                );
                // Empty RelOp is malformed but tolerate: pop the just-pushed
                // node so we don't leak an unclosed frame.
                if e.name().as_ref() == b"RelOp" {
                    if let Some(node) = stack.pop() {
                        if let Some(parent) = stack.last_mut() {
                            parent.children.push(node);
                        } else {
                            roots.push(node);
                        }
                    }
                }
            }
            Event::End(ref e) => {
                let name_owned = e.name();
                let name = std::str::from_utf8(name_owned.as_ref()).unwrap_or("");
                match name {
                    "RelOp" => {
                        if let Some(node) = stack.pop() {
                            if let Some(parent) = stack.last_mut() {
                                parent.children.push(node);
                            } else {
                                roots.push(node);
                            }
                        }
                    }
                    "MissingIndexes" => {
                        if opts.warn_on_missing_index {
                            let msg = match missing_index_impact {
                                Some(i) => format!("missing index (est. impact {:.1}%)", i),
                                None => "missing index suggested".into(),
                            };
                            plan_warnings.push(PlanWarning {
                                kind: WarningKind::MissingIndex,
                                message: msg,
                            });
                        }
                        missing_index_impact = None;
                    }
                    _ => {}
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    let root = roots.into_iter().next().ok_or_else(|| {
        AppError::internal("plan XML had no RelOp — nothing to render")
    })?;

    Ok(QueryPlan {
        statement_text,
        root,
        warnings: plan_warnings,
        is_actual,
    })
}

fn handle_open(
    e: &quick_xml::events::BytesStart,
    stack: &mut Vec<PlanNode>,
    statement_text: &mut Option<String>,
    missing_index_impact: &mut Option<f64>,
    opts: &PlanCaptureOptions,
) {
    let name_bytes = e.name();
    let name = std::str::from_utf8(name_bytes.as_ref()).unwrap_or("");
    match name {
        "StmtSimple" => {
            if statement_text.is_none() {
                *statement_text = attr(e, "StatementText");
            }
        }
        "RelOp" => {
            let physical = attr(e, "PhysicalOp").unwrap_or_else(|| "Unknown".into());
            let logical = attr(e, "LogicalOp").unwrap_or_default();
            let est_rows = attr(e, "EstimateRows").and_then(|s| s.parse::<f64>().ok());
            let cost = attr(e, "EstimatedTotalSubtreeCost")
                .and_then(|s| s.parse::<f64>().ok());
            let id = attr(e, "NodeId")
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(stack.len() as u32);
            let op_kind = classify_op(&physical);
            let mut warnings: Vec<PlanWarning> = Vec::new();
            if let Some(rows) = est_rows {
                if matches!(op_kind, OpKind::TableScan | OpKind::IndexScan)
                    && rows >= opts.warn_on_scan_rows_above
                {
                    warnings.push(PlanWarning {
                        kind: WarningKind::LargeScan,
                        message: format!("{physical} over {:.0} rows", rows),
                    });
                }
            }
            let display_name = if logical.is_empty() || logical == physical {
                physical
            } else {
                format!("{physical} ({logical})")
            };
            stack.push(PlanNode {
                id,
                name: display_name,
                op_kind,
                estimated_rows: est_rows,
                actual_rows: None,
                estimated_cost: cost,
                warnings,
                object: None,
                children: Vec::new(),
            });
        }
        "Object" => {
            if let Some(top) = stack.last_mut() {
                let db = attr(e, "Database").unwrap_or_default();
                let schema = attr(e, "Schema").unwrap_or_default();
                let table = attr(e, "Table").unwrap_or_default();
                let index = attr(e, "Index").unwrap_or_default();
                let mut parts: Vec<String> = Vec::new();
                if !db.is_empty() {
                    parts.push(strip_brackets(&db));
                }
                if !schema.is_empty() {
                    parts.push(strip_brackets(&schema));
                }
                if !table.is_empty() {
                    parts.push(strip_brackets(&table));
                }
                let mut label = parts.join(".");
                if !index.is_empty() {
                    label.push_str(&format!(" [{}]", strip_brackets(&index)));
                }
                if !label.is_empty() && top.object.is_none() {
                    top.object = Some(label);
                }
            }
        }
        "RunTimeCountersPerThread" => {
            if let Some(top) = stack.last_mut() {
                if let Some(ar) = attr(e, "ActualRows").and_then(|s| s.parse::<f64>().ok()) {
                    top.actual_rows = Some(top.actual_rows.unwrap_or(0.0) + ar);
                }
            }
        }
        "MissingIndexGroup" => {
            *missing_index_impact = attr(e, "Impact").and_then(|s| s.parse::<f64>().ok());
        }
        "Warnings" => {
            if attr(e, "NoJoinPredicate").as_deref() == Some("1") {
                if let Some(top) = stack.last_mut() {
                    top.warnings.push(PlanWarning {
                        kind: WarningKind::NoJoinPredicate,
                        message: "no join predicate — cartesian product".into(),
                    });
                }
            }
        }
        "PlanAffectingConvert" => {
            if let Some(top) = stack.last_mut() {
                top.warnings.push(PlanWarning {
                    kind: WarningKind::ImplicitConversion,
                    message: attr(e, "ConvertIssue").unwrap_or_else(|| {
                        "implicit conversion affects plan choice".into()
                    }),
                });
            }
        }
        _ => {}
    }
}

fn attr(e: &quick_xml::events::BytesStart, key: &str) -> Option<String> {
    for attr in e.attributes().flatten() {
        if attr.key.as_ref() == key.as_bytes() {
            return std::str::from_utf8(&attr.value).ok().map(|s| s.to_string());
        }
    }
    None
}

fn strip_brackets(s: &str) -> String {
    s.trim_matches(|c| c == '[' || c == ']').to_string()
}

fn classify_op(physical: &str) -> OpKind {
    let p = physical.to_ascii_lowercase();
    if p.contains("table scan") {
        OpKind::TableScan
    } else if p.contains("index seek") || p.contains("clustered index seek") {
        OpKind::IndexSeek
    } else if p.contains("index scan") || p.contains("clustered index scan") {
        OpKind::IndexScan
    } else if p.contains("sort") {
        OpKind::Sort
    } else if p.contains("hash match") {
        OpKind::HashMatch
    } else if p.contains("nested loops") {
        OpKind::NestedLoops
    } else if p.contains("merge join") {
        OpKind::MergeJoin
    } else if p.contains("compute scalar") {
        OpKind::ComputeScalar
    } else if p.contains("filter") {
        OpKind::Filter
    } else if p.contains("aggregate") || p.contains("stream aggregate") {
        OpKind::Aggregate
    } else if p.contains("parallelism") {
        OpKind::Parallelism
    } else if p.contains("spool") {
        OpKind::Spool
    } else {
        OpKind::Unknown
    }
}

// ---- MySQL / Postgres ------------------------------------------------------
// Intentionally unimplemented — the trait matches on connection engine, and
// these slots fill in when the drivers land.

pub struct MySqlQueryPlanProvider;

#[async_trait]
impl QueryPlanProvider for MySqlQueryPlanProvider {
    async fn capture_plan(
        &self,
        _client: &mut MssqlClient,
        _sql: &str,
        _opts: &PlanCaptureOptions,
    ) -> Result<QueryPlan, AppError> {
        todo!("mysql query plan capture (EXPLAIN FORMAT=TREE) not wired yet")
    }
}

pub struct PostgresQueryPlanProvider;

#[async_trait]
impl QueryPlanProvider for PostgresQueryPlanProvider {
    async fn capture_plan(
        &self,
        _client: &mut MssqlClient,
        _sql: &str,
        _opts: &PlanCaptureOptions,
    ) -> Result<QueryPlan, AppError> {
        todo!("postgres query plan capture (EXPLAIN (FORMAT JSON)) not wired yet")
    }
}
