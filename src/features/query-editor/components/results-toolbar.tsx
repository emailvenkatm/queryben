interface ResultsToolbarProps {
  rowCount: number;
  execMs: number;
  canExport?: boolean;
  onExport?: () => void;
}

export function ResultsToolbar({ rowCount, execMs, canExport, onExport }: ResultsToolbarProps) {
  return (
    <div
      style={{ background: 'var(--color-bg)', borderBottom: '1px solid rgba(26,46,42,0.08)', padding: '0 16px', display: 'flex', alignItems: 'center', gap: 0, flexShrink: 0, height: 36 }}
      role="tablist"
      aria-label="Result panels"
    >
      {['Results', 'Messages', 'Execution Plan'].map((label) => (
        <div
          key={label}
          role="tab"
          aria-selected={label === 'Results'}
          style={{ padding: '0 14px', height: 36, display: 'flex', alignItems: 'center', fontSize: 12, fontWeight: 500, color: label === 'Results' ? 'var(--color-text)' : 'var(--color-text-muted)', cursor: 'pointer', borderBottom: `2px solid ${label === 'Results' ? 'var(--color-primary)' : 'transparent'}`, fontFamily: 'Geist, sans-serif' }}
        >
          {label}
        </div>
      ))}
      <div style={{ flex: 1 }} />
      <div style={{ display: 'flex', alignItems: 'center', gap: 12, paddingRight: 4 }}>
        <span style={{ fontSize: 11, color: 'var(--color-text-muted)', fontFamily: 'Geist Mono, monospace' }}>{rowCount.toLocaleString()} rows</span>
        <span style={{ fontSize: 11, color: 'var(--color-text-muted)', fontFamily: 'Geist Mono, monospace' }}>{execMs} ms</span>
        <button
          type="button"
          onClick={onExport}
          disabled={!canExport}
          style={{ background: 'rgba(26,46,42,0.05)', border: 'none', borderRadius: 6, padding: '4px 10px', fontSize: 11, fontWeight: 500, color: 'var(--color-text-muted)', cursor: canExport ? 'pointer' : 'not-allowed', opacity: canExport ? 1 : 0.5, fontFamily: 'Geist, sans-serif' }}
        >
          Export…
        </button>
      </div>
    </div>
  );
}
