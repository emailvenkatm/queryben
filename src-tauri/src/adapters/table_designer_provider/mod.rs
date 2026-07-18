//! Provider abstraction for the Table Designer.
//!
//! `load` pulls the current shape from the DB into a `TableDesign` the UI can
//! edit; `generate_ddl` compares the (optional) `current` and the user's `next`
//! and emits a review-ready DDL script. Apply runs those statements in a
//! transaction — see `commands::table_designer::apply_table_ddl`.
//!
//! v1 ships with `SqlServerTableDesignerProvider` only. MySql / Postgres
//! implementations of the trait land alongside their engine drivers.

mod ddl;
mod load;
mod sql;

use async_trait::async_trait;

use crate::adapters::mssql::MssqlClient;
use crate::core::table_design::{DdlStatement, TableDesign};
use crate::error::AppError;

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

#[async_trait]
impl TableDesignerProvider for SqlServerTableDesignerProvider {
    async fn load(
        &self,
        client: &mut MssqlClient,
        schema: &str,
        name: &str,
    ) -> Result<TableDesign, AppError> {
        load::load(client, schema, name).await
    }

    fn generate_ddl(
        &self,
        current: Option<&TableDesign>,
        next: &TableDesign,
    ) -> Vec<DdlStatement> {
        ddl::generate_ddl(current, next)
    }
}

// ---- unimplemented engines -------------------------------------------------
// Placeholder impls so the trait registry compiles ahead of the real drivers.

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
