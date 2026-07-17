import { useState } from 'react';
import { useHistoryList, useClearHistory } from '@/features/query-history';
import { formatAppErrorForDisplay } from '@/shared/api/errors';
import type { HistoryEntry } from '@/features/query-history';

function HistoryRow({ entry }: { entry: HistoryEntry }) {
  const ts = new Date(entry.executedAt).toLocaleString();
  const meta = [
    entry.rowCount != null ? `${entry.rowCount} rows` : null,
    entry.durationMs != null ? `${entry.durationMs} ms` : null,
  ].filter(Boolean).join(' · ');

  return (
    <li
      style={{
        padding: '10px 20px',
        borderBottom: '1px solid rgba(26,46,42,0.06)',
        display: 'flex',
        flexDirection: 'column',
        gap: 4,
      }}
    >
      <pre
        style={{
          margin: 0,
          fontFamily: 'Geist Mono, monospace',
          fontSize: 12,
          color: entry.error ? 'var(--color-error)' : 'var(--color-text)',
          whiteSpace: 'pre-wrap',
          wordBreak: 'break-word',
        }}
      >
        {entry.sql}
      </pre>
      <div style={{ fontSize: 11, color: 'var(--color-text-muted)', fontFamily: 'Geist, sans-serif' }}>
        {ts}{meta ? ` · ${meta}` : ''}
        {entry.error && (
          <span style={{ marginLeft: 8, color: 'var(--color-error)' }}>{entry.error}</span>
        )}
      </div>
    </li>
  );
}

export function QueryHistoryPage(): React.ReactElement {
  const [search, setSearch] = useState('');
  const { data, isLoading, error } = useHistoryList({ search: search || undefined, limit: 200 });
  const clearMut = useClearHistory();

  if (isLoading) {
    return (
      <div style={{ padding: 32, color: 'var(--color-text-muted)', fontSize: 13, fontFamily: 'Geist, sans-serif' }}>
        Loading…
      </div>
    );
  }

  if (error) {
    return (
      <div role="alert" style={{ padding: 32, color: 'var(--color-error)', fontSize: 13, fontFamily: 'Geist, sans-serif' }}>
        {formatAppErrorForDisplay(error)}
      </div>
    );
  }

  const entries = data ?? [];

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', background: 'var(--color-bg)' }}>
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: 12,
          padding: '10px 20px',
          borderBottom: '1px solid rgba(26,46,42,0.08)',
          flexShrink: 0,
          background: 'var(--color-bg-elevated)',
        }}
      >
        <input
          type="search"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="Search history…"
          aria-label="Search query history"
          style={{
            flex: 1,
            padding: '6px 12px',
            border: '1px solid rgba(26,46,42,0.12)',
            borderRadius: 6,
            background: 'var(--color-bg)',
            color: 'var(--color-text)',
            fontSize: 13,
            fontFamily: 'Geist, sans-serif',
            outline: 'none',
          }}
        />
        <button
          type="button"
          onClick={() => { void clearMut.mutate(null); }}
          disabled={clearMut.isPending || entries.length === 0}
          style={{
            padding: '6px 14px',
            border: '1px solid rgba(192,57,43,0.3)',
            borderRadius: 6,
            background: 'transparent',
            color: 'var(--color-error)',
            fontSize: 12,
            fontFamily: 'Geist, sans-serif',
            cursor: entries.length === 0 ? 'default' : 'pointer',
            opacity: entries.length === 0 ? 0.4 : 1,
          }}
        >
          Clear history
        </button>
      </div>

      {entries.length === 0 ? (
        <div style={{ padding: 48, textAlign: 'center', color: 'var(--color-text-muted)', fontSize: 13, fontFamily: 'Geist, sans-serif' }}>
          No history yet.
        </div>
      ) : (
        <ul style={{ listStyle: 'none', margin: 0, padding: 0, flex: 1, overflowY: 'auto' }}>
          {entries.map((e) => <HistoryRow key={e.id} entry={e} />)}
        </ul>
      )}
    </div>
  );
}
