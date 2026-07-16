import type { TableMetadata } from '@/shared/types';
import { qualifiedName, sqlLiteral } from './sql-literal';

interface UpdateArgs {
  metadata: TableMetadata;
  columnName: string;
  newValue: unknown;
  oldValue: unknown;
  primaryKeyValues: Record<string, unknown>;
}

export function renderUpdate({ metadata, columnName, newValue, oldValue, primaryKeyValues }: UpdateArgs): string {
  const target = qualifiedName(metadata.schema, metadata.name);
  const where = renderWhere(metadata.primaryKey, primaryKeyValues);
  const wasHint = oldValue === newValue ? '' : `  -- was ${sqlLiteral(oldValue)}`;
  return `UPDATE ${target}\nSET    [${columnName}] = ${sqlLiteral(newValue)}${wasHint}\nWHERE  ${where};`;
}

interface InsertArgs {
  metadata: TableMetadata;
  rowValues: Record<string, unknown>;
}

export function renderInsert({ metadata, rowValues }: InsertArgs): string {
  const target = qualifiedName(metadata.schema, metadata.name);
  const cols = metadata.columns.filter((c) => {
    if (c.isIdentity || c.isComputed) return false;
    if (!Object.prototype.hasOwnProperty.call(rowValues, c.name) && c.isNullable) return false;
    return true;
  });
  if (cols.length === 0) return `INSERT INTO ${target} DEFAULT VALUES;`;
  const colList = cols.map((c) => `[${c.name}]`).join(', ');
  const valList = cols.map((c) => sqlLiteral(rowValues[c.name])).join(', ');
  return `INSERT INTO ${target}\n    (${colList})\nVALUES\n    (${valList});`;
}

interface DeleteArgs {
  metadata: TableMetadata;
  primaryKeyValues: Record<string, unknown>;
}

export function renderDelete({ metadata, primaryKeyValues }: DeleteArgs): string {
  const target = qualifiedName(metadata.schema, metadata.name);
  return `DELETE FROM ${target}\nWHERE       ${renderWhere(metadata.primaryKey, primaryKeyValues)};`;
}

function renderWhere(pkColumns: string[], pkValues: Record<string, unknown>): string {
  if (pkColumns.length === 0) return '1 = 0 /* no primary key */';
  return pkColumns.map((col) => `[${col}] = ${sqlLiteral(pkValues[col])}`).join(' AND ');
}
