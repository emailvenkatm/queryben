import { useState } from 'react';
import ReactMarkdown from 'react-markdown';
import type { Cell } from '../types';

interface Props {
  cell: Cell;
  onChange: (source: string) => void;
}

export function MarkdownCellView({ cell, onChange }: Props) {
  const [editing, setEditing] = useState(cell.source.length === 0);

  if (editing) {
    return (
      <div style={{ padding: 12, background: 'var(--color-bg)' }}>
        <textarea
          value={cell.source}
          onChange={(e) => onChange(e.target.value)}
          onBlur={() => { if (cell.source.trim().length > 0) setEditing(false); }}
          autoFocus
          rows={Math.max(3, cell.source.split('\n').length)}
          placeholder="Write markdown (Cmd/Ctrl+Enter to render)"
          onKeyDown={(e) => {
            if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
              e.preventDefault();
              setEditing(false);
            }
          }}
          style={{
            width: '100%',
            fontFamily: 'Geist Mono, monospace',
            fontSize: 13,
            lineHeight: 1.5,
            border: '1px solid rgba(26,46,42,0.15)',
            borderRadius: 6,
            padding: 10,
            background: 'var(--color-bg-elevated)',
            color: 'var(--color-text)',
            resize: 'vertical',
            outline: 'none',
          }}
        />
      </div>
    );
  }

  return (
    <div
      style={{ padding: 12, background: 'var(--color-bg)', cursor: 'text' }}
      onDoubleClick={() => setEditing(true)}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => { if (e.key === 'Enter') setEditing(true); }}
      aria-label="Markdown cell (double-click to edit)"
    >
      <div
        style={{
          fontSize: 14,
          lineHeight: 1.6,
          color: 'var(--color-text)',
          fontFamily: 'Geist, sans-serif',
        }}
      >
        <ReactMarkdown>
          {cell.source || '*Empty markdown cell -- double-click to edit.*'}
        </ReactMarkdown>
      </div>
    </div>
  );
}
