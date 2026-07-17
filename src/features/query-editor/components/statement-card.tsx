import type { PendingChange, PendingChangeKind } from '@/shared/stores/pending-changes';

const KIND_ACCENT: Record<PendingChangeKind, { band: string; badgeBg: string; badgeColor: string; label: string }> = {
  update: { band: 'var(--color-accent)',        badgeBg: 'rgba(245,158,11,0.2)',  badgeColor: 'var(--color-warning)', label: 'UPDATE' },
  insert: { band: 'var(--color-primary-hover)', badgeBg: 'rgba(34,197,94,0.2)',   badgeColor: 'var(--color-success)', label: 'INSERT' },
  delete: { band: 'var(--color-error)',         badgeBg: 'rgba(220,38,38,0.2)',   badgeColor: 'var(--color-error)',   label: 'DELETE' },
};

interface StatementCardProps {
  change: PendingChange;
  isFailed: boolean;
  onRevert: () => void;
}

export function StatementCard({ change, isFailed, onRevert }: StatementCardProps) {
  const accent = KIND_ACCENT[change.kind];
  return (
    <div style={{ borderRadius: 6, overflow: 'hidden', border: `1px solid ${isFailed ? 'rgba(220,38,38,0.5)' : 'rgba(244,239,231,0.08)'}`, borderLeft: `3px solid ${accent.band}`, background: isFailed ? 'rgba(220,38,38,0.06)' : 'transparent' }}>
      <div style={{ display: 'flex', alignItems: 'center', padding: '7px 12px', gap: 8, background: 'rgba(244,239,231,0.04)', borderBottom: '1px solid rgba(244,239,231,0.06)' }}>
        <span style={{ fontFamily: 'Geist Mono, monospace', fontSize: 10, fontWeight: 700, padding: '2px 6px', borderRadius: 3, letterSpacing: '0.06em', background: accent.badgeBg, color: accent.badgeColor }}>
          {accent.label}
        </span>
        <span style={{ fontSize: 11, color: 'rgba(244,239,231,0.6)', fontFamily: 'Geist Mono, monospace' }}>
          row {change.rowId}{change.columnName ? ` · ${change.columnName}` : ''}
        </span>
        <div style={{ flex: 1 }} />
        <button
          type="button"
          onClick={onRevert}
          style={{ fontSize: 11, color: 'rgba(244,239,231,0.5)', cursor: 'pointer', background: 'none', border: '1px solid rgba(244,239,231,0.10)', borderRadius: 4, padding: '2px 8px', fontFamily: 'Geist, sans-serif' }}
        >
          Revert
        </button>
      </div>
      <pre style={{ background: 'var(--color-bg-sidebar)', padding: '10px 14px', fontFamily: 'Geist Mono, monospace', fontSize: 12, lineHeight: 1.65, color: '#D4C9B8', overflowX: 'auto', margin: 0, whiteSpace: 'pre' }}>
        {change.sql}
      </pre>
    </div>
  );
}
