import { useState } from 'react';
import { formatAppErrorForDisplay } from '@/shared/api/errors';
import { useConnections } from '@/features/connections';
import type { DdlStatement, ObjectChange, SchemaDiff, SchemaSnapshot } from '../types';
import { useSchemaDdl, useSchemaDiff, useSchemaSnapshot } from '../hooks/use-schema-compare';
import { DiffTree } from './DiffTree';
import { MigrationSqlPanel } from './MigrationSqlPanel';
import { ObjectDiffPanel } from './ObjectDiffPanel';

type Category = 'added' | 'dropped' | 'changed';

interface ConnOption { id: string; name: string; database: string; }

function ConnectionPicker({
  side, connections, selectedId, onSelect, disabled,
}: {
  side: 'source' | 'target';
  connections: ConnOption[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  disabled?: boolean;
}) {
  return (
    <label style={{ display: 'flex', flexDirection: 'column', gap: 4, flex: 1, minWidth: 0 }}>
      <span style={{ fontSize: 10, fontFamily: 'Geist Mono, monospace', textTransform: 'uppercase', letterSpacing: '0.06em', color: 'var(--color-text-muted)' }}>
        {side === 'source' ? 'Source' : 'Target'}
      </span>
      <select
        value={selectedId ?? ''}
        onChange={(e) => onSelect(e.target.value)}
        disabled={disabled}
        style={{ padding: '6px 10px', borderRadius: 6, border: '1px solid rgba(26,46,42,0.12)', background: 'var(--color-bg-elevated)', color: 'var(--color-text)', fontSize: 13, fontFamily: 'Geist, sans-serif' }}
      >
        <option value="">Select connection...</option>
        {connections.map((c) => (
          <option key={c.id} value={c.id}>{c.name} - {c.database}</option>
        ))}
      </select>
    </label>
  );
}

export function SchemaCompareScreen() {
  const connectionsQuery = useConnections();
  const connections: ConnOption[] = (connectionsQuery.data ?? []).map((c) => ({
    id: c.id,
    name: c.name,
    database: c.database,
  }));
  const [sourceId, setSourceId] = useState<string | null>(null);
  const [targetId, setTargetId] = useState<string | null>(null);
  const [diff, setDiff] = useState<SchemaDiff | null>(null);
  const [snapshots, setSnapshots] = useState<{ source: SchemaSnapshot | null; target: SchemaSnapshot | null }>({ source: null, target: null });
  const [ddl, setDdl] = useState<DdlStatement[]>([]);
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [selectedChange, setSelectedChange] = useState<ObjectChange | null>(null);
  const [error, setError] = useState<string | null>(null);

  const snapshotMut = useSchemaSnapshot();
  const diffMut = useSchemaDiff();
  const ddlMut = useSchemaDdl();

  const running = snapshotMut.isPending || diffMut.isPending;
  const canRun = Boolean(sourceId && targetId && sourceId !== targetId) && !running;

  async function runCompare() {
    if (!sourceId || !targetId) return;
    setError(null);
    setSelectedChange(null);
    setSelectedKey(null);
    try {
      const [source, target] = await Promise.all([
        snapshotMut.mutateAsync(sourceId),
        snapshotMut.mutateAsync(targetId),
      ]);
      setSnapshots({ source, target });
      const nextDiff = await diffMut.mutateAsync({ source, target });
      setDiff(nextDiff);
      setDdl(await ddlMut.mutateAsync(nextDiff));
    } catch (err) {
      setError(formatAppErrorForDisplay(err));
    }
  }

  async function regenerateDdl() {
    if (!diff) return;
    try {
      setDdl(await ddlMut.mutateAsync(diff));
    } catch (err) {
      setError(formatAppErrorForDisplay(err));
    }
  }

  function handleSelect(change: ObjectChange, cat: Category) {
    setSelectedChange(change);
    setSelectedKey(`${cat}:${change.qualifiedName}`);
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', overflow: 'hidden', background: 'var(--color-bg)' }}>
      <div style={{ display: 'flex', alignItems: 'flex-end', gap: 12, padding: '10px 20px', borderBottom: '1px solid rgba(26,46,42,0.08)', flexShrink: 0 }}>
        <ConnectionPicker side="source" connections={connections} selectedId={sourceId} onSelect={setSourceId} disabled={running} />
        <div aria-hidden="true" style={{ fontSize: 18, color: 'var(--color-text-muted)', padding: '0 4px 4px' }}>-&gt;</div>
        <ConnectionPicker side="target" connections={connections} selectedId={targetId} onSelect={setTargetId} disabled={running} />
        <button
          type="button"
          onClick={() => void runCompare()}
          disabled={!canRun}
          style={{ padding: '7px 16px', borderRadius: 8, border: 'none', background: 'var(--color-accent)', color: '#fff', fontSize: 13, fontWeight: 500, fontFamily: 'Geist, sans-serif', cursor: canRun ? 'pointer' : 'not-allowed', opacity: canRun ? 1 : 0.5 }}
        >
          {running ? 'Comparing...' : 'Run compare'}
        </button>
      </div>

      {error && (
        <div role="alert" style={{ padding: '8px 20px', background: 'rgba(192,57,43,0.06)', borderBottom: '1px solid rgba(192,57,43,0.25)', color: 'var(--color-error)', fontSize: 12, fontFamily: 'Geist Mono, monospace', flexShrink: 0 }}>
          <strong style={{ fontFamily: 'Geist, sans-serif', marginRight: 6 }}>Compare failed:</strong>
          {error}
        </div>
      )}

      <div style={{ display: 'flex', flex: 1, minHeight: 0 }}>
        <aside style={{ width: 320, borderRight: '1px solid rgba(26,46,42,0.08)', overflowY: 'auto', background: 'var(--color-bg)', flexShrink: 0 }}>
          {diff ? (
            <DiffTree diff={diff} selectedKey={selectedKey} onSelect={handleSelect} />
          ) : (
            <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', gap: 12, padding: '48px 24px', textAlign: 'center' }}>
              <p style={{ fontSize: 13, fontWeight: 500, color: 'var(--color-text)', margin: 0, fontFamily: 'Geist, sans-serif' }}>Compare two schemas</p>
              <p style={{ fontSize: 12, color: 'var(--color-text-muted)', margin: 0, lineHeight: 1.5, maxWidth: 220, fontFamily: 'Geist, sans-serif' }}>
                Pick a source and target connection, then hit Run compare.
              </p>
            </div>
          )}
        </aside>

        <section style={{ flex: 1, display: 'flex', flexDirection: 'column', minWidth: 0 }}>
          <div style={{ flex: 1, minHeight: 0, overflow: 'hidden' }}>
            <ObjectDiffPanel
              change={selectedChange}
              sourceLabel={snapshots.source?.label ?? 'source'}
              targetLabel={snapshots.target?.label ?? 'target'}
            />
          </div>
          <MigrationSqlPanel
            statements={ddl}
            isGenerating={ddlMut.isPending}
            onGenerate={() => void regenerateDdl()}
          />
        </section>
      </div>
    </div>
  );
}
