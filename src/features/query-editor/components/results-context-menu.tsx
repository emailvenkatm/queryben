import { useEffect, useRef } from 'react';
import { CopyIcon, DownloadIcon } from 'lucide-react';
import type { QueryResult, CellValue } from '@/shared/types';
import { useResultsCopy } from '@/features/results-copy';

interface ResultsContextMenuProps {
  x: number;
  y: number;
  rowIdx: number;
  cellIdx?: number;
  result: QueryResult;
  selectedRows: Set<number>;
  target?: { schema: string; name: string };
  onExportRow?: (rowIdx: number) => void;
  onClose: () => void;
}

export function ResultsContextMenu({ x, y, rowIdx, cellIdx, result, selectedRows, target, onExportRow, onClose }: ResultsContextMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);
  const { copy } = useResultsCopy();

  useEffect(() => {
    const handler = (evt: MouseEvent): void => {
      if (menuRef.current && !menuRef.current.contains(evt.target as Node)) onClose();
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [onClose]);

  const row = result.rows[rowIdx];
  if (!row) return null;

  const rowsToCopy: CellValue[][] =
    selectedRows.has(rowIdx) && selectedRows.size > 1
      ? Array.from(selectedRows).sort((a, b) => a - b).map((i) => result.rows[i]).filter((r): r is CellValue[] => Boolean(r))
      : [row];

  const cellCol = cellIdx !== undefined ? result.columns[cellIdx] : undefined;
  const cellVal: CellValue = cellIdx !== undefined && row[cellIdx] !== undefined ? row[cellIdx]! : null;

  const actions = [
    { label: 'Copy cell value', disabled: cellIdx === undefined, action: () => { if (cellCol) void copy('cell', [cellCol], [[cellVal]]); }, Icon: CopyIcon },
    { label: 'Copy as INSERT', disabled: false, action: () => void copy('insert', result.columns, rowsToCopy, target), Icon: CopyIcon },
    { label: 'Copy as Markdown', disabled: false, action: () => void copy('markdown', result.columns, rowsToCopy), Icon: CopyIcon },
    { label: 'Copy as JSON', disabled: false, action: () => void copy('json', result.columns, rowsToCopy), Icon: CopyIcon },
    { label: 'Copy as CSV', disabled: false, action: () => void copy('csv', result.columns, rowsToCopy), Icon: CopyIcon },
    { label: 'Export row…', disabled: !onExportRow, action: () => onExportRow?.(rowIdx), Icon: DownloadIcon },
  ];

  return (
    <div
      ref={menuRef}
      role="menu"
      aria-label="Row actions"
      style={{ position: 'fixed', left: x, top: y, zIndex: 50, minWidth: 180, borderRadius: 6, border: '1px solid rgba(26,46,42,0.14)', background: '#fff', boxShadow: '0 4px 16px rgba(26,46,42,0.14)', padding: '4px 0' }}
    >
      {actions.map(({ label, disabled, action, Icon }) => (
        <button
          key={label}
          role="menuitem"
          type="button"
          disabled={disabled}
          style={{ display: 'flex', width: '100%', alignItems: 'center', gap: 8, padding: '6px 12px', fontSize: 12, color: disabled ? 'rgba(26,46,42,0.35)' : 'var(--color-text)', background: 'transparent', border: 'none', cursor: disabled ? 'not-allowed' : 'pointer', fontFamily: 'Geist, sans-serif', textAlign: 'left' }}
          onClick={() => { if (disabled) return; action(); onClose(); }}
        >
          <Icon size={13} style={{ color: 'var(--color-text-muted)' }} aria-hidden="true" />
          {label}
        </button>
      ))}
    </div>
  );
}
