interface BrowseToolbarProps {
  schema: string;
  name: string;
  rowCount: number;
  pendingCount: number;
  canEdit: boolean;
  onAddRow: () => void;
  metadataError: string | null;
  isLoading: boolean;
  onRefresh?: () => void;
}

export function BrowseToolbar({
  schema,
  name,
  rowCount,
  pendingCount,
  canEdit,
  onAddRow,
  metadataError,
  isLoading,
  onRefresh,
}: BrowseToolbarProps): React.ReactElement {
  return (
    <div
      style={{ background: 'var(--color-bg)', borderBottom: '1px solid rgba(26,46,42,0.10)', padding: '8px 16px', display: 'flex', alignItems: 'center', gap: 8, flexShrink: 0 }}
      role="toolbar"
      aria-label="Table browse toolbar"
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 13 }}>
        <span style={{ color: 'var(--color-text-muted)' }}>{schema}</span>
        <span style={{ color: 'rgba(26,46,42,0.3)' }}>/</span>
        <span style={{ fontWeight: 600, color: 'var(--color-primary)' }}>{name}</span>
        <span style={{ fontSize: 11, color: 'var(--color-text-muted)', fontFamily: 'Geist Mono, monospace', marginLeft: 4 }}>
          {isLoading ? 'loading…' : `${rowCount.toLocaleString()} rows`}
        </span>
      </div>

      <span style={{ width: 1, height: 18, background: 'rgba(26,46,42,0.10)', margin: '0 2px' }} aria-hidden="true" />

      <button
        type="button"
        onClick={onAddRow}
        disabled={!canEdit}
        title={canEdit ? 'New row' : (metadataError ?? 'Read-only')}
        style={{ display: 'inline-flex', alignItems: 'center', gap: 5, padding: '5px 10px', fontSize: 12, fontWeight: 500, color: 'var(--color-text-muted)', border: '1px solid rgba(26,46,42,0.14)', background: canEdit ? '#fff' : 'rgba(26,46,42,0.03)', cursor: canEdit ? 'pointer' : 'not-allowed', borderRadius: 6, fontFamily: 'Geist, sans-serif' }}
      >
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true">
          <path d="M6 1v10M1 6h10" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
        </svg>
        New row
      </button>

      <button
        type="button"
        onClick={onRefresh}
        disabled={!onRefresh || isLoading}
        title="Refresh rows"
        style={{ display: 'inline-flex', alignItems: 'center', gap: 5, padding: '5px 10px', fontSize: 12, fontWeight: 500, color: 'var(--color-text-muted)', border: '1px solid rgba(26,46,42,0.14)', background: (!onRefresh || isLoading) ? 'rgba(26,46,42,0.03)' : '#fff', cursor: (!onRefresh || isLoading) ? 'not-allowed' : 'pointer', borderRadius: 6, fontFamily: 'Geist, sans-serif' }}
      >
        <svg
          width="12"
          height="12"
          viewBox="0 0 12 12"
          fill="none"
          aria-hidden="true"
          style={isLoading ? { animation: 'qb-spin 700ms linear infinite' } : undefined}
        >
          <path d="M10 6A4 4 0 1 1 6 2a4 4 0 0 1 2.83 1.17L10 2v3H7l1.06-1.06A2.5 2.5 0 1 0 8.5 6" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
        Refresh
      </button>

      <div style={{ flex: 1 }} />

      {metadataError && (
        <span style={{ fontSize: 11, color: 'var(--color-error)', fontFamily: 'Geist Mono, monospace' }}>
          {metadataError}
        </span>
      )}
      {!canEdit && !metadataError && (
        <span style={{ fontSize: 11, color: 'var(--color-text-muted)', fontFamily: 'Geist Mono, monospace' }}>Read-only</span>
      )}
      <span style={{ fontSize: 11, color: pendingCount > 0 ? 'var(--color-accent-hover)' : 'var(--color-text-muted)', fontFamily: 'Geist Mono, monospace', fontWeight: pendingCount > 0 ? 600 : 400 }}>
        {pendingCount} pending
      </span>
    </div>
  );
}
