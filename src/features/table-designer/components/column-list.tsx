import type { DesignColumn } from '../types';

interface Props {
  columns: DesignColumn[];
  primaryKey: string[];
  onChange: (cols: DesignColumn[]) => void;
  onTogglePk: (name: string) => void;
}

const CELL: React.CSSProperties = { padding: '4px 8px', borderBottom: '1px solid rgba(26,46,42,0.06)', fontSize: 12, fontFamily: 'Geist, sans-serif', color: 'var(--color-text)' };
const TH: React.CSSProperties = { ...CELL, fontFamily: 'Geist Mono, monospace', fontSize: 10, textTransform: 'uppercase', letterSpacing: '0.06em', color: 'var(--color-text-muted)', background: 'var(--color-bg-elevated)', textAlign: 'left', fontWeight: 600 };
const INPUT: React.CSSProperties = { width: '100%', padding: '3px 6px', fontFamily: 'Geist Mono, monospace', fontSize: 12, border: '1px solid rgba(26,46,42,0.14)', borderRadius: 4, background: 'var(--color-bg)', color: 'var(--color-text)' };

export function ColumnList({ columns, primaryKey, onChange, onTogglePk }: Props) {
  const patch = (idx: number, delta: Partial<DesignColumn>): void =>
    onChange(columns.map((c, i) => (i === idx ? { ...c, ...delta } : c)));
  const remove = (idx: number): void => onChange(columns.filter((_, i) => i !== idx));
  const add = (): void => onChange([...columns, { name: `Column${columns.length + 1}`, sqlType: 'nvarchar(255)', isNullable: true, isIdentity: false, isComputed: false, computedExpression: null, defaultExpression: null, ordinal: columns.length }]);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
        <h3 style={{ fontSize: 12, fontFamily: 'Geist Mono, monospace', textTransform: 'uppercase', letterSpacing: '0.08em', color: 'var(--color-text-muted)', margin: 0 }}>Columns</h3>
        <button type="button" onClick={add} style={{ padding: '4px 10px', fontSize: 11, fontFamily: 'Geist, sans-serif', borderRadius: 6, border: '1px solid rgba(26,46,42,0.14)', background: 'var(--color-bg-elevated)', color: 'var(--color-text)', cursor: 'pointer' }}>
          + Add column
        </button>
      </div>
      <div style={{ border: '1px solid rgba(26,46,42,0.08)', borderRadius: 6, overflow: 'hidden', background: 'var(--color-bg)' }}>
        <table style={{ width: '100%', borderCollapse: 'collapse' }}>
          <colgroup>
            <col style={{ width: 40 }} /><col style={{ width: '22%' }} /><col style={{ width: '22%' }} />
            <col style={{ width: 70 }} /><col style={{ width: 70 }} /><col style={{ width: 70 }} />
            <col style={{ width: '20%' }} /><col style={{ width: '20%' }} /><col style={{ width: 60 }} />
          </colgroup>
          <thead>
            <tr>
              <th style={TH} title="Primary key">PK</th>
              <th style={TH}>Name</th><th style={TH}>SQL type</th>
              <th style={TH}>Null</th><th style={TH}>Ident.</th><th style={TH}>Comp.</th>
              <th style={TH}>Default</th><th style={TH}>Formula</th>
              <th style={TH} aria-label="Actions"> </th>
            </tr>
          </thead>
          <tbody>
            {columns.length === 0 && (
              <tr><td colSpan={9} style={{ ...CELL, textAlign: 'center', color: 'var(--color-text-muted)', padding: '20px 8px' }}>No columns. Click <em>Add column</em> to start.</td></tr>
            )}
            {columns.map((col, idx) => (
              <tr key={idx}>
                <td style={{ ...CELL, textAlign: 'center' }}>
                  <input type="checkbox" checked={primaryKey.includes(col.name)} onChange={() => onTogglePk(col.name)} aria-label={`PK: ${col.name}`} />
                </td>
                <td style={CELL}><input style={INPUT} value={col.name} onChange={(e) => patch(idx, { name: e.target.value })} aria-label="Column name" /></td>
                <td style={CELL}><input style={INPUT} value={col.sqlType} onChange={(e) => patch(idx, { sqlType: e.target.value })} disabled={col.isComputed} aria-label="SQL type" /></td>
                <td style={{ ...CELL, textAlign: 'center' }}><input type="checkbox" checked={col.isNullable} onChange={(e) => patch(idx, { isNullable: e.target.checked })} disabled={col.isComputed} aria-label="Nullable" /></td>
                <td style={{ ...CELL, textAlign: 'center' }}><input type="checkbox" checked={col.isIdentity} onChange={(e) => patch(idx, { isIdentity: e.target.checked })} disabled={col.isComputed} aria-label="Identity" /></td>
                <td style={{ ...CELL, textAlign: 'center' }}>
                  <input type="checkbox" checked={col.isComputed} onChange={(e) => patch(idx, { isComputed: e.target.checked, ...(e.target.checked ? { defaultExpression: null, isIdentity: false } : { computedExpression: null }) })} aria-label="Computed" />
                </td>
                <td style={CELL}><input style={INPUT} value={col.defaultExpression ?? ''} onChange={(e) => patch(idx, { defaultExpression: e.target.value === '' ? null : e.target.value })} disabled={col.isComputed} placeholder="e.g. GETUTCDATE()" aria-label="Default" /></td>
                <td style={CELL}><input style={INPUT} value={col.computedExpression ?? ''} onChange={(e) => patch(idx, { computedExpression: e.target.value === '' ? null : e.target.value })} disabled={!col.isComputed} placeholder="e.g. [A] + [B]" aria-label="Formula" /></td>
                <td style={{ ...CELL, textAlign: 'center' }}>
                  <button type="button" onClick={() => remove(idx)} aria-label={`Remove ${col.name}`} style={{ background: 'transparent', border: 'none', color: 'var(--color-text-muted)', cursor: 'pointer', fontSize: 14, padding: 2 }}>×</button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
