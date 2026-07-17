import type { QueryTab } from '@/shared/types';

interface TabStripProps {
  tabs: QueryTab[];
  activeTabId: string | null;
  onTabChange: (id: string) => void;
  onTabClose: (id: string) => void;
  onNewTab?: () => void;
}

export function TabStrip({ tabs, activeTabId, onTabChange, onTabClose, onNewTab }: TabStripProps) {
  if (tabs.length === 0) return null;

  return (
    <div
      role="tablist"
      aria-label="Query tabs"
      style={{ background: 'rgba(26,46,42,0.04)', borderBottom: '1px solid rgba(26,46,42,0.08)', display: 'flex', alignItems: 'flex-end', padding: '0 16px 0', gap: 2, flexShrink: 0, overflowX: 'auto' }}
      className="scrollbar-none"
    >
      {tabs.map((tab) => {
        const isActive = tab.id === activeTabId;
        return (
          <div
            key={tab.id}
            role="tab"
            aria-selected={isActive}
            tabIndex={0}
            style={{ padding: '8px 16px 7px', fontSize: 12, fontWeight: 500, color: isActive ? 'var(--color-text)' : 'var(--color-text-muted)', cursor: 'pointer', borderBottom: `2px solid ${isActive ? 'var(--color-accent)' : 'transparent'}`, display: 'flex', alignItems: 'center', gap: 6, whiteSpace: 'nowrap', flexShrink: 0, transition: 'color 80ms', fontFamily: 'Geist, sans-serif' }}
            onClick={() => onTabChange(tab.id)}
            onKeyDown={(e) => e.key === 'Enter' && onTabChange(tab.id)}
          >
            <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true">
              <path d="M1 1h10v10H1z" stroke={isActive ? 'rgba(213,138,74,0.6)' : 'rgba(26,46,42,0.25)'} strokeWidth="1" rx="1" />
              <path d="M3 4h6M3 7h4" stroke={isActive ? 'rgba(213,138,74,0.6)' : 'rgba(26,46,42,0.25)'} strokeWidth="1" strokeLinecap="round" />
            </svg>

            <span style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
              {tab.isDirty && (
                <span style={{ width: 5, height: 5, borderRadius: '50%', background: 'var(--color-accent)', display: 'inline-block' }} aria-hidden="true" />
              )}
              {tab.title}
            </span>

            <button
              type="button"
              onClick={(e) => { e.stopPropagation(); onTabClose(tab.id); }}
              aria-label={`Close ${tab.title}`}
              style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', width: 14, height: 14, flexShrink: 0, borderRadius: 3, border: 'none', background: 'transparent', cursor: 'pointer', opacity: isActive ? 0.6 : 0, padding: 0, color: 'currentColor' }}
              onMouseEnter={(e) => { (e.currentTarget as HTMLElement).style.opacity = '1'; (e.currentTarget as HTMLElement).style.background = 'rgba(0,0,0,0.05)'; }}
              onMouseLeave={(e) => { (e.currentTarget as HTMLElement).style.opacity = isActive ? '0.6' : '0'; (e.currentTarget as HTMLElement).style.background = 'transparent'; }}
            >
              <svg width="8" height="8" viewBox="0 0 8 8" fill="none" aria-hidden="true">
                <path d="M1 1l6 6M7 1L1 7" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
              </svg>
            </button>
          </div>
        );
      })}

      <button
        type="button"
        onClick={onNewTab}
        aria-label="New query tab"
        style={{ padding: '6px 10px', color: 'var(--color-text-muted)', background: 'none', border: 'none', cursor: 'pointer', display: 'flex', alignItems: 'center', justifyContent: 'center' }}
      >
        <svg width="13" height="13" viewBox="0 0 13 13" fill="none" aria-hidden="true">
          <path d="M6.5 2v9M2 6.5h9" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
        </svg>
      </button>
    </div>
  );
}
