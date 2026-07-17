import { useEffect, useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { formatAppErrorForDisplay } from '@/shared/api/errors';
import { useImportExecute, useImportPreview } from '../api';
import type { ColumnMap, ImportFormat, ImportOptions, ImportPreview, ImportResult, WizardStep } from '../types';
import { SQL_TYPE_CHOICES } from '../types';

interface Props {
  isOpen: boolean;
  connectionId: string | null;
  defaultSchema: string;
  defaultTable: string;
  onImported?: (result: ImportResult) => void;
}

export interface UseImportReturn {
  step: WizardStep;
  path: string | null;
  format: ImportFormat;
  preview: ImportPreview | null;
  mapping: ColumnMap[];
  targetSchema: string;
  targetTable: string;
  options: ImportOptions;
  errorMsg: string | null;
  result: ImportResult | null;
  inFlight: boolean;
  canGoNext: boolean;
  canImport: boolean;
  setMapping: (m: ColumnMap[]) => void;
  setTargetSchema: (v: string) => void;
  setTargetTable: (v: string) => void;
  setOptions: (o: ImportOptions) => void;
  goBack: () => void;
  goNext: () => void;
  pickFile: () => void;
  runImport: () => void;
}

function inferSqlType(t: string): string {
  switch (t.toUpperCase()) {
    case 'INT': return 'INT';
    case 'BIGINT': return 'BIGINT';
    case 'FLOAT': return 'FLOAT';
    case 'BOOL': return 'BIT';
    case 'DATE_TIME': return 'DATETIME2';
    default: return 'NVARCHAR(255)';
  }
}

const ORDER: WizardStep[] = ['source', 'preview', 'mapping', 'execute'];
const prev = (s: WizardStep): WizardStep => ORDER[Math.max(0, ORDER.indexOf(s) - 1)] ?? 'source';
const next = (s: WizardStep): WizardStep => ORDER[Math.min(ORDER.length - 1, ORDER.indexOf(s) + 1)] ?? 'execute';

export function useImport({ isOpen, connectionId, defaultSchema, defaultTable, onImported }: Props): UseImportReturn {
  const [step, setStep] = useState<WizardStep>('source');
  const [path, setPath] = useState<string | null>(null);
  const [format, setFormat] = useState<ImportFormat>('csv');
  const [preview, setPreview] = useState<ImportPreview | null>(null);
  const [mapping, setMapping] = useState<ColumnMap[]>([]);
  const [targetSchema, setTargetSchema] = useState(defaultSchema);
  const [targetTable, setTargetTable] = useState(defaultTable);
  const [options, setOptions] = useState<ImportOptions>({ skipHeaderRow: true, batchSize: 500 });
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const [result, setResult] = useState<ImportResult | null>(null);

  const previewMut = useImportPreview();
  const executeMut = useImportExecute();

  useEffect(() => {
    if (!isOpen) return;
    setStep('source');
    setPath(null);
    setPreview(null);
    setMapping([]);
    setTargetSchema(defaultSchema);
    setTargetTable(defaultTable);
    setErrorMsg(null);
    setResult(null);
  }, [isOpen, defaultSchema, defaultTable]);

  const pickFile = () => {
    setErrorMsg(null);
    void (async () => {
      try {
        const chosen = await open({
          title: 'Import data',
          multiple: false,
          filters: [
            { name: 'CSV', extensions: ['csv'] },
            { name: 'JSON', extensions: ['json'] },
          ],
        });
        if (!chosen) return;
        const chosenPath = Array.isArray(chosen) ? chosen[0]! : chosen;
        const inferred: ImportFormat = chosenPath.toLowerCase().endsWith('.json') ? 'json' : 'csv';
        setPath(chosenPath);
        setFormat(inferred);
        const p = await previewMut.mutateAsync({ path: chosenPath, format: inferred });
        setPreview(p);
        setMapping(
          p.headers.map((h, i) => ({
            sourceColumn: h,
            targetColumn: h,
            targetType: inferSqlType(p.inferredTypes[i] ?? ''),
            include: true,
          })),
        );
        if (!defaultTable) {
          const stem = chosenPath.split(/[\\/]/).pop()!.replace(/\.[^.]+$/, '').replace(/[^A-Za-z0-9_]/g, '_');
          setTargetTable(stem || 'ImportedTable');
        }
        setStep('preview');
      } catch (err) {
        setErrorMsg(formatAppErrorForDisplay(err));
      }
    })();
  };

  const runImport = () => {
    if (!connectionId || !path) return;
    setErrorMsg(null);
    void (async () => {
      try {
        const activeMapping = mapping
          .filter((m) => m.include && m.targetColumn.trim() !== '')
          .map((m) => ({ sourceColumn: m.sourceColumn, targetColumn: m.targetColumn, skip: false }));
        const res = await executeMut.mutateAsync({
          connectionId, path, format,
          targetSchema: targetSchema.trim(),
          targetTable: targetTable.trim(),
          columnMapping: activeMapping,
          options,
        });
        setResult(res);
        onImported?.(res);
      } catch (err) {
        setErrorMsg(formatAppErrorForDisplay(err));
      }
    })();
  };

  const inFlight = previewMut.isPending || executeMut.isPending;

  const canGoNext =
    step === 'preview' ? preview !== null && mapping.length > 0
    : step === 'mapping' ? mapping.some((m) => m.include && m.targetColumn.trim() !== '')
    : true;

  const canImport =
    targetSchema.trim() !== '' &&
    targetTable.trim() !== '' &&
    connectionId !== null &&
    path !== null &&
    mapping.some((m) => m.include && m.targetColumn.trim() !== '');

  return {
    step, path, format, preview, mapping, targetSchema, targetTable, options,
    errorMsg, result, inFlight, canGoNext, canImport,
    setMapping, setTargetSchema, setTargetTable, setOptions,
    goBack: () => setStep(prev(step)),
    goNext: () => setStep(next(step)),
    pickFile, runImport,
  };
}

export { SQL_TYPE_CHOICES };
