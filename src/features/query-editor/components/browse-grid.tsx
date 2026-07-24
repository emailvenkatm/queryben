import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { invoke } from '@/shared/api/tauri';
import { usePendingChangesStore } from '@/shared/stores/pending-changes';
import type { ColumnType, QueryResult, ResultColumn, TableMetadata } from '@/shared/types';
import { useBrowseMutations, type InsertRow, NULL_SENTINEL } from '../hooks/use-browse-mutations';
import { BrowseToolbar } from './browse-toolbar';
import { CellEditor, InsertCellInput } from './cell-editor';

const TYPE_STYLES: Record<ColumnType, { bg: string; color: string; label: string }> = {
  number:   { bg: 'rgba(46,125,50,0.10)',   color: 'var(--color-success)', label: 'int' },
  string:   { bg: 'rgba(21,101,192,0.10)',  color: '#1565c0', label: 'nvc' },
  datetime: { bg: 'rgba(136,14,79,0.10)',   color: '#880e4f', label: 'dt' },
  boolean:  { bg: 'rgba(106,27,154,0.10)',  color: '#6a1b9a', label: 'bit' },
  null:     { bg: 'rgba(38,50,56,0.08)',    color: 'var(--color-text-muted)', label: '—' },
  unknown:  { bg: 'rgba(38,50,56,0.08)',    color: 'var(--color-text-muted)', label: '?' },
};

function sqlTypeToColumnType(sqlType: string): ColumnType {
  const t = sqlType.toLowerCase();
  if (t.startsWith('int') || t === 'bigint' || t === 'smallint' || t === 'tinyint' ||
      t.startsWith('decimal') || t.startsWith('numeric') || t === 'float' || t === 'real' ||
      t.startsWith('money')) return 'number';
  if (t === 'bit') return 'boolean';
  if (t.includes('date') || t.includes('time')) return 'datetime';
  return 'string';
}

function BrowseCellDisplay({ value }: { value: unknown }) {
  if (value === null || value === undefined) {
    return (
      <span style={{ display: 'inline-flex', fontFamily: 'Geist Mono, monospace', fontSize: 10, fontWeight: 600, color: 'rgba(26,46,42,0.35)', background: 'rgba(26,46,42,0.06)', border: '1px solid rgba(26,46,42,0.12)', borderRadius: 3, padding: '1px 5px', letterSpacing: '0.04em', fontStyle: 'italic' }}>
        NULL
      </span>
    );
  }
  return <span className="select-text">{String(value)}</span>;
}

interface RowContextMenuProps {
  x: number;
  y: number;
  canDelete: boolean;
  onDelete: () => void;
}

function RowContextMenu({ x, y, canDelete, onDelete }: RowContextMenuProps) {
  return (
    <div role="menu" style={{ position: 'fixed', left: x, top: y, zIndex: 50, minWidth: 160, borderRadius: 6, border: '1px solid rgba(26,46,42,0.14)', background: '#fff', boxShadow: '0 4px 16px rgba(26,46,42,0.14)', padding: '4px 0' }}>
      <button
        type="button"
        role="menuitem"
        onClick={onDelete}
        disabled={!canDelete}
        style={{ display: 'flex', width: '100%', alignItems: 'center', gap: 8, padding: '6px 12px', fontSize: 12, color: canDelete ? 'var(--color-error)' : 'var(--color-text-muted)', background: 'transparent', border: 'none', cursor: canDelete ? 'pointer' : 'not-allowed', fontFamily: 'Geist, sans-serif', textAlign: 'left' }}
      >
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true">
          <path d="M2 3h8M5 3V2h2v1M4 3l.5 7M8 3l-.5 7" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
        </svg>
        Delete row
      </button>
    </div>
  );
}

export interface BrowseGridProps {
  result: QueryResult | null;
  connectionId: string;
  tabId: string;
  browseTable: { schema: string; name: string };
  isLoading?: boolean;
  onRefresh?: () => void;
}

export function BrowseGrid({
  result,
  connectionId,
  tabId,
  browseTable,
  isLoading = false,
  onRefresh,
}: BrowseGridProps) {
  const [metadata, setMetadata] = useState<TableMetadata | null>(null);
  const [metadataError, setMetadataError] = useState<string | null>(null);
  const [metadataReloadKey, setMetadataReloadKey] = useState(0);
  const [editing, setEditing] = useState<{ rowId: string; columnName: string } | null>(null);
  const [insertRows, setInsertRows] = useState<InsertRow[]>([]);
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null);
  const contextMenuRowIdx = useRef<number>(-1);

  const allChanges = usePendingChangesStore((s) => s.changes);
  const changes = useMemo(() => allChanges.filter((c) => c.tabId === tabId), [allChanges, tabId]);

  useEffect(() => {
    let cancelled = false;
    setMetadataError(null);
    invoke<TableMetadata>('get_table_metadata', { connectionId, schema: browseTable.schema, name: browseTable.name })
      .then((m) => { if (!cancelled) setMetadata(m); })
      .catch((err: unknown) => {
        if (cancelled) return;
        const msg = err instanceof Error
          ? err.message
          : typeof err === 'object' && err !== null && 'message' in err
            ? String((err as { message: unknown }).message)
            : String(err);
        setMetadataError(msg);
      });
    return () => { cancelled = true; };
  }, [connectionId, browseTable.schema, browseTable.name, metadataReloadKey]);

  const reloadMetadata = useCallback(() => {
    setMetadata(null);
    setMetadataError(null);
    setMetadataReloadKey((k) => k + 1);
  }, []);

  // Clear insert rows when all pending changes are cleared externally.
  const prevChangeCount = useRef(changes.length);
  useEffect(() => {
    if (prevChangeCount.current > 0 && changes.length === 0) setInsertRows([]);
    prevChangeCount.current = changes.length;
  }, [changes.length]);

  const cellUpdates = useMemo(() => {
    const map = new Map<string, unknown>();
    for (const c of changes) {
      if (c.kind === 'update' && c.columnName) map.set(`${c.rowId}::${c.columnName}`, c.newValue);
    }
    return map;
  }, [changes]);

  const deletedRows = useMemo(() => {
    const set = new Set<string>();
    for (const c of changes) if (c.kind === 'delete') set.add(c.rowId);
    return set;
  }, [changes]);

  const displayColumns: ResultColumn[] = useMemo(() => {
    if (result) return result.columns;
    if (!metadata) return [];
    return metadata.columns.map((c) => ({ name: c.name, columnType: sqlTypeToColumnType(c.sqlType), nullable: c.isNullable }));
  }, [result, metadata]);

  const displayRows = result?.rows ?? [];
  const displayRowCount = result?.rowCount ?? 0;

  const { isEditable, handleCommitEdit, handleInsertCellCommit, handleDeleteRow, rowIdForIdx } =
    useBrowseMutations({ tabId, metadata, rows: displayRows as unknown[][], columns: displayColumns, insertRows });

  const handleAddRow = useCallback(() => {
    if (!metadata || !isEditable) return;
    setInsertRows((rows) => [...rows, { rowId: `new-${crypto.randomUUID()}`, values: {} }]);
  }, [isEditable, metadata]);

  const commitInsert = useCallback(
    (rowId: string, columnName: string, next: typeof NULL_SENTINEL | string) => {
      handleInsertCellCommit(rowId, columnName, next);
      setInsertRows((rows) =>
        rows.map((r) => {
          if (r.rowId !== rowId) return r;
          const newValue = next === NULL_SENTINEL ? null : next;
          return { ...r, values: { ...r.values, [columnName]: newValue } };
        }),
      );
    },
    [handleInsertCellCommit],
  );

  const handleRowContext = useCallback((evt: React.MouseEvent, rowIdx: number) => {
    evt.preventDefault();
    contextMenuRowIdx.current = rowIdx;
    setContextMenu({ x: evt.clientX, y: evt.clientY });
  }, []);

  useEffect(() => {
    if (!contextMenu) return;
    const handler = () => setContextMenu(null);
    document.addEventListener('click', handler);
    return () => document.removeEventListener('click', handler);
  }, [contextMenu]);

  return (
    <div className="flex flex-col h-full" style={{ background: 'var(--color-bg)', position: 'relative' }}>
      <BrowseToolbar
        schema={browseTable.schema}
        name={browseTable.name}
        rowCount={displayRowCount}
        pendingCount={changes.length}
        canEdit={isEditable}
        onAddRow={handleAddRow}
        metadataError={metadataError}
        isLoading={isLoading}
        onRefresh={() => { reloadMetadata(); onRefresh?.(); }}
      />

      <div className="flex-1 overflow-auto">
        {displayColumns.length === 0 ? (
          <div style={{ padding: 40, textAlign: 'center', fontSize: 12, color: metadataError ? 'var(--color-error)' : 'var(--color-text-muted)', fontFamily: 'Geist, sans-serif', display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 12 }}>
            <div>{metadataError ?? 'Loading table…'}</div>
            {metadataError && (
              <button
                type="button"
                onClick={() => { reloadMetadata(); onRefresh?.(); }}
                style={{ padding: '6px 14px', fontSize: 12, fontFamily: 'Geist, sans-serif', color: 'var(--color-text)', background: 'var(--color-bg-elevated)', border: '1px solid rgba(26,46,42,0.14)', borderRadius: 6, cursor: 'pointer' }}
              >
                Retry
              </button>
            )}
          </div>
        ) : (
          <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 12, minWidth: 600 }} aria-label="Table browse">
            <thead>
              <tr>
                <th style={{ width: 44, background: 'rgba(26,46,42,0.04)', borderBottom: '1px solid rgba(26,46,42,0.10)', borderRight: '1px solid rgba(26,46,42,0.06)', position: 'sticky', top: 0, zIndex: 11, padding: '7px 10px', textAlign: 'right' }} scope="col">
                  <span style={{ fontSize: 10, color: 'var(--color-text-muted)' }}>#</span>
                </th>
                {displayColumns.map((col) => {
                  const s = TYPE_STYLES[col.columnType] ?? TYPE_STYLES.unknown;
                  return (
                    <th key={col.name} scope="col" style={{ background: 'rgba(26,46,42,0.04)', borderBottom: '1px solid rgba(26,46,42,0.10)', borderRight: '1px solid rgba(26,46,42,0.06)', padding: '7px 12px', userSelect: 'none', whiteSpace: 'nowrap', position: 'sticky', top: 0, zIndex: 10 }}>
                      <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
                        <span style={{ fontSize: 11, fontWeight: 600, color: 'var(--color-text-muted)' }}>{col.name}</span>
                        <span style={{ fontSize: 9, fontWeight: 600, padding: '1px 4px', borderRadius: 3, letterSpacing: '0.04em', background: s.bg, color: s.color }} aria-hidden="true">{s.label}</span>
                      </div>
                    </th>
                  );
                })}
              </tr>
            </thead>
            <tbody>
              {displayRows.map((row, rowIdx) => {
                const rowId = rowIdForIdx(rowIdx);
                const isDeleted = deletedRows.has(rowId);
                return (
                  <tr key={rowId} onContextMenu={(e) => handleRowContext(e, rowIdx)} style={{ background: isDeleted ? 'rgba(192,57,43,0.12)' : undefined, textDecoration: isDeleted ? 'line-through' : undefined, color: isDeleted ? 'var(--color-text-muted)' : undefined }}>
                    <td style={{ width: 44, textAlign: 'right', paddingRight: 10, fontSize: 11, color: isDeleted ? 'var(--color-error)' : 'rgba(26,46,42,0.3)', fontFamily: 'Geist Mono, monospace', borderBottom: '1px solid rgba(26,46,42,0.045)', borderRight: '1px solid rgba(26,46,42,0.07)', background: isDeleted ? 'rgba(192,57,43,0.12)' : 'rgba(26,46,42,0.025)' }}>
                      {isDeleted ? '×' : rowIdx + 1}
                    </td>
                    {displayColumns.map((col, cellIdx) => {
                      const rawValue = (row as unknown[])[cellIdx];
                      const pendingValue = cellUpdates.get(`${rowId}::${col.name}`);
                      const displayValue = pendingValue !== undefined ? pendingValue : rawValue;
                      const hasPending = cellUpdates.has(`${rowId}::${col.name}`);
                      const isEditingThisCell = editing?.rowId === rowId && editing?.columnName === col.name;
                      return (
                        <td
                          key={col.name}
                          onDoubleClick={() => { if (!isEditable || isDeleted) return; setEditing({ rowId, columnName: col.name }); }}
                          style={{ padding: 0, borderBottom: '1px solid rgba(26,46,42,0.045)', borderRight: '1px solid rgba(26,46,42,0.04)', whiteSpace: 'nowrap', fontFamily: 'Geist Mono, monospace', fontSize: 12, color: 'var(--color-text)', verticalAlign: 'middle', maxWidth: 320, overflow: 'hidden', textOverflow: 'ellipsis', background: hasPending && !isDeleted ? 'rgba(213,138,74,0.18)' : undefined, cursor: isEditable && !isDeleted ? 'text' : 'default' }}
                        >
                          {isEditingThisCell ? (
                            <CellEditor
                              initial={displayValue}
                              sqlType={metadata?.columns.find((c) => c.name === col.name)?.sqlType ?? ''}
                              isNullable={col.nullable}
                              onCommit={(v) => { handleCommitEdit(rowIdx, col.name, v); setEditing(null); }}
                              onCancel={() => setEditing(null)}
                            />
                          ) : (
                            <div style={{ padding: '5px 12px' }}>
                              <BrowseCellDisplay value={displayValue} />
                            </div>
                          )}
                        </td>
                      );
                    })}
                  </tr>
                );
              })}

              {insertRows.map((ins) => (
                <tr key={ins.rowId} style={{ background: 'rgba(42,87,81,0.15)' }}>
                  <td style={{ width: 44, textAlign: 'right', paddingRight: 10, fontSize: 11, color: 'var(--color-primary-hover)', fontFamily: 'Geist Mono, monospace', borderBottom: '1px solid rgba(26,46,42,0.045)', borderRight: '1px solid rgba(26,46,42,0.07)', background: 'rgba(42,87,81,0.20)', fontWeight: 600 }}>+</td>
                  {displayColumns.map((col) => {
                    const metaCol = metadata?.columns.find((c) => c.name === col.name);
                    const isAutoCol = metaCol?.isIdentity === true || metaCol?.isComputed === true;
                    return (
                      <td key={col.name} style={{ padding: 0, borderBottom: '1px solid rgba(26,46,42,0.045)', borderRight: '1px solid rgba(26,46,42,0.04)', whiteSpace: 'nowrap', fontFamily: 'Geist Mono, monospace', fontSize: 12, background: isAutoCol ? 'repeating-linear-gradient(45deg, rgba(26,46,42,0.03), rgba(26,46,42,0.03) 4px, rgba(26,46,42,0.06) 4px, rgba(26,46,42,0.06) 8px)' : '#fff' }}>
                        {isAutoCol ? (
                          <div style={{ padding: '5px 12px', display: 'flex', alignItems: 'center', gap: 6, color: 'rgba(26,46,42,0.4)' }} title="Auto-generated (IDENTITY or computed column)">
                            <svg width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden="true">
                              <rect x="2" y="5" width="6" height="4" rx="0.5" stroke="currentColor" strokeWidth="1" />
                              <path d="M3.5 5V3.5a1.5 1.5 0 013 0V5" stroke="currentColor" strokeWidth="1" />
                            </svg>
                            <span style={{ fontStyle: 'italic', fontSize: 11 }}>auto</span>
                          </div>
                        ) : (
                          <InsertCellInput
                            value={ins.values[col.name]}
                            sqlType={metaCol?.sqlType ?? ''}
                            isNullable={metaCol?.isNullable ?? col.nullable}
                            columnName={col.name}
                            onCommit={(v) => commitInsert(ins.rowId, col.name, v)}
                          />
                        )}
                      </td>
                    );
                  })}
                </tr>
              ))}

              {displayRows.length === 0 && insertRows.length === 0 && (
                <tr>
                  <td colSpan={displayColumns.length + 1} style={{ padding: '32px 16px', textAlign: 'center', fontSize: 12, color: 'var(--color-text-muted)', fontFamily: 'Geist, sans-serif', borderBottom: '1px solid rgba(26,46,42,0.045)' }}>
                    {isLoading ? 'Loading rows…' : isEditable ? 'No rows. Click + New row to add one.' : 'No rows.'}
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        )}
      </div>

      {contextMenu && (
        <RowContextMenu
          x={contextMenu.x}
          y={contextMenu.y}
          canDelete={isEditable}
          onDelete={() => { if (contextMenuRowIdx.current >= 0) handleDeleteRow(contextMenuRowIdx.current); setContextMenu(null); }}
        />
      )}
    </div>
  );
}
