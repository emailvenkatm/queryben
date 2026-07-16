import type { CellValue, RowFormatter } from '../types';
import { defaultCopyConfig } from '../api';

function alignmentToken(align: 'left' | 'right' | 'center'): string {
  if (align === 'right') return '---:';
  if (align === 'center') return ':---:';
  return ':---';
}

function escapeMdCell(value: CellValue): string {
  if (value === null) return '';
  return String(value).replace(/\|/g, '\\|').replace(/\r?\n/g, '<br/>');
}

export const markdownFormatter: RowFormatter = {
  id: 'markdown',
  label: 'Copy as Markdown table',
  format(columns, rows, _target, config = defaultCopyConfig) {
    if (columns.length === 0) return '';
    const header = `| ${columns.map((c) => escapeMdCell(c.name)).join(' | ')} |`;
    const separator = `| ${columns.map((c) => c.columnType === 'number' ? alignmentToken(config.markdownAlignNumbers) : ':---').join(' | ')} |`;
    const body = rows.map((row) => `| ${columns.map((_, idx) => escapeMdCell(row[idx] ?? null)).join(' | ')} |`).join('\n');
    return rows.length === 0 ? `${header}\n${separator}` : `${header}\n${separator}\n${body}`;
  },
};
