import { useMutation } from '@tanstack/react-query';
import { invoke } from '@tauri-apps/api/core';
import type { ExportRequest, ExportResult } from '../types';

async function exportResultSet(req: ExportRequest): Promise<ExportResult> {
  return invoke<ExportResult>('export_result_set', {
    format: req.format,
    path: req.path,
    columns: req.columns,
    rows: req.rows,
  });
}

export function useExport() {
  return useMutation<ExportResult, unknown, ExportRequest>({
    mutationFn: exportResultSet,
  });
}
