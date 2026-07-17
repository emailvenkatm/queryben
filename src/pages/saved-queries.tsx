import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useSavedList, useDeleteQuery } from '@/features/saved-queries';
import { useActiveConnectionStore } from '@/shared/stores/active-connection';
import { useOpenTabsStore } from '@/shared/stores/open-tabs';
import { formatAppErrorForDisplay } from '@/shared/api/errors';
import type { SavedQuery } from '@/features/saved-queries';

function SavedRow({ query, onOpen, onDelete }: { query: SavedQuery; onOpen: () => void; onDelete: () => void }) {
  const ts = new Date(query.updatedAt).toLocaleDateString();
  return (
    <li style={{ padding: '10px 20px', borderBottom: '1px solid rgba(26,46,42,0.06)', display: 'flex', alignItems: 'flex-start', gap: 12 }}>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ fontSize: 13, fontWeight: 500, color: 'var(--color-text)', fontFamily: 'Geist, sans-serif', marginBottom: 3 }}>
          {query.name}
        </div>
        <pre style={{ margin: 0, fontFamily: 'Geist Mono, monospace', fontSize: 11, color: 'var(--color-text-muted)', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
          {query.sql}
        </pre>
        <div style={{ fontSize: 11, color: 'var(--color-text-muted)', marginTop: 3, fontFamily: 'Geist, sans-serif' }}>
          {query.folder ? `${query.folder} · ` : ''}{ts}
        </div>
      </div>
      <div style={{ display: 'flex', gap: 6, flexShrink: 0 }}>
        <button type="button" onClick={onOpen} style={{ padding: '4px 10px', border: '1px solid rgba(26,46,42,0.15)', borderRadius: 5, background: 'transparent', color: 'var(--color-text)', fontSize: 11, fontFamily: 'Geist, sans-serif', cursor: 'pointer' }}>
          Open
        </button>
        <button type="button" onClick={onDelete} aria-label={`Delete ${query.name}`} style={{ padding: '4px 8px', border: '1px solid rgba(192,57,43,0.2)', borderRadius: 5, background: 'transparent', color: 'var(--color-error)', fontSize: 11, fontFamily: 'Geist, sans-serif', cursor: 'pointer' }}>
          Delete
        </button>
      </div>
    </li>
  );
}

export function SavedQueriesPage(): React.ReactElement {
  const [search, setSearch] = useState('');
  const { data, isLoading, error } = useSavedList({ search: search || undefined });
  const deleteMut = useDeleteQuery();
  const navigate = useNavigate();
  const activeConnectionId = useActiveConnectionStore((s) => s.activeConnectionId);
  const openTab = useOpenTabsStore((s) => s.openTab);

  const openQuery = (q: SavedQuery): void => {
    const connId = q.connectionId ?? activeConnectionId ?? '';
    const tabId = openTab({
      id: crypto.randomUUID(),
      connectionId: connId,
      title: q.name,
      sql: q.sql,
      isDirty: false,
      createdAt: new Date().toISOString(),
    });
    navigate(`/editor?tab=${tabId}`);
  };

  if (isLoading) {
    return <div style={{ padding: 32, color: 'var(--color-text-muted)', fontSize: 13, fontFamily: 'Geist, sans-serif' }}>Loading…</div>;
  }

  if (error) {
    return <div role="alert" style={{ padding: 32, color: 'var(--color-error)', fontSize: 13, fontFamily: 'Geist, sans-serif' }}>{formatAppErrorForDisplay(error)}</div>;
  }

  const queries = data ?? [];

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', background: 'var(--color-bg)' }}>
      <div style={{ padding: '10px 20px', borderBottom: '1px solid rgba(26,46,42,0.08)', flexShrink: 0, background: 'var(--color-bg-elevated)' }}>
        <input
          type="search"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="Search saved queries…"
          aria-label="Search saved queries"
          style={{ width: '100%', padding: '6px 12px', border: '1px solid rgba(26,46,42,0.12)', borderRadius: 6, background: 'var(--color-bg)', color: 'var(--color-text)', fontSize: 13, fontFamily: 'Geist, sans-serif', outline: 'none', boxSizing: 'border-box' }}
        />
      </div>
      {queries.length === 0 ? (
        <div style={{ padding: 48, textAlign: 'center', color: 'var(--color-text-muted)', fontSize: 13, fontFamily: 'Geist, sans-serif' }}>No saved queries yet.</div>
      ) : (
        <ul style={{ listStyle: 'none', margin: 0, padding: 0, flex: 1, overflowY: 'auto' }}>
          {queries.map((q) => (
            <SavedRow key={q.id} query={q} onOpen={() => openQuery(q)} onDelete={() => { void deleteMut.mutate(q.id); }} />
          ))}
        </ul>
      )}
    </div>
  );
}
