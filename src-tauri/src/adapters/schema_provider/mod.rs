//! Provider abstraction for schema-compare. Different engines have wildly
//! different DDL surfaces (sys.* vs pg_catalog vs information_schema.*), so
//! each engine ships its own `SchemaProvider` impl and the compare commands
//! route on connection kind.
//!
//! For v1 only `SqlServerSchemaProvider` is real. MySql / Postgres return
//! `todo!()` so a future crate PR is a plug-in.

mod ddl;
mod diff;
mod snapshot;
mod sql;

use async_trait::async_trait;

use crate::adapters::mssql::MssqlClient;
use crate::core::schema_diff::{DdlStatement, SchemaDiff, SchemaObject};
use crate::error::AppError;

pub use diff::compute_diff;

#[async_trait]
pub trait SchemaProvider: Send + Sync {
    async fn snapshot(&self, client: &mut MssqlClient) -> Result<Vec<SchemaObject>, AppError>;
    fn generate_ddl(&self, diff: &SchemaDiff) -> Vec<DdlStatement>;
}

pub struct SqlServerSchemaProvider;

#[async_trait]
impl SchemaProvider for SqlServerSchemaProvider {
    async fn snapshot(
        &self,
        client: &mut MssqlClient,
    ) -> Result<Vec<SchemaObject>, AppError> {
        snapshot::snapshot_all(client).await
    }

    fn generate_ddl(&self, diff: &SchemaDiff) -> Vec<DdlStatement> {
        ddl::generate_ddl(diff)
    }
}

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
