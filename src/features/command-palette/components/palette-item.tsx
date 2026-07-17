function Kbd({ children }: { children: React.ReactNode }) {
  return (
    <span
      style={{ background: 'rgba(26,46,42,0.07)', border: '1px solid rgba(26,46,42,0.10)', borderRadius: 4, padding: '2px 6px', fontSize: 10, fontFamily: 'Geist Mono, monospace', color: 'var(--color-text-muted)' }}
      aria-hidden="true"
    >
      {children}
    </span>
  );
}

interface PaletteItemProps {
  icon: React.ReactNode;
  label: React.ReactNode;
  sub?: string;
  kbd?: string;
  isFocused?: boolean;
  onClick: () => void;
}

export function PaletteItem({ icon, label, sub, kbd, isFocused, onClick }: PaletteItemProps) {
  return (
    <div
      role="option"
      aria-selected={isFocused}
      onClick={onClick}
      style={{ display: 'flex', alignItems: 'center', gap: 12, padding: '9px 16px', cursor: 'pointer', position: 'relative', background: isFocused ? 'rgba(213,138,74,0.06)' : 'transparent', transition: 'background 60ms' }}
      onMouseEnter={(e) => { if (!isFocused) (e.currentTarget as HTMLElement).style.background = 'rgba(26,46,42,0.04)'; }}
      onMouseLeave={(e) => { (e.currentTarget as HTMLElement).style.background = isFocused ? 'rgba(213,138,74,0.06)' : 'transparent'; }}
    >
      {isFocused && (
        <span style={{ position: 'absolute', left: 6, top: 6, bottom: 6, width: 2, background: 'var(--color-accent)', borderRadius: 1 }} aria-hidden="true" />
      )}
      <div style={{ width: 30, height: 30, borderRadius: 8, display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0 }}>
        {icon}
      </div>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ fontSize: 13, fontWeight: 500, color: 'var(--color-text)', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
          {label}
        </div>
        {sub && (
          <div style={{ fontSize: 11, color: 'var(--color-text-muted)', marginTop: 1, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
            {sub}
          </div>
        )}
      </div>
      {kbd && (
        <div style={{ display: 'flex', gap: 4, alignItems: 'center', flexShrink: 0 }}>
          <Kbd>{kbd}</Kbd>
        </div>
      )}
    </div>
  );
}

export function SectionHeader({ label }: { label: string }) {
  return (
    <div style={{ padding: '10px 16px 4px', fontSize: 10, fontWeight: 700, color: 'var(--color-text-muted)', textTransform: 'uppercase', letterSpacing: '0.08em' }}>
      {label}
    </div>
  );
}

export function HighlightMatch({ text, query }: { text: string; query: string }) {
  if (!query) return <>{text}</>;
  const idx = text.toLowerCase().indexOf(query.toLowerCase());
  if (idx === -1) return <>{text}</>;
  return (
    <>
      {text.slice(0, idx)}
      <span style={{ color: 'var(--color-accent)', fontWeight: 600 }}>{text.slice(idx, idx + query.length)}</span>
      {text.slice(idx + query.length)}
    </>
  );
}
