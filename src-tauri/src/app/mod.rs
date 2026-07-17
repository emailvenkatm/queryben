// Use-case orchestrators. Thin glue between the IPC surface and the adapters.

pub mod execute_query;
pub mod execute_transaction;
pub mod introspect;
pub mod row_convert;
pub mod session;
