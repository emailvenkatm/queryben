import { useCallback, useState } from 'react';
import { flexRender } from '@tanstack/react-table';
import type { CellValue, QueryOutcome, QueryResult } from '@/shared/types';
import { ExportDialog } from '@/features/export';
import { targetFromSql } from '@/features/results-copy';
import { useResultsTable } from '../hooks/use-results-table';
import { ResultsToolbar } from './results-toolbar';
import { ResultsContextMenu } from './results-context-menu';
import { TypeBadge, CellDisplay } from './cell-renderer';

interface ResultsGridProps {
  outcome: QueryOutcome;
  browseTable?: { schema: string; name: string };
  sql?: string;
}

function ResultSetHeader({ index, rowCount, durationMs, variant = 'ok' }: { index: number; rowCount: number; durationMs: number; variant?: 'ok' | 'error' }) {
  const isError = variant === 'error';
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '6px 16px', background: isError ? 'rgba(220,38,38,0.04)' : 'rgba(42,87,81,0.04)', borderTop: '1px solid rgba(26,46,42,0.08)', borderBottom: '1px solid rgba(26,46,42,0.08)', fontSize: 12, fontFamily: 'Geist, sans-serif', color: isError ? 'var(--color-error)' : 'var(--color-primary)', fontWeight: 500 }} role="heading" aria-level={3}>
      <span style={{ fontWeight: 600 }}>Result {index}</span>
      {!isError && (
        <>
          <span style={{ color: 'rgba(26,46,42,0.3)' }}>·</span>
          <span style={{ fontFamily: 'Geist Mono, monospace', fontSize: 11, color: 'var(--color-text-muted)' }}>{rowCount.toLocaleString()} {rowCount === 1 ? 'row' : 'rows'}</span>
          <span style={{ color: 'rgba(26,46,42,0.3)' }}>·</span>
          <span style={{ fontFamily: 'Geist Mono, monospace', fontSize: 11, color: 'var(--color-text-muted)' }}>{durationMs} ms</span>
        </>
      )}
      {isError && <span style={{ fontFamily: 'Geist Mono, monospace', fontSize: 11, color: 'var(--color-error)' }}>failed</span>}
    </div>
  );
}

function ResultSetError({ message }: { message: string }) {
  return (
    <div role="alert" style={{ padding: '10px 16px', background: 'rgba(220,38,38,0.06)', borderBottom: '1px solid rgba(192,57,43,0.25)', color: 'var(--color-error)', fontSize: 12, fontFamily: 'Geist Mono, monospace', whiteSpace: 'pre-wrap', wordBreak: 'break-word' }}>
      <strong style={{ fontFamily: 'Geist, sans-serif', fontWeight: 600, marginRight: 6 }}>Query failed:</strong>
      {message}
    </div>
  );
}

export function SingleResultGrid({ result, target, onExportRow }: { result: QueryResult; target?: { schema: string; name: string }; onExportRow?: (rowIdx: number) => void }) {
  const { table, selectedRows, toggleRow } = useResultsTable(result);
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number; rowIdx: number } | null>(null);
  const [contextCellIdx, setContextCellIdx] = useState<number | undefined>(undefined);

  const handleRowClick = useCallback((rowIdx: number, evt: React.MouseEvent) => {
    toggleRow(rowIdx, evt.metaKey || evt.ctrlKey);
  }, [toggleRow]);

  const handleContextMenu = useCallback((evt: React.MouseEvent, rowIdx: number, cellIdx?: number) => {
    evt.preventDefault();
    setContextCellIdx(cellIdx);
    setContextMenu({ x: evt.clientX, y: evt.clientY, rowIdx });
  }, []);

  return (
    <div style={{ position: 'relative' }}>
      <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 12, minWidth: 600 }} aria-label="Query results">
        <thead>
          {table.getHeaderGroups().map((hg) => (
            <tr key={hg.id}>
              <th style={{ width: 44, background: 'var(--color-border)', borderBottom: '1px solid rgba(26,46,42,0.12)', borderRight: '1px solid rgba(26,46,42,0.06)', position: 'sticky', top: 0, zIndex: 11, padding: '0 10px', textAlign: 'right' }} scope="col" aria-label="Row number">
                <span style={{ fontSize: 10, color: 'var(--color-text-muted)' }}>#</span>
              </th>
              {hg.headers.map((header) => {
                const col = result.columns.find((c) => c.name === header.id);
                return (
                  <th key={header.id} scope="col" style={{ background: 'var(--color-border)', borderBottom: '1px solid rgba(26,46,42,0.12)', borderRight: '1px solid rgba(26,46,42,0.06)', padding: 0, userSelect: 'none', whiteSpace: 'nowrap', position: 'sticky', top: 0, zIndex: 10 }} onClick={header.column.getToggleSortingHandler()} aria-sort={header.column.getIsSorted() === 'asc' ? 'ascending' : header.column.getIsSorted() === 'desc' ? 'descending' : 'none'}>
                    <div style={{ display: 'flex', alignItems: 'center', height: 34, padding: '0 10px', cursor: header.column.getCanSort() ? 'pointer' : 'default', gap: 0 }}>
                      <span style={{ fontSize: 11, fontWeight: 600, color: 'var(--color-text-muted)', letterSpacing: '0.01em' }}>{flexRender(header.column.columnDef.header, header.getContext())}</span>
                      {col && <TypeBadge type={col.columnType} />}
                      {header.column.getIsSorted() === 'asc' && <svg style={{ marginLeft: 4, color: 'var(--color-accent)' }} width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden="true"><path d="M2 7l3-4 3 4" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" /></svg>}
                      {header.column.getIsSorted() === 'desc' && <svg style={{ marginLeft: 4, color: 'var(--color-accent)' }} width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden="true"><path d="M2 3l3 4 3-4" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" /></svg>}
                    </div>
                  </th>
                );
              })}
            </tr>
          ))}
        </thead>
        <tbody>
          {table.getRowModel().rows.map((row, rowIdx) => {
            const isSelected = selectedRows.has(rowIdx);
            return (
              <tr key={row.id} onClick={(evt) => handleRowClick(rowIdx, evt)} onContextMenu={(evt) => handleContextMenu(evt, rowIdx)} style={{ cursor: 'default', background: isSelected ? 'rgba(213,138,74,0.09)' : 'transparent' }} onMouseEnter={(e) => { if (!isSelected) (e.currentTarget as HTMLElement).style.background = 'rgba(26,46,42,0.025)'; }} onMouseLeave={(e) => { (e.currentTarget as HTMLElement).style.background = isSelected ? 'rgba(213,138,74,0.09)' : 'transparent'; }}>
                <td style={{ width: 44, textAlign: 'right', paddingRight: 10, fontSize: 11, color: 'var(--color-text-muted)', fontFamily: 'Geist Mono, monospace', borderBottom: '1px solid rgba(26,46,42,0.045)', borderRight: '1px solid rgba(26,46,42,0.04)', background: isSelected ? 'rgba(213,138,74,0.14)' : 'var(--color-bg-elevated)', position: 'sticky', left: 0 }}>
                  {rowIdx + 1}
                </td>
                {row.getVisibleCells().map((cell, cellIdx) => {
                  const col = result.columns[cellIdx];
                  return (
                    <td key={cell.id} onContextMenu={(evt) => handleContextMenu(evt, rowIdx, cellIdx)} style={{ padding: '5px 10px', borderBottom: '1px solid rgba(26,46,42,0.045)', borderRight: '1px solid rgba(26,46,42,0.04)', whiteSpace: 'nowrap', fontFamily: 'Geist Mono, monospace', fontSize: 12, color: 'var(--color-text)', verticalAlign: 'middle', maxWidth: 320, overflow: 'hidden', textOverflow: 'ellipsis', background: isSelected ? 'rgba(213,138,74,0.09)' : undefined }}>
                      <CellDisplay value={cell.getValue<CellValue>()} type={col?.columnType} />
                    </td>
                  );
                })}
              </tr>
            );
          })}
        </tbody>
      </table>
      {contextMenu && (
        <ResultsContextMenu
          x={contextMenu.x}
          y={contextMenu.y}
          rowIdx={contextMenu.rowIdx}
          cellIdx={contextCellIdx}
          result={result}
          selectedRows={selectedRows}
          target={target}
          onExportRow={onExportRow}
          onClose={() => { setContextMenu(null); setContextCellIdx(undefined); }}
        />
      )}
    </div>
  );
}

export function ResultsGrid({ outcome, browseTable, sql }: ResultsGridProps) {
  const { resultSets, error, totalDurationMs } = outcome;
  const isMulti = resultSets.length > 1 || (resultSets.length >= 1 && Boolean(error));
  const target = browseTable ?? (sql ? targetFromSql(sql) : undefined);
  const [exportOpen, setExportOpen] = useState(false);
  const [exportRow, setExportRow] = useState<number | null>(null);
  const first = resultSets[0];
  const canExport = Boolean(first && first.rows.length > 0);
  const exportColumns = first?.columns ?? [];
  const exportRows = exportRow !== null && first?.rows[exportRow] ? [first.rows[exportRow]!] : first?.rows ?? [];

  const closeExport = () => { setExportOpen(false); setExportRow(null); };
  const openRowExport = (rowIdx: number) => { setExportRow(rowIdx); setExportOpen(true); };

  if (resultSets.length === 0) {
    return (
      <div className="relative flex flex-col h-full" style={{ background: 'var(--color-bg)' }}>
        <ResultsToolbar rowCount={0} execMs={totalDurationMs} />
        {error && <ResultSetError message={error} />}
        {!error && <div style={{ display: 'flex', height: '100%', alignItems: 'center', justifyContent: 'center' }}><span style={{ fontSize: 13, color: 'var(--color-text-muted)' }}>No result sets.</span></div>}
      </div>
    );
  }

  if (!isMulti && first) {
    return (
      <div className="relative flex flex-col h-full" style={{ background: 'var(--color-bg)' }}>
        <ResultsToolbar rowCount={first.rowCount} execMs={totalDurationMs} onExport={() => setExportOpen(true)} canExport={canExport} />
        <div className="flex-1 overflow-auto">
          <SingleResultGrid result={first} target={target} onExportRow={openRowExport} />
        </div>
        <ExportDialog open={exportOpen} onClose={closeExport} columns={exportColumns} rows={exportRows} defaultFilename="results" />
      </div>
    );
  }

  const totalRows = resultSets.reduce((n, rs) => n + rs.rowCount, 0);
  return (
    <div className="relative flex flex-col h-full" style={{ background: 'var(--color-bg)' }}>
      <ResultsToolbar rowCount={totalRows} execMs={totalDurationMs} onExport={() => setExportOpen(true)} canExport={canExport} />
      <div className="flex-1 overflow-auto">
        {resultSets.map((rs, idx) => (
          <div key={idx}>
            <ResultSetHeader index={idx + 1} rowCount={rs.rowCount} durationMs={rs.durationMs} />
            <SingleResultGrid result={rs} target={target} onExportRow={openRowExport} />
          </div>
        ))}
        {error && (
          <>
            <ResultSetHeader index={resultSets.length + 1} rowCount={0} durationMs={0} variant="error" />
            <ResultSetError message={error} />
          </>
        )}
      </div>
      <ExportDialog open={exportOpen} onClose={closeExport} columns={exportColumns} rows={exportRows} defaultFilename="results" />
    </div>
  );
}
