//! Object-scripter provider abstraction.
//!
//! SSMS-parity "Script as CREATE / ALTER / DROP / SELECT / INSERT" flow.
//! Given a `SchemaObjectRef` (kind + schema + name) and a `ScriptAction`,
//! return a SQL string the frontend drops into a fresh query tab.
//!
//! The provider trait is engine-agnostic; `SqlServerObjectScripter` is the
//! only real impl in v1. For table CREATE we reuse
//! `SqlServerTableDesignerProvider::load` + `render_create_table` (via the
//! provider's `generate_ddl(None, &next)` path) rather than duplicating the
//! sys.* introspection. For view/proc/function CREATE we `OBJECT_DEFINITION`
//! passthrough — warts and all, matches ADS behavior.

use async_trait::async_trait;
use std::path::Path;
use tiberius::{Query, Row};

use crate::core::object_script::{ObjectKind, SchemaObjectRef};
use crate::core::table_design::TableDesign;
use crate::error::AppError;
use crate::adapters::mssql::MssqlClient;
use crate::adapters::table_designer_provider::{
    SqlServerTableDesignerProvider, TableDesignerProvider,
};

// ---- config ---------------------------------------------------------------

const CONFIG_FILE: &str = "scripter.config.json";

/// Options loaded from `<app_data_dir>/scripter.config.json`. Defaults are the
/// SSMS-parity choices most teams use — bracket identifiers, schema-qualified,
/// `IF EXISTS` guard on DROP, `NULL` placeholder in INSERT templates.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ScripterOptions {
    #[serde(default = "default_true")]
    pub bracket_identifiers: bool,
    #[serde(default = "default_true")]
    pub include_schema_prefix: bool,
    #[serde(default = "default_true")]
    pub include_drop_if_exists_guard: bool,
    #[serde(default = "default_placeholder")]
    pub insert_template_placeholder: String,
}

impl Default for ScripterOptions {
    fn default() -> Self {
        Self {
            bracket_identifiers: true,
            include_schema_prefix: true,
            include_drop_if_exists_guard: true,
            insert_template_placeholder: default_placeholder(),
        }
    }
}

fn default_true() -> bool {
    true
}
fn default_placeholder() -> String {
    "NULL".into()
}

impl ScripterOptions {
    /// Read `scripter.config.json` from `app_data_dir`. Any failure short-
    /// circuits to `Default` — the Script-as menu must not go dark because of
    /// a stray trailing comma in a config file.
    pub fn load(app_data_dir: &Path) -> Self {
        let path = app_data_dir.join(CONFIG_FILE);
        let Ok(bytes) = std::fs::read(&path) else {
            return Self::default();
        };
        match serde_json::from_slice::<ScripterOptions>(&bytes) {
            Ok(cfg) => cfg,
            Err(err) => {
                tracing::warn!(
                    target: "queryben::scripter",
                    ?path,
                    error = %err,
                    "scripter.config.json malformed; using defaults"
                );
                Self::default()
            }
        }
    }

    /// Render an identifier respecting `bracket_identifiers`.
    pub fn ident(&self, name: &str) -> String {
        if self.bracket_identifiers {
            format!("[{name}]")
        } else {
            name.to_string()
        }
    }

    /// Render a schema-qualified name respecting both flags.
    pub fn qualified(&self, schema: &str, name: &str) -> String {
        if self.include_schema_prefix {
            format!("{}.{}", self.ident(schema), self.ident(name))
        } else {
            self.ident(name)
        }
    }
}

// ---- trait ----------------------------------------------------------------

#[async_trait]
pub trait ObjectScripter: Send + Sync {
    async fn script_create(
        &self,
        client: &mut MssqlClient,
        obj: &SchemaObjectRef,
    ) -> Result<String, AppError>;

    async fn script_alter(
        &self,
        client: &mut MssqlClient,
        obj: &SchemaObjectRef,
    ) -> Result<String, AppError>;

    fn script_drop(&self, obj: &SchemaObjectRef) -> String;

    async fn script_insert_template(
        &self,
        client: &mut MssqlClient,
        obj: &SchemaObjectRef,
    ) -> Result<String, AppError>;
}

// ---- SQL Server -----------------------------------------------------------

pub struct SqlServerObjectScripter {
    pub options: ScripterOptions,
}

impl SqlServerObjectScripter {
    pub fn new(options: ScripterOptions) -> Self {
        Self { options }
    }
}

// Column list used by the INSERT template. Skips IDENTITY (server refuses
// explicit values without SET IDENTITY_INSERT) and computed columns (server
// disallows explicit values entirely).
const INSERT_COLUMNS_SQL: &str = "SELECT c.name,
       c.is_identity,
       c.is_computed
  FROM sys.columns AS c
  JOIN sys.tables  AS t ON t.object_id = c.object_id
  JOIN sys.schemas AS s ON s.schema_id = t.schema_id
 WHERE s.name = @P1
   AND t.name = @P2
 ORDER BY c.column_id";

// OBJECT_DEFINITION returns the source text for views / procs / functions /
// triggers. NULL for tables / indexes — hence the branch in `script_create`.
const OBJECT_DEFINITION_SQL: &str =
    "SELECT OBJECT_DEFINITION(OBJECT_ID(@P1)) AS body";

// For DROP INDEX on Index objects. We take `table` from the ref, so this is
// only used when the caller supplied it.
#[async_trait]
impl ObjectScripter for SqlServerObjectScripter {
    async fn script_create(
        &self,
        client: &mut MssqlClient,
        obj: &SchemaObjectRef,
    ) -> Result<String, AppError> {
        match obj.kind {
            ObjectKind::Table => {
                // Reuse the table-designer provider: `load` pulls the sys.*
                // shape into a TableDesign, `generate_ddl(None, &design)`
                // renders the same CREATE TABLE string the designer preview
                // shows. Any DDL improvement to the designer flows through.
                let provider = SqlServerTableDesignerProvider;
                let design: TableDesign =
                    provider.load(client, &obj.schema, &obj.name).await?;
                let stmts = provider.generate_ddl(None, &design);
                let sql = stmts
                    .first()
                    .map(|s| s.sql.clone())
                    .unwrap_or_else(|| "-- table designer returned no CREATE".into());
                Ok(sql)
            }
            ObjectKind::View | ObjectKind::Procedure | ObjectKind::Function => {
                let body = fetch_object_definition(client, &obj.schema, &obj.name).await?;
                Ok(body.trim().to_string())
            }
            ObjectKind::Index => {
                // sys.indexes → CREATE [UNIQUE] INDEX. Requires the parent
                // table name; the caller should have populated obj.table.
                let table = obj.table.as_deref().ok_or_else(|| {
                    AppError::internal(
                        "index script requires parent table in SchemaObjectRef.table",
                    )
                })?;
                script_create_index(client, &obj.schema, table, &obj.name, &self.options).await
            }
        }
    }

    async fn script_alter(
        &self,
        client: &mut MssqlClient,
        obj: &SchemaObjectRef,
    ) -> Result<String, AppError> {
        match obj.kind {
            ObjectKind::View | ObjectKind::Procedure | ObjectKind::Function => {
                // OBJECT_DEFINITION always returns "CREATE …"; rewrite leading
                // CREATE to ALTER so the user can hit F5 to replace in place.
                let body = fetch_object_definition(client, &obj.schema, &obj.name).await?;
                Ok(rewrite_create_to_alter(&body, obj.kind.ddl_keyword()))
            }
            ObjectKind::Table => {
                // No fully-general "ALTER TABLE that preserves everything"
                // exists in SQL Server. Emit an ALTER template with the target
                // table already filled in so the user drops in ADD/DROP/ALTER
                // COLUMN clauses.
                let q = self.options.qualified(&obj.schema, &obj.name);
                Ok(format!(
                    "-- SQL Server has no ALTER-in-place; add/drop columns explicitly:\nALTER TABLE {q}\n  -- ADD COLUMN [name] TYPE NULL,\n  -- DROP COLUMN [name],\n  -- ALTER COLUMN [name] TYPE NOT NULL\n;",
                ))
            }
            ObjectKind::Index => {
                // ALTER INDEX in SQL Server only rebuilds/reorganizes — the
                // column list is fixed. Emit the REBUILD form; users wanting
                // a structural change do DROP + CREATE.
                let table = obj.table.as_deref().ok_or_else(|| {
                    AppError::internal(
                        "index script requires parent table in SchemaObjectRef.table",
                    )
                })?;
                let idx = self.options.ident(&obj.name);
                let tbl = self.options.qualified(&obj.schema, table);
                Ok(format!("ALTER INDEX {idx} ON {tbl} REBUILD;"))
            }
        }
    }

    fn script_drop(&self, obj: &SchemaObjectRef) -> String {
        render_drop(obj, &self.options)
    }

    async fn script_insert_template(
        &self,
        client: &mut MssqlClient,
        obj: &SchemaObjectRef,
    ) -> Result<String, AppError> {
        if !matches!(obj.kind, ObjectKind::Table) {
            return Err(AppError::internal(
                "INSERT template is only supported for tables",
            ));
        }
        let cols = fetch_insertable_columns(client, &obj.schema, &obj.name).await?;
        Ok(render_insert_template(&obj.schema, &obj.name, &cols, &self.options))
    }
}

// ---- CREATE INDEX (live-DB) -----------------------------------------------

async fn script_create_index(
    client: &mut MssqlClient,
    schema: &str,
    table: &str,
    index_name: &str,
    opts: &ScripterOptions,
) -> Result<String, AppError> {
    // Pull the index shape from sys.indexes + sys.index_columns for the given
    // {schema, table, index}. STRING_AGG (SQL 2017+) collapses key columns
    // into a single row so the fetch is a single scalar-ish result.
    const IDX_SQL: &str = "SELECT i.is_unique,
              (SELECT STRING_AGG(c.name, ',') WITHIN GROUP (ORDER BY ic.key_ordinal)
                 FROM sys.index_columns AS ic
                 JOIN sys.columns AS c
                   ON c.object_id = ic.object_id AND c.column_id = ic.column_id
                WHERE ic.object_id = i.object_id
                  AND ic.index_id = i.index_id
                  AND ic.is_included_column = 0) AS key_columns,
              (SELECT STRING_AGG(c.name, ',') WITHIN GROUP (ORDER BY ic.key_ordinal)
                 FROM sys.index_columns AS ic
                 JOIN sys.columns AS c
                   ON c.object_id = ic.object_id AND c.column_id = ic.column_id
                WHERE ic.object_id = i.object_id
                  AND ic.index_id = i.index_id
                  AND ic.is_included_column = 1) AS include_columns
         FROM sys.indexes AS i
         JOIN sys.tables  AS t ON t.object_id = i.object_id
         JOIN sys.schemas AS s ON s.schema_id = t.schema_id
        WHERE s.name = @P1
          AND t.name = @P2
          AND i.name = @P3";

    let mut q = Query::new(IDX_SQL);
    q.bind(schema.to_string());
    q.bind(table.to_string());
    q.bind(index_name.to_string());
    let rows = q.query(client).await?.into_first_result().await?;
    let Some(row) = rows.into_iter().next() else {
        return Err(AppError::NotFound(format!(
            "index {schema}.{table}.{index_name} not found"
        )));
    };
    let is_unique = row.try_get::<bool, _>(0).map_err(AppError::from)?.unwrap_or(false);
    let key_cols_raw = row
        .try_get::<&str, _>(1)
        .map_err(AppError::from)?
        .unwrap_or("");
    let inc_cols_raw = row
        .try_get::<&str, _>(2)
        .map_err(AppError::from)?
        .unwrap_or("");
    let key_cols = split_csv_render(key_cols_raw, opts);
    let inc_cols = split_csv_render(inc_cols_raw, opts);

    let unique = if is_unique { "UNIQUE " } else { "" };
    let idx = opts.ident(index_name);
    let tbl = opts.qualified(schema, table);
    let include = if inc_cols.is_empty() {
        String::new()
    } else {
        format!(" INCLUDE ({inc_cols})")
    };
    Ok(format!(
        "CREATE {unique}INDEX {idx} ON {tbl} ({key_cols}){include};"
    ))
}

fn split_csv_render(raw: &str, opts: &ScripterOptions) -> String {
    raw.split(',')
        .filter(|p| !p.is_empty())
        .map(|p| opts.ident(p.trim()))
        .collect::<Vec<_>>()
        .join(", ")
}

// ---- OBJECT_DEFINITION ----------------------------------------------------

async fn fetch_object_definition(
    client: &mut MssqlClient,
    schema: &str,
    name: &str,
) -> Result<String, AppError> {
    let mut q = Query::new(OBJECT_DEFINITION_SQL);
    // OBJECT_ID accepts "[schema].[name]". Both are safely wrapped so
    // reserved words / dots don't break the lookup.
    q.bind(format!("[{schema}].[{name}]"));
    let rows = q.query(client).await?.into_first_result().await?;
    let Some(row) = rows.into_iter().next() else {
        return Err(AppError::NotFound(format!(
            "object {schema}.{name} not found"
        )));
    };
    match row.try_get::<&str, _>(0).map_err(AppError::from)? {
        Some(body) if !body.trim().is_empty() => Ok(body.to_string()),
        _ => Err(AppError::NotFound(format!(
            "OBJECT_DEFINITION returned NULL for {schema}.{name} (encrypted or non-scriptable)"
        ))),
    }
}

// SQL Server's OBJECT_DEFINITION always ships "CREATE …". For ALTER we want
// the same body with "CREATE" → "ALTER" on the first occurrence only.
// Case-insensitive match on the leading keyword. Copied intent from
// `schema_provider::rewrite_create_to_alter` — kept local so the two paths
// evolve independently.
fn rewrite_create_to_alter(body: &str, keyword: &str) -> String {
    let lower = body.to_ascii_lowercase();
    if let Some(idx) = lower.find("create") {
        let after = &body[idx + "create".len()..];
        let after_trim = after.trim_start();
        if after_trim
            .to_ascii_lowercase()
            .starts_with(&keyword.to_ascii_lowercase())
        {
            let mut out = String::with_capacity(body.len());
            out.push_str(&body[..idx]);
            out.push_str("ALTER");
            out.push_str(after);
            return out;
        }
    }
    format!("-- rewrite fallback (body didn't start with CREATE {keyword})\n{body}")
}

// ---- INSERT TEMPLATE ------------------------------------------------------

async fn fetch_insertable_columns(
    client: &mut MssqlClient,
    schema: &str,
    name: &str,
) -> Result<Vec<String>, AppError> {
    let mut q = Query::new(INSERT_COLUMNS_SQL);
    q.bind(schema.to_string());
    q.bind(name.to_string());
    let rows = q.query(client).await?.into_first_result().await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let name: Option<&str> = row.try_get(0).map_err(AppError::from)?;
        let is_identity = row.try_get::<bool, _>(1).map_err(AppError::from)?.unwrap_or(false);
        let is_computed = row.try_get::<bool, _>(2).map_err(AppError::from)?.unwrap_or(false);
        if is_identity || is_computed {
            continue;
        }
        if let Some(n) = name {
            out.push(n.to_string());
        }
    }
    if out.is_empty() {
        return Err(AppError::NotFound(format!(
            "table {schema}.{name} has no insertable columns"
        )));
    }
    Ok(out)
}

// Pure renderer for the INSERT template. Extracted so unit tests can exercise
// it without a live database.
pub fn render_insert_template(
    schema: &str,
    name: &str,
    columns: &[String],
    opts: &ScripterOptions,
) -> String {
    let target = opts.qualified(schema, name);
    let col_list = columns
        .iter()
        .map(|c| opts.ident(c))
        .collect::<Vec<_>>()
        .join(", ");
    let placeholders = std::iter::repeat(opts.insert_template_placeholder.as_str())
        .take(columns.len())
        .collect::<Vec<_>>()
        .join(", ");
    format!("INSERT INTO {target} ({col_list})\nVALUES ({placeholders});")
}

// ---- DROP -----------------------------------------------------------------

fn render_drop(obj: &SchemaObjectRef, opts: &ScripterOptions) -> String {
    match obj.kind {
        ObjectKind::Table | ObjectKind::View | ObjectKind::Procedure | ObjectKind::Function => {
            let kw = obj.kind.ddl_keyword();
            let q = opts.qualified(&obj.schema, &obj.name);
            if opts.include_drop_if_exists_guard {
                format!("DROP {kw} IF EXISTS {q};")
            } else {
                format!("DROP {kw} {q};")
            }
        }
        ObjectKind::Index => {
            let idx = opts.ident(&obj.name);
            // DROP INDEX requires the parent table. If it's missing we still
            // emit best-effort DDL so the user sees a clear placeholder they
            // can hand-edit.
            let table_ref = match obj.table.as_deref() {
                Some(t) => opts.qualified(&obj.schema, t),
                None => "[<table>]".to_string(),
            };
            if opts.include_drop_if_exists_guard {
                format!("DROP INDEX IF EXISTS {idx} ON {table_ref};")
            } else {
                format!("DROP INDEX {idx} ON {table_ref};")
            }
        }
    }
}

// Pure renderer for SELECT TOP 100. Extracted so the command layer can call
// it without touching the trait (no DB IO needed).
pub fn render_select_top(obj: &SchemaObjectRef, opts: &ScripterOptions, top_n: u32) -> String {
    let q = opts.qualified(&obj.schema, &obj.name);
    format!("SELECT TOP {top_n} * FROM {q};")
}

// Pure renderer for DROP AND CREATE. Composes `render_drop` + a supplied
// CREATE body (fetched from the live provider). Splits so the frontend gets
// one string with both statements separated by GO in SSMS-style; we skip GO
// because our transaction runner would treat it as a syntax error.
pub fn render_drop_and_create(drop_sql: &str, create_sql: &str) -> String {
    format!("{}\n\n{}", drop_sql.trim_end(), create_sql.trim_start())
}

// Row extractor pattern used elsewhere in this crate. Local so we don't reach
// into another infra module for a helper.
#[allow(dead_code)]
fn str_col(row: &Row, idx: usize, label: &str) -> Result<String, AppError> {
    match row.try_get::<&str, _>(idx).map_err(AppError::from)? {
        Some(s) => Ok(s.to_string()),
        None => Err(AppError::internal(format!(
            "scripter row column {label} (idx {idx}) was NULL"
        ))),
    }
}
