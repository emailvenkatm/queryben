import type { CellValue, ColumnMeta, CopyConfig, CopyTarget, RowFormatter } from '../types';
import { defaultCopyConfig } from '../api';

function quoteIdent(name: string, config: CopyConfig): string {
  if (config.insertBracketIdentifiers) return `[${name.replace(/]/g, ']]')}]`;
  if (config.insertQuoteIdentifiers) return `"${name.replace(/"/g, '""')}"`;
  return name;
}

function formatTarget(target: CopyTarget | undefined, config: CopyConfig): string {
  if (!target) return '<table>';
  return `${quoteIdent(target.schema, config)}.${quoteIdent(target.name, config)}`;
}

function isIsoDate(s: string): boolean {
  return /^\d{4}-\d{2}-\d{2}([ T]\d{2}:\d{2}(:\d{2}(\.\d+)?)?([+-]\d{2}:?\d{2}|Z)?)?$/.test(s);
}

function isBase64Bytes(s: string): boolean {
  return /^[A-Za-z0-9+/]{24,}={0,2}$/.test(s);
}

function base64ToHex(b64: string): string {
  const bin = atob(b64);
  let hex = '0x';
  for (let i = 0; i < bin.length; i++) hex += bin.charCodeAt(i).toString(16).padStart(2, '0').toUpperCase();
  return hex;
}

function formatValue(value: CellValue, col: ColumnMeta | undefined): string {
  if (value === null) return 'NULL';
  if (typeof value === 'boolean') return value ? '1' : '0';
  if (typeof value === 'number') return Number.isFinite(value) ? String(value) : 'NULL';
  const s = String(value);
  const type = col?.columnType;
  if (type === 'datetime' || (type === undefined && isIsoDate(s))) return `'${s.replace(/'/g, "''")}'`;
  if (type === 'unknown' && isBase64Bytes(s)) return base64ToHex(s);
  return `'${s.replace(/'/g, "''")}'`;
}

export const insertFormatter: RowFormatter = {
  id: 'insert',
  label: 'Copy as INSERT',
  format(columns, rows, target, config = defaultCopyConfig) {
    const tgt = formatTarget(target, config);
    const cols = columns.map((c) => quoteIdent(c.name, config)).join(', ');
    return rows
      .map((row) => {
        const vals = columns.map((col, i) => formatValue(row[i] ?? null, col)).join(', ');
        return `INSERT INTO ${tgt} (${cols}) VALUES (${vals});`;
      })
      .join('\n');
  },
};

export function targetFromSql(sql: string): CopyTarget | undefined {
  const stripped = sql.replace(/--[^\n]*/g, '').replace(/\/\*[\s\S]*?\*\//g, '');
  const m = stripped.match(/\bfrom\s+(?:\[([^\]]+)\]|"([^"]+)"|(\w+))\s*\.\s*(?:\[([^\]]+)\]|"([^"]+)"|(\w+))/i);
  if (m) {
    const schema = m[1] ?? m[2] ?? m[3];
    const name = m[4] ?? m[5] ?? m[6];
    if (schema && name) return { schema, name };
  }
  const single = stripped.match(/\bfrom\s+(?:\[([^\]]+)\]|"([^"]+)"|(\w+))/i);
  if (single) {
    const name = single[1] ?? single[2] ?? single[3];
    if (name) return { schema: 'dbo', name };
  }
  return undefined;
}
