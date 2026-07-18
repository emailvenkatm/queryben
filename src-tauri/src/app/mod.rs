// Use-cases. Thin glue between the IPC surface and the adapters — this is
// where retry, cache, and error mapping live so IPC stays boring.

pub mod execute_query;
pub mod execute_transaction;
pub mod introspect;
pub mod row_convert;
pub mod session;
