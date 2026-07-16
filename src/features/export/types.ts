export type ExportFormat = 'csv' | 'json' | 'xlsx';

export interface ExportFormatOption {
  id: ExportFormat;
  label: string;
  extension: string;
  description: string;
}

export interface ExportRequest {
  format: ExportFormat;
  path: string;
  columns: Array<{ name: string }>;
  rows: unknown[][];
}

export interface ExportResult {
  rowsWritten: number;
  bytesWritten: number;
  path: string;
}
