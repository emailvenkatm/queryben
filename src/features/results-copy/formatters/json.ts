import type { CellValue, ColumnMeta, RowFormatter } from '../types';

function normalise(value: CellValue): unknown {
  return value === null ? null : value;
}

export const jsonFormatter: RowFormatter = {
  id: 'json',
  label: 'Copy as JSON',
  format(columns: ColumnMeta[], rows) {
    const objects = rows.map((row) => {
      const obj: Record<string, unknown> = {};
      columns.forEach((col, idx) => { obj[col.name] = normalise(row[idx] ?? null); });
      return obj;
    });
    return JSON.stringify(objects, null, 2);
  },
};
