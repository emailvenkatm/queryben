//! Turn a `SchemaDiff` into a review-ready DDL script. DROPs come first so
//! name collisions can't block a subsequent CREATE.

use crate::core::schema_diff::{
    ColumnSpec, DdlStatement, IndexSpec, ObjectChange, ObjectKind, SchemaDiff, SchemaObject,
};

use super::sql::split_index_qname;

pub(super) fn generate_ddl(diff: &SchemaDiff) -> Vec<DdlStatement> {
    let mut out: Vec<DdlStatement> = Vec::new();

    for change in &diff.dropped {
        if let Some(stmt) = drop_statement(change) {
            out.push(stmt);
        }
    }

    for change in &diff.added {
        if let Some(stmt) = create_statement(change) {
            out.push(stmt);
        }
    }

    for change in &diff.changed {
        alter_statements(change, &mut out);
    }

    out
}

fn drop_statement(change: &ObjectChange) -> Option<DdlStatement> {
    let obj = change.target.as_ref()?;
    let sql = match obj.kind {
        ObjectKind::Table => format!("DROP TABLE [{}].[{}];", obj.schema, obj.name),
        ObjectKind::View => format!("DROP VIEW [{}].[{}];", obj.schema, obj.name),
        ObjectKind::Procedure => format!("DROP PROCEDURE [{}].[{}];", obj.schema, obj.name),
        ObjectKind::Function => format!("DROP FUNCTION [{}].[{}];", obj.schema, obj.name),
        ObjectKind::Index => match split_index_qname(&change.qualified_name) {
            Some((table, idx)) => format!("DROP INDEX [{}] ON [{}];", idx, table),
            None => format!(
                "-- unable to drop index {}: could not parse qualified name",
                change.qualified_name
            ),
        },
    };
    Some(DdlStatement {
        object_kind: obj.kind.clone(),
        object_name: change.qualified_name.clone(),
        kind: "DROP".into(),
        sql,
    })
}

fn create_statement(change: &ObjectChange) -> Option<DdlStatement> {
    let obj = change.source.as_ref()?;
    match obj.kind {
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
    }
}

fn alter_statements(change: &ObjectChange, out: &mut Vec<DdlStatement>) {
    let (Some(source), Some(target)) = (change.source.as_ref(), change.target.as_ref()) else {
        return;
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
                // OBJECT_DEFINITION always ships CREATE, but ALTER reuses the
                // same body shape.
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
            // Index diff = drop then recreate; SQL Server has no in-place
            // ALTER for column list changes.
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
                    sql: generate_create_index(&source.schema, &change.qualified_name, idx),
                });
            }
        }
    }
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
    format!("-- rewrite fallback (body didn't start with CREATE {keyword})\n{body}")
}
