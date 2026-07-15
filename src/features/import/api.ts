import { useMutation } from '@tanstack/react-query';
import { commands } from '@/shared/api/tauri-bindings';
import type { ColumnMapping, ImportFormat, ImportOptions, ImportPreview, ImportResult } from './types';

interface PreviewInput {
  path: string;
  format: ImportFormat;
}

interface ExecuteInput {
  connectionId: string;
  path: string;
  format: ImportFormat;
  targetSchema: string;
  targetTable: string;
  columnMapping: ColumnMapping[];
  options: ImportOptions;
}

export function useImportPreview() {
  return useMutation<ImportPreview, unknown, PreviewInput>({
    mutationFn: ({ path, format }) => commands.importPreview(path, format),
  });
}

export function useImportExecute() {
  return useMutation<ImportResult, unknown, ExecuteInput>({
    mutationFn: ({ connectionId, path, format, targetSchema, targetTable, columnMapping, options }) =>
      commands.importExecute(connectionId, path, format, targetSchema, targetTable, columnMapping, options),
  });
}
