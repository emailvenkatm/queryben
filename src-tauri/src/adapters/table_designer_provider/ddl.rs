//! Diff a current `TableDesign` against the user's `next` and emit a
//! review-ready DDL script. CREATE when `current` is None.

use std::collections::BTreeMap;

use crate::core::table_design::{
    DdlStatement, DesignColumn, DesignForeignKey, DesignIndex, TableDesign,
};

pub(super) fn generate_ddl(
    current: Option<&TableDesign>,
    next: &TableDesign,
) -> Vec<DdlStatement> {
    match current {
        None => vec![DdlStatement {
            kind: "CREATE".into(),
            label: format!("CREATE TABLE [{}].[{}]", next.schema, next.name),
            sql: render_create_table(next),
        }],
        Some(cur) => render_alter(cur, next),
    }
}

fn render_create_table(t: &TableDesign) -> String {
    let mut lines: Vec<String> = Vec::with_capacity(t.columns.len() + 2);
    for col in &t.columns {
        lines.push(format!("  {}", render_column_def(col)));
    }
    if !t.primary_key.is_empty() {
        let cols = t
            .primary_key
            .iter()
            .map(|c| format!("[{c}]"))
            .collect::<Vec<_>>()
            .join(", ");
        let pk_name = t
            .pk_name
            .clone()
            .unwrap_or_else(|| format!("PK_{}", t.name));
        lines.push(format!("  CONSTRAINT [{pk_name}] PRIMARY KEY ({cols})"));
    }
    for fk in &t.foreign_keys {
        lines.push(format!("  {}", render_fk_inline(fk)));
    }
    let body = lines.join(",\n");
    format!(
        "CREATE TABLE [{}].[{}] (\n{}\n);",
        t.schema, t.name, body
    )
}

fn render_column_def(col: &DesignColumn) -> String {
    // Computed columns take a completely different shape: no type, no NULL,
    // no default — just the AS clause. IDENTITY on a computed column is
    // illegal so we ignore that flag too.
    if col.is_computed {
        let expr = col.computed_expression.as_deref().unwrap_or("NULL");
        return format!("[{}] AS ({})", col.name, expr);
    }
    let mut parts = vec![format!("[{}] {}", col.name, col.sql_type)];
    if col.is_identity {
        parts.push("IDENTITY(1,1)".into());
    }
    parts.push(if col.is_nullable { "NULL".into() } else { "NOT NULL".into() });
    if let Some(def) = col.default_expression.as_ref() {
        parts.push(format!("DEFAULT {def}"));
    }
    parts.join(" ")
}

fn render_fk_inline(fk: &DesignForeignKey) -> String {
    let cols = fk
        .columns
        .iter()
        .map(|c| format!("[{c}]"))
        .collect::<Vec<_>>()
        .join(", ");
    let ref_cols = fk
        .referenced_columns
        .iter()
        .map(|c| format!("[{c}]"))
        .collect::<Vec<_>>()
        .join(", ");
    let mut s = format!(
        "CONSTRAINT [{}] FOREIGN KEY ({}) REFERENCES [{}].[{}] ({})",
        fk.name, cols, fk.referenced_schema, fk.referenced_table, ref_cols
    );
    if let Some(a) = fk.on_delete.as_deref() {
        s.push_str(&format!(" ON DELETE {a}"));
    }
    if let Some(a) = fk.on_update.as_deref() {
        s.push_str(&format!(" ON UPDATE {a}"));
    }
    s
}

fn render_alter(cur: &TableDesign, next: &TableDesign) -> Vec<DdlStatement> {
    let mut out: Vec<DdlStatement> = Vec::new();

    let cur_cols: BTreeMap<String, &DesignColumn> =
        cur.columns.iter().map(|c| (c.name.clone(), c)).collect();
    let next_cols: BTreeMap<String, &DesignColumn> =
        next.columns.iter().map(|c| (c.name.clone(), c)).collect();

    // Preserve declared order for adds/drops so the DDL reads top-down.
    let cur_order: Vec<&str> = cur.columns.iter().map(|c| c.name.as_str()).collect();
    let next_order: Vec<&str> = next.columns.iter().map(|c| c.name.as_str()).collect();

    // Drop columns not in the new shape. Do these before ADD/ALTER so a
    // rename-via-drop-add ordering matches server evaluation.
    for name in &cur_order {
        if !next_cols.contains_key(*name) {
            out.push(DdlStatement {
                kind: "ALTER".into(),
                label: format!("DROP COLUMN [{name}]"),
                sql: format!(
                    "ALTER TABLE [{}].[{}] DROP COLUMN [{}];",
                    next.schema, next.name, name
                ),
            });
        }
    }

    // Add new columns.
    for name in &next_order {
        if !cur_cols.contains_key(*name) {
            let Some(col) = next_cols.get(*name) else { continue };
            out.push(DdlStatement {
                kind: "ALTER".into(),
                label: format!("ADD COLUMN [{name}]"),
                sql: format!(
                    "ALTER TABLE [{}].[{}] ADD {};",
                    next.schema,
                    next.name,
                    render_column_def(col)
                ),
            });
        }
    }

    // ALTER shared columns whose type / nullability changed. Identity and
    // computed flag transitions require drop+add + data migration in real
    // life — we surface them as commented no-ops so the user sees they were
    // detected but nothing dangerous ships silently.
    for name in &next_order {
        let (Some(next_col), Some(cur_col)) = (next_cols.get(*name), cur_cols.get(*name)) else {
            continue;
        };
        if next_col == cur_col {
            continue;
        }
        if next_col.is_identity != cur_col.is_identity
            || next_col.is_computed != cur_col.is_computed
        {
            out.push(DdlStatement {
                kind: "ALTER".into(),
                label: format!("SKIP [{name}] — identity/computed transition"),
                sql: format!(
                    "-- [{name}] identity/computed flag changed; drop+recreate + data migration required — not auto-generated",
                ),
            });
            continue;
        }
        if next_col.sql_type != cur_col.sql_type
            || next_col.is_nullable != cur_col.is_nullable
        {
            let nullable = if next_col.is_nullable { "NULL" } else { "NOT NULL" };
            out.push(DdlStatement {
                kind: "ALTER".into(),
                label: format!("ALTER COLUMN [{name}]"),
                sql: format!(
                    "ALTER TABLE [{}].[{}] ALTER COLUMN [{}] {} {};",
                    next.schema, next.name, name, next_col.sql_type, nullable
                ),
            });
        }
        // Default expression drift: drop the old default constraint then add
        // the new one. Requires knowing the constraint name; SQL Server auto-
        // generates DF_<table>_<col>_<hex>, which we can't discover from the
        // TableDesign shape alone, so we emit a NOTE for the user to hand-edit
        // when the default changed on an existing column.
        if next_col.default_expression != cur_col.default_expression
            && !cur_col.is_computed
            && !next_col.is_computed
        {
            out.push(DdlStatement {
                kind: "ALTER".into(),
                label: format!("NOTE default drift on [{name}]"),
                sql: format!(
                    "-- [{name}] default changed; drop the auto-named DF_ constraint then re-ADD DEFAULT — server names it uniquely, cannot auto-generate",
                ),
            });
        }
    }

    // Primary key: drop & recreate when the key shape changes. SQL Server has
    // no in-place PK alteration.
    if cur.primary_key != next.primary_key || cur.pk_name != next.pk_name {
        if !cur.primary_key.is_empty() {
            let pk_name = cur
                .pk_name
                .clone()
                .unwrap_or_else(|| format!("PK_{}", cur.name));
            out.push(DdlStatement {
                kind: "ALTER".into(),
                label: format!("DROP PRIMARY KEY [{pk_name}]"),
                sql: format!(
                    "ALTER TABLE [{}].[{}] DROP CONSTRAINT [{}];",
                    next.schema, next.name, pk_name
                ),
            });
        }
        if !next.primary_key.is_empty() {
            let cols = next
                .primary_key
                .iter()
                .map(|c| format!("[{c}]"))
                .collect::<Vec<_>>()
                .join(", ");
            let pk_name = next
                .pk_name
                .clone()
                .unwrap_or_else(|| format!("PK_{}", next.name));
            out.push(DdlStatement {
                kind: "ALTER".into(),
                label: format!("ADD PRIMARY KEY [{pk_name}]"),
                sql: format!(
                    "ALTER TABLE [{}].[{}] ADD CONSTRAINT [{}] PRIMARY KEY ({});",
                    next.schema, next.name, pk_name, cols
                ),
            });
        }
    }

    diff_indexes(cur, next, &mut out);
    diff_foreign_keys(cur, next, &mut out);

    out
}

fn diff_indexes(cur: &TableDesign, next: &TableDesign, out: &mut Vec<DdlStatement>) {
    let cur_idx: BTreeMap<String, &DesignIndex> =
        cur.indexes.iter().map(|i| (i.name.clone(), i)).collect();
    let next_idx: BTreeMap<String, &DesignIndex> =
        next.indexes.iter().map(|i| (i.name.clone(), i)).collect();

    for (name, idx) in &cur_idx {
        if !next_idx.contains_key(name) {
            out.push(DdlStatement {
                kind: "DROP".into(),
                label: format!("DROP INDEX [{name}]"),
                sql: format!(
                    "DROP INDEX [{}] ON [{}].[{}];",
                    idx.name, next.schema, next.name
                ),
            });
        }
    }
    for (name, idx) in &next_idx {
        let should_create = match cur_idx.get(name) {
            None => true,
            Some(existing) => *existing != *idx,
        };
        if !should_create {
            continue;
        }
        if cur_idx.contains_key(name) {
            out.push(DdlStatement {
                kind: "DROP".into(),
                label: format!("DROP INDEX [{name}]"),
                sql: format!(
                    "DROP INDEX [{}] ON [{}].[{}];",
                    name, next.schema, next.name
                ),
            });
        }
        out.push(DdlStatement {
            kind: "CREATE".into(),
            label: format!("CREATE INDEX [{name}]"),
            sql: render_create_index(&next.schema, &next.name, idx),
        });
    }
}

fn diff_foreign_keys(cur: &TableDesign, next: &TableDesign, out: &mut Vec<DdlStatement>) {
    let cur_fks: BTreeMap<String, &DesignForeignKey> =
        cur.foreign_keys.iter().map(|f| (f.name.clone(), f)).collect();
    let next_fks: BTreeMap<String, &DesignForeignKey> =
        next.foreign_keys.iter().map(|f| (f.name.clone(), f)).collect();

    for name in cur_fks.keys() {
        if !next_fks.contains_key(name) {
            out.push(DdlStatement {
                kind: "ALTER".into(),
                label: format!("DROP FOREIGN KEY [{name}]"),
                sql: format!(
                    "ALTER TABLE [{}].[{}] DROP CONSTRAINT [{}];",
                    next.schema, next.name, name
                ),
            });
        }
    }
    for (name, fk) in &next_fks {
        let should_add = match cur_fks.get(name) {
            None => true,
            Some(existing) => *existing != *fk,
        };
        if !should_add {
            continue;
        }
        if cur_fks.contains_key(name) {
            out.push(DdlStatement {
                kind: "ALTER".into(),
                label: format!("DROP FOREIGN KEY [{name}]"),
                sql: format!(
                    "ALTER TABLE [{}].[{}] DROP CONSTRAINT [{}];",
                    next.schema, next.name, name
                ),
            });
        }
        out.push(DdlStatement {
            kind: "ALTER".into(),
            label: format!("ADD FOREIGN KEY [{name}]"),
            sql: format!(
                "ALTER TABLE [{}].[{}] ADD {};",
                next.schema,
                next.name,
                render_fk_inline(fk)
            ),
        });
    }
}

fn render_create_index(schema: &str, table: &str, idx: &DesignIndex) -> String {
    let cols = idx
        .columns
        .iter()
        .map(|c| format!("[{c}]"))
        .collect::<Vec<_>>()
        .join(", ");
    let unique = if idx.is_unique { "UNIQUE " } else { "" };
    format!(
        "CREATE {unique}INDEX [{}] ON [{}].[{}] ({});",
        idx.name, schema, table, cols
    )
}
