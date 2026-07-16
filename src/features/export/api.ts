import type { ExportFormat, ExportFormatOption } from './types';

export const FORMAT_OPTIONS: readonly ExportFormatOption[] = [
  { id: 'csv', label: 'CSV', extension: 'csv', description: 'Comma-separated. Opens in Excel, Numbers, Google Sheets, everything.' },
  { id: 'json', label: 'JSON', extension: 'json', description: 'Array of objects keyed by column name. Nulls preserved.' },
  { id: 'xlsx', label: 'Excel (XLSX)', extension: 'xlsx', description: 'Native Excel workbook. Header row, typed cells, one sheet.' },
] as const;

export function optionFor(format: ExportFormat): ExportFormatOption {
  return FORMAT_OPTIONS.find((o) => o.id === format) ?? FORMAT_OPTIONS[0]!;
}
