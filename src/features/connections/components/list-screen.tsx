import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useConnections } from '../api';
import { useActiveConnectionStore } from '@/shared/stores/active-connection';
import { useOpenTabsStore } from '@/shared/stores/open-tabs';
import { ListRow } from './list-row';
import { ListEmpty } from './list-empty';
import { EditSheet } from './edit-sheet';
import type { Connection, Environment } from '@/shared/types';

type GroupKey = 'recent' | 'azure_sql' | 'sql_server' | 'sql_dw';

const GROUP_LABELS: Record<GroupKey, string> = {
  recent: 'Recent',
  azure_sql: 'Azure SQL',
  sql_server: 'SQL Server (On-Premises)',
  sql_dw: 'SQL Data Warehouse',
};

function groupConnections(connections: Connection[]): Map<GroupKey, Connection[]> {
  const map = new Map<GroupKey, Connection[]>([
    ['recent', []], ['azure_sql', []], ['sql_server', []], ['sql_dw', []],
  ]);
  const cutoff = Date.now() - 7 * 24 * 60 * 60 * 1000;
  for (const conn of connections) {
    const lastMs = conn.lastUsed ? new Date(conn.lastUsed).getTime() : 0;
    if (lastMs > cutoff) {
      map.get('recent')!.push(conn);
    } else if (conn.server.includes('.azuresynapse.net') || conn.server.includes('sqldw')) {
      map.get('sql_dw')!.push(conn);
    } else if (conn.server.includes('.database.windows.net') || conn.server.includes('.azure.')) {
      map.get('azure_sql')!.push(conn);
    } else {
      map.get('sql_server')!.push(conn);
    }
  }
  return map;
}

interface ListScreenProps {
  onAddConnection: () => void;
}

export function ListScreen({ onAddConnection }: ListScreenProps) {
  const navigate = useNavigate();
  const { data: connections, isLoading, error } = useConnections();
  const setActiveConnection = useActiveConnectionStore((s) => s.setActiveConnection);
  const openTab = useOpenTabsStore((s) => s.openTab);
  const [editing, setEditing] = useState<Connection | null>(null);

  if (isLoading) {
    return (
      <div style={{ display: 'flex', height: '100%', alignItems: 'center', justifyContent: 'center' }}>
        <span style={{ fontSize: 13, color: 'var(--color-text-muted)' }}>Loading connections…</span>
      </div>
    );
  }

  if (error) {
    return (
      <div style={{ display: 'flex', height: '100%', alignItems: 'center', justifyContent: 'center' }}>
        <span style={{ fontSize: 13, color: 'var(--color-error)' }}>Failed to load connections.</span>
      </div>
    );
  }

  if (!connections || connections.length === 0) {
    return <ListEmpty onAdd={onAddConnection} />;
  }

  const groups = groupConnections(connections);
  const totalCount = connections.length;
  const envCount = new Set(
    connections.map((c) => c.environment).filter((e): e is Environment => e !== undefined),
  ).size;

  const handleConnect = (conn: Connection): void => {
    setActiveConnection(conn.id);
    const tabId = openTab({
      id: crypto.randomUUID(),
      connectionId: conn.id,
      title: `${conn.database} · ${conn.server}`,
      sql: '',
      isDirty: false,
      createdAt: new Date().toISOString(),
    });
    navigate(`/editor?tab=${tabId}`);
  };

  return (
    <div style={{ display: 'flex', height: '100%', flexDirection: 'column', background: 'var(--color-bg)' }}>
      <div style={{ background: 'var(--color-bg)', borderBottom: '1px solid rgba(26,46,42,0.08)', padding: '0 24px', height: 52, display: 'flex', alignItems: 'center', justifyContent: 'space-between', flexShrink: 0 }}>
        <div>
          <h1 style={{ fontSize: 16, fontWeight: 600, color: 'var(--color-text)', letterSpacing: '-0.02em', margin: 0 }}>Connections</h1>
          <p style={{ fontSize: 12, color: 'var(--color-text-muted)', margin: '1px 0 0' }}>
            {totalCount} connection{totalCount !== 1 ? 's' : ''} across {envCount} environment{envCount !== 1 ? 's' : ''}
          </p>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 8, background: 'rgba(26,46,42,0.05)', border: '1px solid rgba(26,46,42,0.10)', borderRadius: 8, padding: '6px 12px', width: 220 }}>
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true">
              <circle cx="6" cy="6" r="4.5" stroke="var(--color-text-muted)" strokeWidth="1.3" />
              <path d="M10 10l2.5 2.5" stroke="var(--color-text-muted)" strokeWidth="1.3" strokeLinecap="round" />
            </svg>
            <span style={{ fontSize: 13, color: 'var(--color-text-muted)' }}>Filter connections…</span>
          </div>
          <button
            type="button"
            onClick={onAddConnection}
            aria-label="Add connection"
            style={{ background: 'var(--color-accent)', color: '#fff', fontSize: 13, fontWeight: 500, padding: '7px 16px', borderRadius: 8, border: 'none', cursor: 'pointer', display: 'flex', alignItems: 'center', gap: 6, letterSpacing: '-0.01em', fontFamily: 'Geist, sans-serif' }}
          >
            <svg width="13" height="13" viewBox="0 0 13 13" fill="none" aria-hidden="true">
              <path d="M6.5 1.5v10M1.5 6.5h10" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
            </svg>
            Add connection
          </button>
        </div>
      </div>

      <div style={{ flex: 1, overflowY: 'auto', padding: '8px 0' }}>
        {(Object.entries(GROUP_LABELS) as [GroupKey, string][]).map(([key, label], idx) => {
          const items = groups.get(key) ?? [];
          if (items.length === 0) return null;
          return (
            <div key={key}>
              {idx > 0 && <div style={{ borderTop: '1px solid rgba(26,46,42,0.07)', margin: '12px 20px 4px' }} />}
              <div style={{ fontSize: 10, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase', color: 'var(--color-text-muted)', padding: '16px 20px 6px' }}>
                {label}
              </div>
              <div style={{ padding: '0 12px' }}>
                {items.map((conn) => (
                  <ListRow key={conn.id} conn={conn} onConnect={handleConnect} onEdit={setEditing} />
                ))}
              </div>
            </div>
          );
        })}
      </div>

      <div style={{ background: 'var(--color-bg)', borderTop: '1px solid rgba(26,46,42,0.08)', padding: '0 20px', height: 28, display: 'flex', alignItems: 'center', flexShrink: 0 }}>
        <span style={{ fontSize: 11, color: 'var(--color-text-muted)', fontFamily: 'Geist Mono, monospace' }}>{totalCount} connections</span>
      </div>

      <EditSheet
        connection={editing}
        open={editing !== null}
        onOpenChange={(open) => { if (!open) setEditing(null); }}
      />
    </div>
  );
}
