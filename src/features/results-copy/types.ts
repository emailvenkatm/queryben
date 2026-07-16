import type { CellValue, ResultColumn } from '@/shared/types';

export type ColumnMeta = ResultColumn;

export interface CopyTarget {
  schema: string;
  name: string;
}

export interface CopyConfig {
  insertQuoteIdentifiers: boolean;
  insertBracketIdentifiers: boolean;
  csvDelimiter: string;
  markdownAlignNumbers: 'left' | 'right' | 'center';
  dateFormat: 'iso' | string;
}

export interface RowFormatter {
  id: string;
  label: string;
  format(columns: ColumnMeta[], rows: CellValue[][], target?: CopyTarget, config?: CopyConfig): string;
}

export type FormatterRegistry = Record<string, RowFormatter>;

export type { CellValue };
