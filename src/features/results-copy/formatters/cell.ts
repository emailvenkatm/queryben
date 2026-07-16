import type { RowFormatter } from '../types';

export const cellValueFormatter: RowFormatter = {
  id: 'cell',
  label: 'Copy cell value',
  format(_columns, rows) {
    const row = rows[0];
    if (!row) return '';
    const value = row[0];
    if (value === undefined || value === null) return 'NULL';
    if (typeof value === 'boolean') return value ? 'true' : 'false';
    return String(value);
  },
};
