import type {
  ColumnMapping,
  ImportFormat,
  ImportOptions,
  ImportPreview,
  ImportResult,
} from '@/shared/api/tauri-bindings';

export type { ColumnMapping, ImportFormat, ImportOptions, ImportPreview, ImportResult };

export type ImportSource =
  | { kind: 'file'; path: string; format: ImportFormat }
  | { kind: 'ads' };

export type WizardStep = 'source' | 'preview' | 'mapping' | 'execute';

export const SQL_TYPE_CHOICES = [
  'INT',
  'BIGINT',
  'FLOAT',
  'BIT',
  'DATETIME2',
  'DATE',
  'NVARCHAR(50)',
  'NVARCHAR(255)',
  'NVARCHAR(MAX)',
  'DECIMAL(18,4)',
  'UNIQUEIDENTIFIER',
] as const;

export interface ColumnMap {
  sourceColumn: string;
  targetColumn: string;
  targetType: string;
  include: boolean;
}
