import { useCallback, useMemo } from 'react';
import { usePendingChangesStore } from '@/shared/stores/pending-changes';
import type { TableMetadata, TableColumn } from '@/shared/types';
import { renderUpdate, renderInsert, renderDelete } from './render-sql';

// Sentinel: distinguishes "user wants NULL" from "user typed empty string".
export const NULL_SENTINEL = Symbol('NULL');
export type EditingValue = string | typeof NULL_SENTINEL;

export interface InsertRow {
  rowId: string;
  values: Record<string, unknown>;
}

function rowIdForIdx(rowIdx: number): string {
  return `r${rowIdx}`;
}

function coerceForColumn(text: string, column: TableColumn | undefined): unknown {
  if (!column) return text;
  const t = text.trim();
  if (t === '' && column.isNullable) return null;
  const sqlType = column.sqlType.toLowerCase();
  if (
    sqlType.startsWith('int') || sqlType === 'bigint' || sqlType === 'smallint' ||
    sqlType === 'tinyint' || sqlType.startsWith('decimal') || sqlType.startsWith('numeric') ||
    sqlType === 'float' || sqlType === 'real' || sqlType.startsWith('money')
  ) {
    const n = Number(t);
    return Number.isFinite(n) ? n : text;
  }
  if (sqlType === 'bit') {
    if (t === '1' || t.toLowerCase() === 'true') return true;
    if (t === '0' || t.toLowerCase() === 'false') return false;
    return text;
  }
  return text;
}

interface UseBrowseMutationsParams {
  tabId: string;
  metadata: TableMetadata | null;
  rows: unknown[][];
  columns: Array<{ name: string; nullable: boolean }>;
  insertRows: InsertRow[];
}

export function useBrowseMutations({
  tabId,
  metadata,
  rows,
  columns,
  insertRows,
}: UseBrowseMutationsParams) {
  const stage = usePendingChangesStore((s) => s.stage);
  const allChanges = usePendingChangesStore((s) => s.changes);
  const changes = useMemo(
    () => allChanges.filter((c) => c.tabId === tabId),
    [allChanges, tabId],
  );

  const isEditable = metadata?.isEditable ?? false;

  const rowValuesForIdx = useCallback(
    (rowIdx: number): Record<string, unknown> => {
      const values: Record<string, unknown> = {};
      const row = rows[rowIdx];
      if (!row) return values;
      columns.forEach((col, idx) => { values[col.name] = row[idx]; });
      return values;
    },
    [rows, columns],
  );

  const buildPkValues = useCallback(
    (rowValues: Record<string, unknown>): Record<string, unknown> | null => {
      if (!metadata) return null;
      const pk: Record<string, unknown> = {};
      for (const col of metadata.primaryKey) {
        if (!(col in rowValues)) return null;
        pk[col] = rowValues[col];
      }
      return pk;
    },
    [metadata],
  );

  const handleCommitEdit = useCallback(
    (rowIdx: number, columnName: string, next: EditingValue): void => {
      if (!metadata || !isEditable) return;
      // Server-maintained columns (IDENTITY, computed, rowversion) can't accept
      // a UPDATE. The browse grid already blocks the double-click, but a stray
      // keyboard path or a future refactor shouldn't be able to slip one past.
      const column = metadata.columns.find((c) => c.name === columnName);
      if (!column || !column.isEditable) return;
      const rowId = rowIdForIdx(rowIdx);
      const rowValues = rowValuesForIdx(rowIdx);
      const oldValue = rowValues[columnName];
      const newValue =
        next === NULL_SENTINEL
          ? null
          : coerceForColumn(String(next), column);
      if (oldValue === newValue) return;
      const pkValues = buildPkValues(rowValues);
      if (!pkValues) return;
      const sql = renderUpdate({ metadata, columnName, newValue, oldValue, primaryKeyValues: pkValues });
      stage({ id: crypto.randomUUID(), tabId, kind: 'update', rowId, columnName, oldValue, newValue, primaryKeyValues: pkValues, sql });
    },
    [buildPkValues, isEditable, metadata, rowValuesForIdx, stage, tabId],
  );

  const handleInsertCellCommit = useCallback(
    (rowId: string, columnName: string, next: EditingValue): void => {
      if (!metadata) return;
      const col = metadata.columns.find((c) => c.name === columnName);
      const newValue = next === NULL_SENTINEL ? null : coerceForColumn(String(next), col);
      const currentRow = insertRows.find((r) => r.rowId === rowId);
      const nextValues = { ...(currentRow?.values ?? {}), [columnName]: newValue };
      const existing = changes.find((c) => c.kind === 'insert' && c.rowId === rowId);
      const sql = renderInsert({ metadata, rowValues: nextValues });
      if (existing) {
        usePendingChangesStore.getState().unstage(existing.id);
      }
      stage({ id: crypto.randomUUID(), tabId, kind: 'insert', rowId, rowValues: nextValues, sql });
    },
    [changes, insertRows, metadata, stage, tabId],
  );

  const handleDeleteRow = useCallback(
    (rowIdx: number): void => {
      if (!metadata || !isEditable) return;
      const rowId = rowIdForIdx(rowIdx);
      const rowValues = rowValuesForIdx(rowIdx);
      const pkValues = buildPkValues(rowValues);
      if (!pkValues) return;
      const sql = renderDelete({ metadata, primaryKeyValues: pkValues });
      stage({ id: crypto.randomUUID(), tabId, kind: 'delete', rowId, primaryKeyValues: pkValues, sql });
    },
    [buildPkValues, isEditable, metadata, rowValuesForIdx, stage, tabId],
  );

  return { isEditable, handleCommitEdit, handleInsertCellCommit, handleDeleteRow, rowIdForIdx };
}
