import type { CellValue, RowFormatter } from '../types';
import { defaultCopyConfig } from '../api';

function escapeCsvCell(value: CellValue, delimiter: string): string {
  if (value === null) return '';
  if (typeof value === 'boolean') return value ? 'true' : 'false';
  const s = String(value);
  const needsQuoting = s.includes(delimiter) || s.includes('"') || s.includes('\n') || s.includes('\r');
  if (!needsQuoting) return s;
  return `"${s.replace(/"/g, '""')}"`;
}

export const csvFormatter: RowFormatter = {
  id: 'csv',
  label: 'Copy as CSV',
  format(columns, rows, _target, config = defaultCopyConfig) {
    const delimiter = config.csvDelimiter || ',';
    const header = columns.map((c) => escapeCsvCell(c.name, delimiter)).join(delimiter);
    const body = rows.map((row) => columns.map((_, idx) => escapeCsvCell(row[idx] ?? null, delimiter)).join(delimiter)).join('\n');
    return rows.length === 0 ? header : `${header}\n${body}`;
  },
};
