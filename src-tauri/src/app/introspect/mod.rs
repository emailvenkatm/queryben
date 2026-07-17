//! Object-explorer schema introspection: schemas, tables, views, procs, fns,
//! plus a fast row/column estimate per table.

mod rows;
mod schema;
mod sql;
mod table_metadata;

pub use schema::{get_schema, list_tables};
pub use table_metadata::get_table_metadata;
