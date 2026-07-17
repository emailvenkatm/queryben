import { useTableDesigner } from '../hooks/use-table-designer';
import { ColumnList } from './column-list';
import { IndexEditor } from './index-editor';
import { FkEditor } from './fk-editor';
import { DdlPreview } from './ddl-preview';

const LABEL: React.CSSProperties = { fontSize: 10, fontFamily: 'Geist Mono, monospace', textTransform: 'uppercase', letterSpacing: '0.06em', color: 'var(--color-text-muted)' };
const FIELD: React.CSSProperties = { padding: '5px 8px', border: '1px solid rgba(26,46,42,0.14)', borderRadius: 4, fontFamily: 'Geist Mono, monospace', fontSize: 12, color: 'var(--color-text)' };

export function TableDesignerScreen() {
  const { isNew, next, ddl, isLoading, loadError, applyError, applyOk, isApplying, isGenerating, setNext, togglePk, apply } = useTableDesigner();

  if (!next && isLoading && !isNew) {
    return <div style={{ padding: 24, color: 'var(--color-text-muted)' }}>Loading…</div>;
  }
  if (loadError && !isNew) {
    return <div style={{ padding: 24, color: 'var(--color-error)' }}>Couldn't load table: {loadError instanceof Error ? loadError.message : String(loadError)}</div>;
  }
  if (!next) {
    return <div style={{ padding: 24, color: 'var(--color-text-muted)' }}>Getting things ready…</div>;
  }

  const availableColumns = next.columns.map((c) => c.name).filter((n) => n.length > 0);
  const canApply = ddl.length > 0 && !isApplying && !isGenerating;

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', background: 'var(--color-bg)', overflow: 'hidden' }}>
      <header style={{ padding: '10px 20px', borderBottom: '1px solid rgba(26,46,42,0.08)', display: 'flex', alignItems: 'center', gap: 12, flexShrink: 0 }}>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
          <div style={{ ...LABEL }}>{isNew ? 'New table' : 'Design table'}</div>
          <div style={{ fontSize: 16, fontWeight: 500, color: 'var(--color-text)', fontFamily: 'Geist Mono, monospace' }}>[{next.schema}].[{next.name}]</div>
        </div>
        <div style={{ marginLeft: 'auto', display: 'flex', alignItems: 'center', gap: 8 }}>
          {applyOk && <span style={{ fontSize: 12, color: 'var(--color-success, #2A5751)' }}>{applyOk}</span>}
          {applyError && <span style={{ fontSize: 12, color: 'var(--color-error)' }}>{applyError}</span>}
          <button type="button" onClick={apply} disabled={!canApply} aria-busy={isApplying} style={{ padding: '7px 16px', borderRadius: 8, border: 'none', background: 'var(--color-accent)', color: '#fff', fontSize: 13, fontWeight: 500, fontFamily: 'Geist, sans-serif', cursor: canApply ? 'pointer' : 'not-allowed', opacity: canApply ? 1 : 0.5, letterSpacing: '-0.01em' }}>
            {isApplying ? 'Applying…' : `Apply ${ddl.length} change${ddl.length === 1 ? '' : 's'}`}
          </button>
        </div>
      </header>

      <div style={{ flex: 1, minHeight: 0, display: 'grid', gridTemplateColumns: '1fr', gridTemplateRows: '1fr 45%' }}>
        <div style={{ overflowY: 'auto', padding: 20, display: 'flex', flexDirection: 'column', gap: 20 }}>
          <div style={{ display: 'flex', gap: 12, flexWrap: 'wrap' }}>
            <label style={{ display: 'flex', flexDirection: 'column', gap: 3, minWidth: 160 }}>
              <span style={LABEL}>Schema</span>
              <input value={next.schema} onChange={(e) => setNext({ ...next, schema: e.target.value })} disabled={!isNew} style={{ ...FIELD, background: isNew ? 'var(--color-bg)' : 'var(--color-bg-elevated)' }} aria-label="Schema" />
            </label>
            <label style={{ display: 'flex', flexDirection: 'column', gap: 3, minWidth: 220 }}>
              <span style={LABEL}>Table name</span>
              <input value={next.name} onChange={(e) => setNext({ ...next, name: e.target.value })} disabled={!isNew} style={{ ...FIELD, background: isNew ? 'var(--color-bg)' : 'var(--color-bg-elevated)' }} aria-label="Table name" />
            </label>
            <label style={{ display: 'flex', flexDirection: 'column', gap: 3, minWidth: 220 }}>
              <span style={LABEL}>PK constraint name (optional)</span>
              <input value={next.pkName ?? ''} onChange={(e) => setNext({ ...next, pkName: e.target.value === '' ? null : e.target.value })} placeholder={`PK_${next.name}`} style={{ ...FIELD, background: 'var(--color-bg)' }} aria-label="PK constraint name" />
            </label>
          </div>

          <ColumnList columns={next.columns} primaryKey={next.primaryKey} onChange={(cols) => setNext({ ...next, columns: cols })} onTogglePk={togglePk} />
          <IndexEditor indexes={next.indexes} availableColumns={availableColumns} onChange={(idxs) => setNext({ ...next, indexes: idxs })} />
          <FkEditor foreignKeys={next.foreignKeys} availableColumns={availableColumns} onChange={(fks) => setNext({ ...next, foreignKeys: fks })} />
        </div>
        <DdlPreview statements={ddl} isGenerating={isGenerating} />
      </div>
    </div>
  );
}
