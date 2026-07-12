//! Provider abstraction for the Table Designer.
//!
//! `load` pulls the current shape from the DB into a `TableDesign` the UI can
//! edit; `generate_ddl` compares the (optional) `current` and the user's `next`
//! and emits a review-ready DDL script. Apply runs those statements in a
//! transaction — see `commands::table_designer::apply_table_ddl`.
//!
//! For v1 only `SqlServerTableDesignerProvider` is real. MySql / Postgres stubs
//! ship so the future dispatch on connection engine is a one-line swap.

use async_trait::async_trait;
use std::collections::BTreeMap;
use tiberius::{Query, Row};

use crate::core::table_design::{
    DdlStatement, DesignColumn, DesignForeignKey, DesignIndex, TableDesign,
};
use crate::error::AppError;
use crate::adapters::mssql::MssqlClient;

// ---- config ---------------------------------------------------------------

// Options loaded from `<app_data_dir>/designer.config.json`. Defaults are the
// safe MSSQL choices most teams use.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DesignerOptions {
    #[serde(default = "default_string_type")]
    pub default_string_type: String,
    #[serde(default = "default_int_type")]
    pub default_int_type: String,
    #[serde(default = "default_true")]
    pub wrap_in_transaction: bool,
    #[serde(default)]
    pub generate_drop_first_for_new_indexes: bool,
}

impl Default for DesignerOptions {
    fn default() -> Self {
        Self {
            default_string_type: default_string_type(),
            default_int_type: default_int_type(),
            wrap_in_transaction: true,
            generate_drop_first_for_new_indexes: false,
        }
    }
}

fn default_string_type() -> String {
    "NVARCHAR(255)".into()
}
fn default_int_type() -> String {
    "INT".into()
}
fn default_true() -> bool {
    true
}

// ---- trait ----------------------------------------------------------------

#[async_trait]
pub trait TableDesignerProvider: Send + Sync {
    async fn load(
        &self,
        client: &mut MssqlClient,
        schema: &str,
        name: &str,
    ) -> Result<TableDesign, AppError>;

    fn generate_ddl(
        &self,
        current: Option<&TableDesign>,
        next: &TableDesign,
    ) -> Vec<DdlStatement>;
}

// ---- SQL Server -----------------------------------------------------------

pub struct SqlServerTableDesignerProvider;

// Column metadata. Same shape as `commands::query::COLUMNS_SQL` but pulls the
// computed_column expression when the column is computed. sys.computed_columns
// is the source of truth for the formula; INFORMATION_SCHEMA doesn't expose
// it.
const COLUMNS_SQL: &str = "SELECT c.name AS column_name,
       TYPE_NAME(c.user_type_id) AS data_type,
       c.max_length,
       c.precision,
       c.scale,
       c.is_nullable,
       c.is_identity,
       c.is_computed,
       cc.definition AS computed_expr,
       OBJECT_DEFINITION(dc.object_id) AS default_expr,
       c.column_id
  FROM sys.columns AS c
  JOIN sys.tables  AS t ON t.object_id = c.object_id
  JOIN sys.schemas AS s ON s.schema_id = t.schema_id
  LEFT JOIN sys.computed_columns AS cc
         ON cc.object_id = c.object_id AND cc.column_id = c.column_id
  LEFT JOIN sys.default_constraints AS dc
         ON dc.parent_object_id = c.object_id
        AND dc.parent_column_id = c.column_id
 WHERE s.name = @P1
   AND t.name = @P2
 ORDER BY c.column_id";

// Primary key columns in key order. Also grabs the constraint name so DROP can
// name it explicitly (SQL Server auto-generates PK_<table>_<hex> otherwise).
const PK_SQL: &str = "SELECT i.name AS pk_name,
       c.name AS column_name,
       ic.key_ordinal
  FROM sys.indexes AS i
  JOIN sys.index_columns AS ic
    ON ic.object_id = i.object_id AND ic.index_id = i.index_id
  JOIN sys.columns AS c
    ON c.object_id = ic.object_id AND c.column_id = ic.column_id
  JOIN sys.tables AS t ON t.object_id = i.object_id
  JOIN sys.schemas AS s ON s.schema_id = t.schema_id
 WHERE i.is_primary_key = 1
   AND s.name = @P1
   AND t.name = @P2
 ORDER BY ic.key_ordinal";

// Non-PK indexes with their key columns. STRING_AGG (SQL 2017+) works on
// Azure SQL v12; older on-prem instances may not have it, but v1 targets
// modern SQL Server exclusively.
const IDX_SQL: &str = "SELECT i.name AS index_name,
       i.is_unique,
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
   AND i.is_primary_key = 0
   AND i.is_unique_constraint = 0
   AND s.name = @P1
   AND t.name = @P2";

// Foreign key constraints. One row per FK — the referenced/child column lists
// are STRING_AGG'd so a composite FK survives as a single row.
const FK_SQL: &str = "SELECT fk.name AS fk_name,
       rs.name AS referenced_schema,
       rt.name AS referenced_table,
       (SELECT STRING_AGG(pc.name, ',') WITHIN GROUP (ORDER BY fkc.constraint_column_id)
          FROM sys.foreign_key_columns AS fkc
          JOIN sys.columns AS pc
            ON pc.object_id = fkc.parent_object_id
           AND pc.column_id = fkc.parent_column_id
         WHERE fkc.constraint_object_id = fk.object_id) AS parent_columns,
       (SELECT STRING_AGG(rc.name, ',') WITHIN GROUP (ORDER BY fkc.constraint_column_id)
          FROM sys.foreign_key_columns AS fkc
          JOIN sys.columns AS rc
            ON rc.object_id = fkc.referenced_object_id
           AND rc.column_id = fkc.referenced_column_id
         WHERE fkc.constraint_object_id = fk.object_id) AS referenced_columns,
       fk.delete_referential_action_desc,
       fk.update_referential_action_desc
  FROM sys.foreign_keys AS fk
  JOIN sys.tables AS t ON t.object_id = fk.parent_object_id
  JOIN sys.schemas AS s ON s.schema_id = t.schema_id
  JOIN sys.tables AS rt ON rt.object_id = fk.referenced_object_id
  JOIN sys.schemas AS rs ON rs.schema_id = rt.schema_id
 WHERE s.name = @P1
   AND t.name = @P2";

#[async_trait]
impl TableDesignerProvider for SqlServerTableDesignerProvider {
    async fn load(
        &self,
        client: &mut MssqlClient,
        schema: &str,
        name: &str,
    ) -> Result<TableDesign, AppError> {
        // Columns
        let mut col_q = Query::new(COLUMNS_SQL);
        col_q.bind(schema.to_string());
        col_q.bind(name.to_string());
        let col_rows = col_q.query(client).await?.into_first_result().await?;
        let mut columns: Vec<DesignColumn> = Vec::with_capacity(col_rows.len());
        for row in col_rows {
            let col_name = str_col(&row, 0, "column_name")?;
            let data_type = str_col(&row, 1, "data_type")?;
            let max_length = row
                .try_get::<i16, _>(2)
                .map_err(AppError::from)?
                .unwrap_or(0) as i32;
            let precision = row.try_get::<u8, _>(3).map_err(AppError::from)?.unwrap_or(0) as i32;
            let scale = row.try_get::<u8, _>(4).map_err(AppError::from)?.unwrap_or(0) as i32;
            let is_nullable = row
                .try_get::<bool, _>(5)
                .map_err(AppError::from)?
                .unwrap_or(true);
            let is_identity = row
                .try_get::<bool, _>(6)
                .map_err(AppError::from)?
                .unwrap_or(false);
            let is_computed = row
                .try_get::<bool, _>(7)
                .map_err(AppError::from)?
                .unwrap_or(false);
            let computed_expression = row
                .try_get::<&str, _>(8)
                .map_err(AppError::from)?
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            let default_expression = row
                .try_get::<&str, _>(9)
                .map_err(AppError::from)?
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            let ordinal = row
                .try_get::<i32, _>(10)
                .map_err(AppError::from)?
                .unwrap_or(0)
                .max(0) as u32;

            columns.push(DesignColumn {
                name: col_name,
                sql_type: format_sql_type(&data_type, max_length, precision, scale),
                is_nullable,
                is_identity,
                is_computed,
                computed_expression,
                default_expression,
                ordinal,
            });
        }

        // Primary key
        let mut pk_q = Query::new(PK_SQL);
        pk_q.bind(schema.to_string());
        pk_q.bind(name.to_string());
        let pk_rows = pk_q.query(client).await?.into_first_result().await?;
        let mut primary_key: Vec<String> = Vec::new();
        let mut pk_name: Option<String> = None;
        for row in pk_rows {
            if pk_name.is_none() {
                pk_name = row
                    .try_get::<&str, _>(0)
                    .map_err(AppError::from)?
                    .map(|s| s.to_string());
            }
            let col = str_col(&row, 1, "column_name")?;
            primary_key.push(col);
        }

        // Indexes (non-PK)
        let mut idx_q = Query::new(IDX_SQL);
        idx_q.bind(schema.to_string());
        idx_q.bind(name.to_string());
        let idx_rows = idx_q.query(client).await?.into_first_result().await?;
        let mut indexes: Vec<DesignIndex> = Vec::with_capacity(idx_rows.len());
        for row in idx_rows {
            let idx_name = str_col(&row, 0, "index_name")?;
            let is_unique = row
                .try_get::<bool, _>(1)
                .map_err(AppError::from)?
                .unwrap_or(false);
            let key_columns_str = row
                .try_get::<&str, _>(2)
                .map_err(AppError::from)?
                .unwrap_or("")
                .to_string();
            let cols: Vec<String> = key_columns_str
                .split(',')
                .filter(|s| !s.is_empty())
                .map(|s| s.trim().to_string())
                .collect();
            indexes.push(DesignIndex {
                name: idx_name,
                is_unique,
                columns: cols,
            });
        }

        // Foreign keys
        let mut fk_q = Query::new(FK_SQL);
        fk_q.bind(schema.to_string());
        fk_q.bind(name.to_string());
        let fk_rows = fk_q.query(client).await?.into_first_result().await?;
        let mut foreign_keys: Vec<DesignForeignKey> = Vec::with_capacity(fk_rows.len());
        for row in fk_rows {
            let fk_name = str_col(&row, 0, "fk_name")?;
            let ref_schema = str_col(&row, 1, "referenced_schema")?;
            let ref_table = str_col(&row, 2, "referenced_table")?;
            let parent_columns_str = row
                .try_get::<&str, _>(3)
                .map_err(AppError::from)?
                .unwrap_or("")
                .to_string();
            let referenced_columns_str = row
                .try_get::<&str, _>(4)
                .map_err(AppError::from)?
                .unwrap_or("")
                .to_string();
            let on_delete = row
                .try_get::<&str, _>(5)
                .map_err(AppError::from)?
                .map(|s| normalize_ref_action(s));
            let on_update = row
                .try_get::<&str, _>(6)
                .map_err(AppError::from)?
                .map(|s| normalize_ref_action(s));
            foreign_keys.push(DesignForeignKey {
                name: fk_name,
                columns: split_csv(&parent_columns_str),
                referenced_schema: ref_schema,
                referenced_table: ref_table,
                referenced_columns: split_csv(&referenced_columns_str),
                on_delete,
                on_update,
            });
        }

        Ok(TableDesign {
            schema: schema.to_string(),
            name: name.to_string(),
            columns,
            primary_key,
            pk_name,
            indexes,
            foreign_keys,
        })
    }

    fn generate_ddl(
        &self,
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
}

// ---- CREATE TABLE ---------------------------------------------------------

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

// ---- ALTER TABLE ----------------------------------------------------------

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

    // Indexes: drop-then-create by name diff. Column-set changes on shared
    // names also force drop+create (SQL Server has no ALTER INDEX for column
    // lists).
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
        // Changed index: drop first then recreate. New index: create only.
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

    // Foreign keys: name-based diff, drop-then-add on change.
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

    out
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

// ---- helpers --------------------------------------------------------------

fn str_col(row: &Row, idx: usize, label: &str) -> Result<String, AppError> {
    match row.try_get::<&str, _>(idx).map_err(AppError::from)? {
        Some(s) => Ok(s.to_string()),
        None => Err(AppError::internal(format!(
            "table-designer row column {label} (idx {idx}) was NULL"
        ))),
    }
}

fn split_csv(s: &str) -> Vec<String> {
    s.split(',')
        .filter(|p| !p.is_empty())
        .map(|p| p.trim().to_string())
        .collect()
}

// sys.foreign_keys returns NO_ACTION / CASCADE / SET_NULL / SET_DEFAULT with
// underscores; DDL uses spaces. NO_ACTION is the default so we drop it entirely.
fn normalize_ref_action(raw: &str) -> String {
    match raw.to_ascii_uppercase().as_str() {
        "NO_ACTION" => "NO ACTION".into(),
        "CASCADE" => "CASCADE".into(),
        "SET_NULL" => "SET NULL".into(),
        "SET_DEFAULT" => "SET DEFAULT".into(),
        other => other.replace('_', " "),
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
            n if n > 0 => format!("{dt}({})", n / 2),
            _ => dt,
        },
        "decimal" | "numeric" => format!("{dt}({precision},{scale})"),
        _ => dt,
    }
}

// ---- stubs for future engines --------------------------------------------

pub struct MysqlTableDesignerProvider;

#[async_trait]
impl TableDesignerProvider for MysqlTableDesignerProvider {
    async fn load(
        &self,
        _client: &mut MssqlClient,
        _schema: &str,
        _name: &str,
    ) -> Result<TableDesign, AppError> {
        todo!("MysqlTableDesignerProvider::load — pending MySQL driver plumbing")
    }
    fn generate_ddl(
        &self,
        _current: Option<&TableDesign>,
        _next: &TableDesign,
    ) -> Vec<DdlStatement> {
        todo!("MysqlTableDesignerProvider::generate_ddl")
    }
}

pub struct PostgresTableDesignerProvider;

#[async_trait]
impl TableDesignerProvider for PostgresTableDesignerProvider {
    async fn load(
        &self,
        _client: &mut MssqlClient,
        _schema: &str,
        _name: &str,
    ) -> Result<TableDesign, AppError> {
        todo!("PostgresTableDesignerProvider::load — pending pgwire driver plumbing")
    }
    fn generate_ddl(
        &self,
        _current: Option<&TableDesign>,
        _next: &TableDesign,
    ) -> Vec<DdlStatement> {
        todo!("PostgresTableDesignerProvider::generate_ddl")
    }
}
