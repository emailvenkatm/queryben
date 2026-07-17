//! SQL Server introspection SQL for the Table Designer, plus row/type helpers
//! shared by the load and DDL paths.

use tiberius::Row;

use crate::error::AppError;

// Column metadata. Same shape as `commands::query::COLUMNS_SQL` but pulls the
// computed_column expression when the column is computed. sys.computed_columns
// is the source of truth for the formula; INFORMATION_SCHEMA doesn't expose
// it.
pub(super) const COLUMNS_SQL: &str = "SELECT c.name AS column_name,
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
pub(super) const PK_SQL: &str = "SELECT i.name AS pk_name,
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
pub(super) const IDX_SQL: &str = "SELECT i.name AS index_name,
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
pub(super) const FK_SQL: &str = "SELECT fk.name AS fk_name,
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

pub(super) fn str_col(row: &Row, idx: usize, label: &str) -> Result<String, AppError> {
    match row.try_get::<&str, _>(idx).map_err(AppError::from)? {
        Some(s) => Ok(s.to_string()),
        None => Err(AppError::internal(format!(
            "table-designer row column {label} (idx {idx}) was NULL"
        ))),
    }
}

pub(super) fn split_csv(s: &str) -> Vec<String> {
    s.split(',')
        .filter(|p| !p.is_empty())
        .map(|p| p.trim().to_string())
        .collect()
}

// sys.foreign_keys returns NO_ACTION / CASCADE / SET_NULL / SET_DEFAULT with
// underscores; DDL uses spaces. NO_ACTION is the default so we drop it entirely.
pub(super) fn normalize_ref_action(raw: &str) -> String {
    match raw.to_ascii_uppercase().as_str() {
        "NO_ACTION" => "NO ACTION".into(),
        "CASCADE" => "CASCADE".into(),
        "SET_NULL" => "SET NULL".into(),
        "SET_DEFAULT" => "SET DEFAULT".into(),
        other => other.replace('_', " "),
    }
}

pub(super) fn format_sql_type(data_type: &str, max_length: i32, precision: i32, scale: i32) -> String {
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
