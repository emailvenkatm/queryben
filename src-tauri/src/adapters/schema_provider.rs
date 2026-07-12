//! Provider abstraction for schema-compare. Different engines have wildly
//! different DDL surfaces (sys.* vs pg_catalog vs information_schema.*), so
//! each engine ships its own `SchemaProvider` impl and the compare commands
//! route on connection kind.
//!
//! For v1 only `SqlServerSchemaProvider` is real. MySql / Postgres return
//! `todo!()` so a future crate PR is a plug-in.

use async_trait::async_trait;
use tiberius::{Query, Row};

use crate::core::schema_diff::{
    ColumnSpec, DdlStatement, IndexSpec, ObjectChange, ObjectKind, SchemaDiff, SchemaObject,
    SchemaSnapshot,
};
use crate::error::AppError;
use crate::adapters::mssql::MssqlClient;

#[async_trait]
pub trait SchemaProvider: Send + Sync {
    async fn snapshot(&self, client: &mut MssqlClient) -> Result<Vec<SchemaObject>, AppError>;
    fn generate_ddl(&self, diff: &SchemaDiff) -> Vec<DdlStatement>;
}

pub struct SqlServerSchemaProvider;

// ---- introspection SQL ------------------------------------------------------
//
// The five queries mirror the shape of the object-explorer introspection in
// commands/query.rs but pull enough detail to diff. Kept as local const strings
// (not moved) so the object-explorer path stays independently readable.

const TABLES_SQL: &str = "SELECT s.name AS schema_name, t.name AS table_name
       FROM sys.tables AS t
       JOIN sys.schemas AS s ON s.schema_id = t.schema_id
      WHERE s.name NOT IN ('sys','INFORMATION_SCHEMA','guest')
        AND s.name NOT LIKE 'db\\_%' ESCAPE '\\'";

const VIEWS_SQL: &str = "SELECT s.name AS schema_name, v.name AS view_name,
            OBJECT_DEFINITION(v.object_id) AS body
       FROM sys.views AS v
       JOIN sys.schemas AS s ON s.schema_id = v.schema_id
      WHERE s.name NOT IN ('sys','INFORMATION_SCHEMA','guest')
        AND s.name NOT LIKE 'db\\_%' ESCAPE '\\'";

const PROCS_SQL: &str = "SELECT s.name AS schema_name, p.name AS proc_name,
            OBJECT_DEFINITION(p.object_id) AS body
       FROM sys.procedures AS p
       JOIN sys.schemas AS s ON s.schema_id = p.schema_id
      WHERE s.name NOT IN ('sys','INFORMATION_SCHEMA','guest')
        AND s.name NOT LIKE 'db\\_%' ESCAPE '\\'";

const FNS_SQL: &str = "SELECT s.name AS schema_name, o.name AS fn_name,
            OBJECT_DEFINITION(o.object_id) AS body
       FROM sys.objects AS o
       JOIN sys.schemas AS s ON s.schema_id = o.schema_id
      WHERE o.type IN ('FN','IF','TF')
        AND s.name NOT IN ('sys','INFORMATION_SCHEMA','guest')
        AND s.name NOT LIKE 'db\\_%' ESCAPE '\\'";

// Column shape per table. Same joins the object-explorer's per-table drill-in
// uses, just yanked across every table in one pass.
const COLUMNS_SQL: &str = "SELECT s.name AS schema_name,
            t.name AS table_name,
            c.name AS column_name,
            TYPE_NAME(c.user_type_id) AS data_type,
            c.max_length,
            c.precision,
            c.scale,
            c.is_nullable,
            c.is_identity,
            c.is_computed,
            OBJECT_DEFINITION(dc.object_id) AS default_expr,
            c.column_id
       FROM sys.columns AS c
       JOIN sys.tables AS t ON t.object_id = c.object_id
       JOIN sys.schemas AS s ON s.schema_id = t.schema_id
       LEFT JOIN sys.default_constraints AS dc
              ON dc.parent_object_id = c.object_id
             AND dc.parent_column_id = c.column_id
      WHERE s.name NOT IN ('sys','INFORMATION_SCHEMA','guest')
        AND s.name NOT LIKE 'db\\_%' ESCAPE '\\'
      ORDER BY s.name, t.name, c.column_id";

// Index shape per table, including PK. Excludes heaps (index_id = 0).
const INDEXES_SQL: &str = "SELECT s.name AS schema_name,
            t.name AS table_name,
            i.name AS index_name,
            i.is_unique,
            i.is_primary_key,
            (SELECT STRING_AGG(c.name, ',') WITHIN GROUP (ORDER BY ic.key_ordinal)
               FROM sys.index_columns AS ic
               JOIN sys.columns AS c
                 ON c.object_id = ic.object_id AND c.column_id = ic.column_id
              WHERE ic.object_id = i.object_id
                AND ic.index_id = i.index_id
                AND ic.is_included_column = 0) AS key_columns
       FROM sys.indexes AS i
       JOIN sys.tables AS t ON t.object_id = i.object_id
       JOIN sys.schemas AS s ON s.schema_id = t.schema_id
      WHERE i.index_id > 0
        AND i.name IS NOT NULL
        AND s.name NOT IN ('sys','INFORMATION_SCHEMA','guest')
        AND s.name NOT LIKE 'db\\_%' ESCAPE '\\'";

#[async_trait]
impl SchemaProvider for SqlServerSchemaProvider {
    async fn snapshot(
        &self,
        client: &mut MssqlClient,
    ) -> Result<Vec<SchemaObject>, AppError> {
        // Five sequential introspections. We collect column + index rows into
        // maps keyed by (schema, table) and stitch them into the table objects.
        let table_rows = fetch(client, TABLES_SQL).await?;
        let view_rows = fetch(client, VIEWS_SQL).await?;
        let proc_rows = fetch(client, PROCS_SQL).await?;
        let fn_rows = fetch(client, FNS_SQL).await?;
        let col_rows = fetch(client, COLUMNS_SQL).await?;
        let idx_rows = fetch(client, INDEXES_SQL).await?;

        let mut columns_by_table: std::collections::BTreeMap<
            (String, String),
            Vec<ColumnSpec>,
        > = std::collections::BTreeMap::new();
        for row in col_rows {
            let schema = str_col(&row, 0, "schema_name")?;
            let table = str_col(&row, 1, "table_name")?;
            let name = str_col(&row, 2, "column_name")?;
            let data_type = str_col(&row, 3, "data_type")?;
            let max_length = row
                .try_get::<i16, _>(4)
                .map_err(AppError::from)?
                .unwrap_or(0) as i32;
            let precision = row.try_get::<u8, _>(5).map_err(AppError::from)?.unwrap_or(0) as i32;
            let scale = row.try_get::<u8, _>(6).map_err(AppError::from)?.unwrap_or(0) as i32;
            let is_nullable = row
                .try_get::<bool, _>(7)
                .map_err(AppError::from)?
                .unwrap_or(true);
            let is_identity = row
                .try_get::<bool, _>(8)
                .map_err(AppError::from)?
                .unwrap_or(false);
            let is_computed = row
                .try_get::<bool, _>(9)
                .map_err(AppError::from)?
                .unwrap_or(false);
            let default_expression = row
                .try_get::<&str, _>(10)
                .map_err(AppError::from)?
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty());
            let ordinal = row.try_get::<i32, _>(11).map_err(AppError::from)?.unwrap_or(0).max(0) as u32;

            columns_by_table
                .entry((schema, table))
                .or_default()
                .push(ColumnSpec {
                    name,
                    sql_type: format_sql_type(&data_type, max_length, precision, scale),
                    is_nullable,
                    is_identity,
                    is_computed,
                    default_expression,
                    ordinal,
                });
        }

        let mut indexes_by_table: std::collections::BTreeMap<
            (String, String),
            Vec<IndexSpec>,
        > = std::collections::BTreeMap::new();
        let mut standalone_indexes: Vec<SchemaObject> = Vec::new();
        for row in idx_rows {
            let schema = str_col(&row, 0, "schema_name")?;
            let table = str_col(&row, 1, "table_name")?;
            let name = str_col(&row, 2, "index_name")?;
            let is_unique = row
                .try_get::<bool, _>(3)
                .map_err(AppError::from)?
                .unwrap_or(false);
            let is_primary_key = row
                .try_get::<bool, _>(4)
                .map_err(AppError::from)?
                .unwrap_or(false);
            let key_columns_str = row
                .try_get::<&str, _>(5)
                .map_err(AppError::from)?
                .unwrap_or("")
                .to_string();
            let columns: Vec<String> = key_columns_str
                .split(',')
                .filter(|s| !s.is_empty())
                .map(|s| s.trim().to_string())
                .collect();

            let idx = IndexSpec {
                name: name.clone(),
                is_unique,
                is_primary_key,
                columns: columns.clone(),
            };
            indexes_by_table
                .entry((schema.clone(), table.clone()))
                .or_default()
                .push(idx.clone());

            // Also surface non-PK indexes as standalone Index objects so the
            // include-object-kinds filter can toggle them separately.
            if !is_primary_key {
                let qname = format!("{schema}.{table}.{name}");
                standalone_indexes.push(SchemaObject {
                    kind: ObjectKind::Index,
                    schema: schema.clone(),
                    name: name.clone(),
                    qualified_name: qname,
                    columns: Vec::new(),
                    indexes: vec![idx],
                    body: None,
                });
            }
        }

        let mut out: Vec<SchemaObject> = Vec::new();

        for row in table_rows {
            let schema = str_col(&row, 0, "schema_name")?;
            let name = str_col(&row, 1, "table_name")?;
            let qname = format!("{schema}.{name}");
            let cols = columns_by_table
                .remove(&(schema.clone(), name.clone()))
                .unwrap_or_default();
            let mut idxs = indexes_by_table
                .remove(&(schema.clone(), name.clone()))
                .unwrap_or_default();
            idxs.sort_by(|a, b| a.name.cmp(&b.name));
            out.push(SchemaObject {
                kind: ObjectKind::Table,
                schema,
                name,
                qualified_name: qname,
                columns: cols,
                indexes: idxs,
                body: None,
            });
        }

        for row in view_rows {
            let schema = str_col(&row, 0, "schema_name")?;
            let name = str_col(&row, 1, "view_name")?;
            let body = row
                .try_get::<&str, _>(2)
                .map_err(AppError::from)?
                .map(|s| s.to_string());
            let qname = format!("{schema}.{name}");
            out.push(SchemaObject {
                kind: ObjectKind::View,
                schema,
                name,
                qualified_name: qname,
                columns: Vec::new(),
                indexes: Vec::new(),
                body,
            });
        }

        for row in proc_rows {
            let schema = str_col(&row, 0, "schema_name")?;
            let name = str_col(&row, 1, "proc_name")?;
            let body = row
                .try_get::<&str, _>(2)
                .map_err(AppError::from)?
                .map(|s| s.to_string());
            let qname = format!("{schema}.{name}");
            out.push(SchemaObject {
                kind: ObjectKind::Procedure,
                schema,
                name,
                qualified_name: qname,
                columns: Vec::new(),
                indexes: Vec::new(),
                body,
            });
        }

        for row in fn_rows {
            let schema = str_col(&row, 0, "schema_name")?;
            let name = str_col(&row, 1, "fn_name")?;
            let body = row
                .try_get::<&str, _>(2)
                .map_err(AppError::from)?
                .map(|s| s.to_string());
            let qname = format!("{schema}.{name}");
            out.push(SchemaObject {
                kind: ObjectKind::Function,
                schema,
                name,
                qualified_name: qname,
                columns: Vec::new(),
                indexes: Vec::new(),
                body,
            });
        }

        out.extend(standalone_indexes);

        out.sort_by(|a, b| {
            a.kind
                .as_config_str()
                .cmp(b.kind.as_config_str())
                .then_with(|| a.qualified_name.cmp(&b.qualified_name))
        });

        Ok(out)
    }

    fn generate_ddl(&self, diff: &SchemaDiff) -> Vec<DdlStatement> {
        let mut out: Vec<DdlStatement> = Vec::new();

        // DROP first so name collisions can't block CREATE. Views/procs/fns
        // use the matching DROP variant so the script is idempotent-ish.
        for change in &diff.dropped {
            let Some(obj) = change.target.as_ref() else {
                continue;
            };
            let sql = match obj.kind {
                ObjectKind::Table => format!(
                    "DROP TABLE [{}].[{}];",
                    obj.schema, obj.name
                ),
                ObjectKind::View => format!(
                    "DROP VIEW [{}].[{}];",
                    obj.schema, obj.name
                ),
                ObjectKind::Procedure => format!(
                    "DROP PROCEDURE [{}].[{}];",
                    obj.schema, obj.name
                ),
                ObjectKind::Function => format!(
                    "DROP FUNCTION [{}].[{}];",
                    obj.schema, obj.name
                ),
                ObjectKind::Index => {
                    // Standalone-index name qualifies as schema.table.index.
                    if let Some((table, idx)) = split_index_qname(&change.qualified_name) {
                        format!("DROP INDEX [{}] ON [{}];", idx, table)
                    } else {
                        format!(
                            "-- unable to drop index {}: could not parse qualified name",
                            change.qualified_name
                        )
                    }
                }
            };
            out.push(DdlStatement {
                object_kind: obj.kind.clone(),
                object_name: change.qualified_name.clone(),
                kind: "DROP".into(),
                sql,
            });
        }

        // CREATE for adds. Tables get a full CREATE TABLE; views/procs/fns
        // re-emit their source body from the source snapshot.
        for change in &diff.added {
            let Some(obj) = change.source.as_ref() else {
                continue;
            };
            let stmt = match obj.kind {
                ObjectKind::Table => Some(DdlStatement {
                    object_kind: obj.kind.clone(),
                    object_name: change.qualified_name.clone(),
                    kind: "CREATE".into(),
                    sql: generate_create_table(obj),
                }),
                ObjectKind::View | ObjectKind::Procedure | ObjectKind::Function => {
                    obj.body.as_ref().map(|body| DdlStatement {
                        object_kind: obj.kind.clone(),
                        object_name: change.qualified_name.clone(),
                        kind: "CREATE".into(),
                        sql: body.trim().to_string(),
                    })
                }
                ObjectKind::Index => obj.indexes.first().map(|idx| DdlStatement {
                    object_kind: obj.kind.clone(),
                    object_name: change.qualified_name.clone(),
                    kind: "CREATE".into(),
                    sql: generate_create_index(&obj.schema, &change.qualified_name, idx),
                }),
            };
            if let Some(s) = stmt {
                out.push(s);
            }
        }

        // Changes: for tables, emit column ADD/DROP and per-column ALTER; for
        // views/procs/fns emit ALTER with the new body. Skips complex FK
        // reordering (out of scope for v1).
        for change in &diff.changed {
            let (Some(source), Some(target)) = (change.source.as_ref(), change.target.as_ref())
            else {
                continue;
            };
            match source.kind {
                ObjectKind::Table => {
                    for stmt in generate_table_alter(change, source, target) {
                        out.push(stmt);
                    }
                }
                ObjectKind::View | ObjectKind::Procedure | ObjectKind::Function => {
                    let keyword = match source.kind {
                        ObjectKind::View => "VIEW",
                        ObjectKind::Procedure => "PROCEDURE",
                        _ => "FUNCTION",
                    };
                    if let Some(body) = source.body.as_ref() {
                        // Rewrite the leading CREATE to ALTER — SQL Server's
                        // OBJECT_DEFINITION always ships CREATE, but ALTER
                        // reuses the same body shape.
                        let altered = rewrite_create_to_alter(body, keyword);
                        out.push(DdlStatement {
                            object_kind: source.kind.clone(),
                            object_name: change.qualified_name.clone(),
                            kind: "ALTER".into(),
                            sql: altered,
                        });
                    }
                }
                ObjectKind::Index => {
                    // Index diff = drop then recreate; SQL Server has no
                    // in-place ALTER for column list changes.
                    if let Some((table, idx_name)) = split_index_qname(&change.qualified_name) {
                        out.push(DdlStatement {
                            object_kind: ObjectKind::Index,
                            object_name: change.qualified_name.clone(),
                            kind: "DROP".into(),
                            sql: format!("DROP INDEX [{}] ON [{}];", idx_name, table),
                        });
                    }
                    if let Some(idx) = source.indexes.first() {
                        out.push(DdlStatement {
                            object_kind: ObjectKind::Index,
                            object_name: change.qualified_name.clone(),
                            kind: "CREATE".into(),
                            sql: generate_create_index(
                                &source.schema,
                                &change.qualified_name,
                                idx,
                            ),
                        });
                    }
                }
            }
        }

        out
    }
}

// ---- generic helpers --------------------------------------------------------

async fn fetch(client: &mut MssqlClient, sql: &'static str) -> Result<Vec<Row>, AppError> {
    Ok(Query::new(sql)
        .query(client)
        .await?
        .into_first_result()
        .await?)
}

fn str_col(row: &Row, idx: usize, label: &str) -> Result<String, AppError> {
    match row.try_get::<&str, _>(idx).map_err(AppError::from)? {
        Some(s) => Ok(s.to_string()),
        None => Err(AppError::internal(format!(
            "schema-compare row column {label} (idx {idx}) was NULL"
        ))),
    }
}

fn format_sql_type(data_type: &str, max_length: i32, precision: i32, scale: i32) -> String {
    let dt = data_type.to_ascii_lowercase();
    match dt.as_str() {
        "char" | "varchar" | "binary" | "varbinary" => match max_length {
            -1 => format!("{dt}(max)"),
            n if n > 0 => format!("{dt}({n})"),
            _ => dt,
        },
        "nchar" | "nvarchar" => match max_length {
            -1 => format!("{dt}(max)"),
            // sys.columns.max_length is byte-length; nvarchar chars = bytes/2.
            n if n > 0 => format!("{dt}({})", n / 2),
            _ => dt,
        },
        "decimal" | "numeric" => format!("{dt}({precision},{scale})"),
        _ => dt,
    }
}

// "schema.table.index" -> ("[schema].[table]", "index").
fn split_index_qname(qname: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = qname.split('.').collect();
    if parts.len() < 3 {
        return None;
    }
    let idx = parts.last()?.to_string();
    let schema = parts[0];
    // Everything between schema and index is the table (usually one segment).
    let table = parts[1..parts.len() - 1].join(".");
    Some((format!("[{schema}].[{table}]"), idx))
}

fn generate_create_table(obj: &SchemaObject) -> String {
    let mut cols: Vec<String> = Vec::with_capacity(obj.columns.len());
    for col in &obj.columns {
        let mut line = format!("  [{}] {}", col.name, col.sql_type);
        if col.is_identity {
            line.push_str(" IDENTITY(1,1)");
        }
        if col.is_nullable {
            line.push_str(" NULL");
        } else {
            line.push_str(" NOT NULL");
        }
        if let Some(def) = col.default_expression.as_ref() {
            // Wrap in DEFAULT (...) so we can round-trip either style. The
            // introspected expression usually already carries its own parens
            // — that's fine, nested parens are harmless.
            line.push_str(&format!(" DEFAULT {}", def));
        }
        cols.push(line);
    }

    let pk = obj.indexes.iter().find(|i| i.is_primary_key);
    if let Some(pk) = pk {
        let cols_list = pk
            .columns
            .iter()
            .map(|c| format!("[{c}]"))
            .collect::<Vec<_>>()
            .join(", ");
        cols.push(format!(
            "  CONSTRAINT [{}] PRIMARY KEY ({})",
            pk.name, cols_list
        ));
    }

    format!(
        "CREATE TABLE [{}].[{}] (\n{}\n);",
        obj.schema,
        obj.name,
        cols.join(",\n")
    )
}

fn generate_create_index(_schema: &str, qname: &str, idx: &IndexSpec) -> String {
    // qname = "schema.table.index"
    let Some((table, idx_name)) = split_index_qname(qname) else {
        return format!("-- unable to render CREATE INDEX for {qname}");
    };
    let cols = idx
        .columns
        .iter()
        .map(|c| format!("[{c}]"))
        .collect::<Vec<_>>()
        .join(", ");
    let unique = if idx.is_unique { "UNIQUE " } else { "" };
    format!("CREATE {unique}INDEX [{idx_name}] ON {table} ({cols});")
}

fn generate_table_alter(
    change: &ObjectChange,
    source: &SchemaObject,
    target: &SchemaObject,
) -> Vec<DdlStatement> {
    use std::collections::BTreeMap;
    let mut out: Vec<DdlStatement> = Vec::new();
    let src_cols: BTreeMap<String, &ColumnSpec> =
        source.columns.iter().map(|c| (c.name.clone(), c)).collect();
    let tgt_cols: BTreeMap<String, &ColumnSpec> =
        target.columns.iter().map(|c| (c.name.clone(), c)).collect();

    // Column DROPs (target has, source doesn't).
    for (name, _) in &tgt_cols {
        if !src_cols.contains_key(name) {
            out.push(DdlStatement {
                object_kind: ObjectKind::Table,
                object_name: change.qualified_name.clone(),
                kind: "ALTER".into(),
                sql: format!(
                    "ALTER TABLE [{}].[{}] DROP COLUMN [{}];",
                    source.schema, source.name, name
                ),
            });
        }
    }

    // Column ADDs (source has, target doesn't).
    for (name, col) in &src_cols {
        if !tgt_cols.contains_key(name) {
            let nullable = if col.is_nullable { "NULL" } else { "NOT NULL" };
            let default = col
                .default_expression
                .as_ref()
                .map(|d| format!(" DEFAULT {}", d))
                .unwrap_or_default();
            out.push(DdlStatement {
                object_kind: ObjectKind::Table,
                object_name: change.qualified_name.clone(),
                kind: "ALTER".into(),
                sql: format!(
                    "ALTER TABLE [{}].[{}] ADD [{}] {} {}{};",
                    source.schema, source.name, name, col.sql_type, nullable, default
                ),
            });
        }
    }

    // Column ALTERs for type / nullability changes on shared columns. Skips
    // identity / computed flag transitions — those need drop+add in real life.
    for (name, src) in &src_cols {
        let Some(tgt) = tgt_cols.get(name) else {
            continue;
        };
        if src.sql_type == tgt.sql_type && src.is_nullable == tgt.is_nullable {
            continue;
        }
        let nullable = if src.is_nullable { "NULL" } else { "NOT NULL" };
        out.push(DdlStatement {
            object_kind: ObjectKind::Table,
            object_name: change.qualified_name.clone(),
            kind: "ALTER".into(),
            sql: format!(
                "ALTER TABLE [{}].[{}] ALTER COLUMN [{}] {} {};",
                source.schema, source.name, name, src.sql_type, nullable
            ),
        });
    }

    out
}

// SQL Server's OBJECT_DEFINITION returns "CREATE VIEW ..." / "CREATE PROC ...".
// For ALTER we need the same body with "CREATE" -> "ALTER" on the first
// occurrence only. Case-insensitive match on the leading keyword.
fn rewrite_create_to_alter(body: &str, keyword: &str) -> String {
    let lower = body.to_ascii_lowercase();
    let create_prefix = "create";
    if let Some(idx) = lower.find(create_prefix) {
        // Only rewrite if "CREATE <keyword>" appears — belt-and-braces so we
        // don't clobber a random word inside a comment.
        let after = &body[idx + create_prefix.len()..];
        let after_trim = after.trim_start();
        let kw_lower = keyword.to_ascii_lowercase();
        if after_trim.to_ascii_lowercase().starts_with(&kw_lower) {
            let mut out = String::with_capacity(body.len());
            out.push_str(&body[..idx]);
            out.push_str("ALTER");
            out.push_str(after);
            return out;
        }
    }
    // Fallback: prefix a fresh ALTER even if the body wasn't the shape we
    // expected — better than silently emitting a duplicate CREATE.
    format!("-- rewrite fallback (body didn't start with CREATE {keyword})\n{body}")
}

// ---- diff engine ------------------------------------------------------------

// Public helper the command layer calls after grabbing snapshots from both
// sides. Lives here (not in domain) because the diff rules depend on how the
// provider filled in each `SchemaObject`.
pub fn compute_diff(
    source: &SchemaSnapshot,
    target: &SchemaSnapshot,
    options: &crate::core::schema_diff::SchemaCompareOptions,
) -> SchemaDiff {
    use std::collections::BTreeMap;

    let key = |o: &SchemaObject| (o.kind.clone(), o.qualified_name.clone());
    let src_map: BTreeMap<_, &SchemaObject> =
        source.objects.iter().map(|o| (key(o), o)).collect();
    let tgt_map: BTreeMap<_, &SchemaObject> =
        target.objects.iter().map(|o| (key(o), o)).collect();

    let mut added: Vec<ObjectChange> = Vec::new();
    let mut dropped: Vec<ObjectChange> = Vec::new();
    let mut changed: Vec<ObjectChange> = Vec::new();
    let mut unchanged_count: u32 = 0;

    for (k, src) in &src_map {
        if !options.includes(&k.0) {
            continue;
        }
        match tgt_map.get(k) {
            None => added.push(ObjectChange {
                kind: k.0.clone(),
                qualified_name: k.1.clone(),
                source: Some((*src).clone()),
                target: None,
                reasons: Vec::new(),
            }),
            Some(tgt) => {
                let reasons = diff_reasons(src, tgt, options);
                if reasons.is_empty() {
                    unchanged_count += 1;
                } else {
                    changed.push(ObjectChange {
                        kind: k.0.clone(),
                        qualified_name: k.1.clone(),
                        source: Some((*src).clone()),
                        target: Some((*tgt).clone()),
                        reasons,
                    });
                }
            }
        }
    }

    for (k, tgt) in &tgt_map {
        if !options.includes(&k.0) {
            continue;
        }
        if !src_map.contains_key(k) {
            dropped.push(ObjectChange {
                kind: k.0.clone(),
                qualified_name: k.1.clone(),
                source: None,
                target: Some((*tgt).clone()),
                reasons: Vec::new(),
            });
        }
    }

    SchemaDiff {
        source_label: source.label.clone(),
        target_label: target.label.clone(),
        added,
        dropped,
        changed,
        unchanged_count,
    }
}

fn diff_reasons(
    source: &SchemaObject,
    target: &SchemaObject,
    options: &crate::core::schema_diff::SchemaCompareOptions,
) -> Vec<String> {
    let mut reasons = Vec::new();
    match source.kind {
        ObjectKind::Table => {
            use std::collections::BTreeMap;
            let s_cols: BTreeMap<&str, &ColumnSpec> = source
                .columns
                .iter()
                .map(|c| (c.name.as_str(), c))
                .collect();
            let t_cols: BTreeMap<&str, &ColumnSpec> = target
                .columns
                .iter()
                .map(|c| (c.name.as_str(), c))
                .collect();
            for (name, sc) in &s_cols {
                match t_cols.get(name) {
                    None => reasons.push(format!("column '{name}' added")),
                    Some(tc) => {
                        if sc.sql_type != tc.sql_type {
                            reasons.push(format!(
                                "column '{name}' type: {} -> {}",
                                tc.sql_type, sc.sql_type
                            ));
                        }
                        if sc.is_nullable != tc.is_nullable {
                            reasons.push(format!(
                                "column '{name}' nullability: {} -> {}",
                                tc.is_nullable, sc.is_nullable
                            ));
                        }
                    }
                }
            }
            for name in t_cols.keys() {
                if !s_cols.contains_key(name) {
                    reasons.push(format!("column '{name}' dropped"));
                }
            }
            // Index churn — added/dropped/differing key lists.
            let s_idx: BTreeMap<&str, &IndexSpec> =
                source.indexes.iter().map(|i| (i.name.as_str(), i)).collect();
            let t_idx: BTreeMap<&str, &IndexSpec> =
                target.indexes.iter().map(|i| (i.name.as_str(), i)).collect();
            for (name, si) in &s_idx {
                match t_idx.get(name) {
                    None => reasons.push(format!("index '{name}' added")),
                    Some(ti) => {
                        if si.columns != ti.columns {
                            reasons.push(format!(
                                "index '{name}' columns: [{}] -> [{}]",
                                ti.columns.join(","),
                                si.columns.join(",")
                            ));
                        }
                        if si.is_unique != ti.is_unique {
                            reasons.push(format!("index '{name}' uniqueness changed"));
                        }
                    }
                }
            }
            for name in t_idx.keys() {
                if !s_idx.contains_key(name) {
                    reasons.push(format!("index '{name}' dropped"));
                }
            }
        }
        ObjectKind::View | ObjectKind::Procedure | ObjectKind::Function => {
            let s = normalize_body(source.body.as_deref().unwrap_or(""), options);
            let t = normalize_body(target.body.as_deref().unwrap_or(""), options);
            if s != t {
                reasons.push("body changed".into());
            }
        }
        ObjectKind::Index => {
            let s = source.indexes.first();
            let t = target.indexes.first();
            if let (Some(s), Some(t)) = (s, t) {
                if s.columns != t.columns {
                    reasons.push(format!(
                        "columns: [{}] -> [{}]",
                        t.columns.join(","),
                        s.columns.join(",")
                    ));
                }
                if s.is_unique != t.is_unique {
                    reasons.push("uniqueness changed".into());
                }
            }
        }
    }
    reasons
}

fn normalize_body(body: &str, options: &crate::core::schema_diff::SchemaCompareOptions) -> String {
    let mut s = body.to_string();
    if options.ignore_whitespace {
        // Collapse all runs of whitespace to a single space so trailing/leading
        // padding drift doesn't flag as a real diff.
        s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    }
    if options.ignore_collation {
        // Strip trailing "COLLATE <name>" clauses; frontend never needs to
        // surface a pure-collation delta.
        while let Some(idx) = s.to_ascii_uppercase().find(" COLLATE ") {
            let tail = &s[idx + " COLLATE ".len()..];
            let end = tail
                .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .unwrap_or(tail.len());
            let mut new = String::with_capacity(s.len());
            new.push_str(&s[..idx]);
            new.push_str(&tail[end..]);
            s = new;
        }
    }
    if options.ignore_fillfactor {
        // Strip "FILLFACTOR = N" chunks (whole-token match).
        s = s
            .split(' ')
            .filter(|tok| !tok.eq_ignore_ascii_case("fillfactor"))
            .collect::<Vec<_>>()
            .join(" ");
    }
    s
}

// ---- stubs for future engines ----------------------------------------------

pub struct MySqlSchemaProvider;

#[async_trait]
impl SchemaProvider for MySqlSchemaProvider {
    async fn snapshot(
        &self,
        _client: &mut MssqlClient,
    ) -> Result<Vec<SchemaObject>, AppError> {
        todo!("MySqlSchemaProvider::snapshot — pending MySQL driver plumbing")
    }
    fn generate_ddl(&self, _diff: &SchemaDiff) -> Vec<DdlStatement> {
        todo!("MySqlSchemaProvider::generate_ddl")
    }
}

pub struct PostgresSchemaProvider;

#[async_trait]
impl SchemaProvider for PostgresSchemaProvider {
    async fn snapshot(
        &self,
        _client: &mut MssqlClient,
    ) -> Result<Vec<SchemaObject>, AppError> {
        todo!("PostgresSchemaProvider::snapshot — pending pgwire driver plumbing")
    }
    fn generate_ddl(&self, _diff: &SchemaDiff) -> Vec<DdlStatement> {
        todo!("PostgresSchemaProvider::generate_ddl")
    }
}
