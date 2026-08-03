function Kbd({ children, light }: { children: React.ReactNode; light?: boolean }) {
  return (
    <span
      style={{ background: light ? 'rgba(255,255,255,0.15)' : 'rgba(26,46,42,0.07)', borderRadius: 3, padding: '1px 5px', fontSize: 10, fontFamily: 'Geist Mono, monospace', color: light ? 'rgba(255,255,255,0.6)' : 'var(--color-text-muted)' }}
      aria-hidden="true"
    >
      {children}
    </span>
  );
}

function Sep() {
  return <span style={{ width: 1, height: 18, background: 'rgba(26,46,42,0.10)', margin: '0 4px', flexShrink: 0 }} aria-hidden="true" />;
}

interface TBtnProps {
  onClick?: () => void;
  disabled?: boolean;
  children: React.ReactNode;
  ariaLabel?: string;
}

function TBtn({ onClick, disabled, children, ariaLabel }: TBtnProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      aria-label={ariaLabel}
      style={{ display: 'flex', alignItems: 'center', gap: 5, padding: '5px 10px', fontSize: 12, fontWeight: 500, color: 'var(--color-text-muted)', border: 'none', background: 'transparent', cursor: disabled ? 'default' : 'pointer', borderRadius: 6, fontFamily: 'Geist, sans-serif', transition: 'background 80ms, color 80ms' }}
      onMouseEnter={(e) => { if (!disabled) { (e.currentTarget as HTMLElement).style.background = 'rgba(26,46,42,0.05)'; (e.currentTarget as HTMLElement).style.color = 'var(--color-text)'; } }}
      onMouseLeave={(e) => { (e.currentTarget as HTMLElement).style.background = 'transparent'; (e.currentTarget as HTMLElement).style.color = 'var(--color-text-muted)'; }}
    >
      {children}
    </button>
  );
}

interface EditorToolbarProps {
  isPending: boolean;
  hasConnection: boolean;
  onRun: () => void;
  onExplain: () => void;
  onToggleAi: () => void;
  aiOpen: boolean;
  onSaveQuery: () => void;
  onOpenSnippets: () => void;
}

export function EditorToolbar({ isPending, hasConnection, onRun, onExplain, onToggleAi, aiOpen, onSaveQuery, onOpenSnippets }: EditorToolbarProps) {
  return (
    <div style={{ background: 'var(--color-bg)', borderBottom: '1px solid rgba(26,46,42,0.08)', padding: '7px 16px', display: 'flex', alignItems: 'center', gap: 8, flexShrink: 0 }} role="toolbar" aria-label="Query editor toolbar">
      <button
        type="button"
        onClick={onRun}
        disabled={isPending || !hasConnection}
        aria-label="Run query (F5)"
        style={{ display: 'flex', alignItems: 'center', gap: 5, background: isPending || !hasConnection ? 'rgba(213,138,74,0.4)' : 'var(--color-accent)', color: '#fff', padding: '6px 14px', fontSize: 13, fontWeight: 600, border: 'none', borderRadius: 6, cursor: isPending || !hasConnection ? 'default' : 'pointer', fontFamily: 'Geist, sans-serif', transition: 'background 80ms' }}
      >
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true">
          <path d="M3 2l7 4-7 4V2z" fill="currentColor" />
        </svg>
        Run
        <Kbd light>F5</Kbd>
      </button>

      <Sep />

      <TBtn disabled={isPending || !hasConnection} onClick={onExplain} ariaLabel="Explain query">
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true">
          <path d="M2 9l3-3-3-3M7 9h3" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
        Explain
      </TBtn>

      <Sep />

      <TBtn onClick={onSaveQuery} disabled={!hasConnection} ariaLabel="Save query">
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true">
          <path d="M2.5 1.5h5.5l2 2v7a.5.5 0 0 1-.5.5h-7a.5.5 0 0 1-.5-.5v-9a.5.5 0 0 1 .5-.5z" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round" />
          <path d="M4 1.5v3h4v-3M4 8h4" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
        </svg>
        Save
      </TBtn>

      <TBtn onClick={onOpenSnippets} ariaLabel="Snippets (Shift+Cmd+P)">
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true">
          <path d="M5 2H3a1 1 0 00-1 1v6a1 1 0 001 1h6a1 1 0 001-1V7" stroke="currentColor" strokeWidth="1.3" />
          <path d="M9 1l2 2-5 5H4V6l5-5z" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round" />
        </svg>
        Snippets
        <Kbd>⇧⌘P</Kbd>
      </TBtn>

      <TBtn onClick={onToggleAi} ariaLabel={aiOpen ? 'Hide AI assistant' : 'Show AI assistant'}>
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true">
          <path d="M6 1v2M6 9v2M1 6h2M9 6h2M2.5 2.5l1.4 1.4M8.1 8.1l1.4 1.4M2.5 9.5l1.4-1.4M8.1 3.9l1.4-1.4" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
          <circle cx="6" cy="6" r="1.3" fill="currentColor" />
        </svg>
        {aiOpen ? 'Hide AI' : 'AI'}
      </TBtn>

      <div style={{ flex: 1 }} />

      <div style={{ fontSize: 11, color: 'var(--color-text-muted)', fontFamily: 'Geist, sans-serif' }}>
        SQL Server
      </div>
    </div>
  );
}
