import type { Cell, CellKind } from '../types';
import { MarkdownCellView } from './MarkdownCellView';
import { SqlCellView } from './SqlCellView';

interface Props {
  cell: Cell;
  connectionId: string | null;
  index: number;
  onChange: (source: string) => void;
  onDelete: () => void;
  onInsertBelow: (kind: CellKind) => void;
}

export function NotebookCell({
  cell,
  connectionId,
  index,
  onChange,
  onDelete,
  onInsertBelow,
}: Props) {
  return (
    <section
      aria-label={`Cell ${index + 1}: ${cell.kind}`}
      style={{
        border: '1px solid rgba(26,46,42,0.10)',
        borderRadius: 8,
        overflow: 'hidden',
        background: 'var(--color-bg)',
      }}
    >
      <CellHeader kind={cell.kind} index={index} onDelete={onDelete} />
      {cell.kind === 'sql' ? (
        <SqlCellView cell={cell} connectionId={connectionId} onChange={onChange} />
      ) : (
        <MarkdownCellView cell={cell} onChange={onChange} />
      )}
      <InsertBar onInsertBelow={onInsertBelow} />
    </section>
  );
}

const kindBadge = (kind: Cell['kind']): React.CSSProperties => ({
  background: kind === 'sql' ? 'rgba(42,87,81,0.10)' : 'rgba(213,138,74,0.10)',
  color: kind === 'sql' ? 'var(--color-primary)' : 'var(--color-accent)',
  padding: '1px 6px',
  borderRadius: 3,
  fontSize: 10,
  fontWeight: 600,
  letterSpacing: '0.04em',
  textTransform: 'uppercase',
});

function CellHeader({
  kind,
  index,
  onDelete,
}: {
  kind: Cell['kind'];
  index: number;
  onDelete: () => void;
}) {
  return (
    <div
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: 8,
        padding: '5px 10px',
        borderBottom: '1px solid rgba(26,46,42,0.06)',
        background: 'var(--color-bg-elevated)',
        fontSize: 11,
        fontFamily: 'Geist Mono, monospace',
        color: 'var(--color-text-muted)',
      }}
    >
      <span style={{ fontWeight: 600 }}>[{index + 1}]</span>
      <span style={kindBadge(kind)}>{kind}</span>
      <div style={{ flex: 1 }} />
      <button
        type="button"
        onClick={onDelete}
        aria-label={`Delete cell ${index + 1}`}
        style={{
          border: 'none',
          background: 'transparent',
          color: 'var(--color-text-muted)',
          cursor: 'pointer',
          fontSize: 11,
          padding: '2px 6px',
          borderRadius: 3,
          fontFamily: 'Geist, sans-serif',
        }}
      >
        Delete
      </button>
    </div>
  );
}

const insertBtnStyle: React.CSSProperties = {
  border: '1px solid rgba(26,46,42,0.15)',
  background: 'transparent',
  fontSize: 11,
  padding: '2px 8px',
  borderRadius: 3,
  cursor: 'pointer',
  color: 'var(--color-text)',
  fontFamily: 'Geist, sans-serif',
};

function InsertBar({ onInsertBelow }: { onInsertBelow: (kind: CellKind) => void }) {
  return (
    <div
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: 6,
        padding: '4px 10px',
        borderTop: '1px solid rgba(26,46,42,0.05)',
        background: 'var(--color-bg-elevated)',
        fontSize: 11,
        color: 'var(--color-text-muted)',
        fontFamily: 'Geist, sans-serif',
      }}
    >
      <span>+ Add below:</span>
      <button type="button" onClick={() => onInsertBelow('sql')} style={insertBtnStyle}>
        SQL
      </button>
      <button type="button" onClick={() => onInsertBelow('markdown')} style={insertBtnStyle}>
        Markdown
      </button>
    </div>
  );
}
