import type { DesignForeignKey } from '../types';

interface Props {
  foreignKeys: DesignForeignKey[];
  availableColumns: string[];
  onChange: (next: DesignForeignKey[]) => void;
}

const CELL: React.CSSProperties = { padding: '4px 8px', borderBottom: '1px solid rgba(26,46,42,0.06)', fontSize: 12, fontFamily: 'Geist, sans-serif', color: 'var(--color-text)', verticalAlign: 'top' };
const TH: React.CSSProperties = { ...CELL, fontFamily: 'Geist Mono, monospace', fontSize: 10, textTransform: 'uppercase', letterSpacing: '0.06em', color: 'var(--color-text-muted)', background: 'var(--color-bg-elevated)', textAlign: 'left', fontWeight: 600 };
const INPUT: React.CSSProperties = { width: '100%', padding: '3px 6px', fontFamily: 'Geist Mono, monospace', fontSize: 12, border: '1px solid rgba(26,46,42,0.14)', borderRadius: 4, background: 'var(--color-bg)', color: 'var(--color-text)' };
const ACTIONS = ['', 'NO ACTION', 'CASCADE', 'SET NULL', 'SET DEFAULT'] as const;
const csv = (s: string): string[] => s.split(',').map((p) => p.trim()).filter((p) => p.length > 0);

export function FkEditor({ foreignKeys, availableColumns, onChange }: Props) {
  const patch = (i: number, d: Partial<DesignForeignKey>) =>
    onChange(foreignKeys.map((fk, k) => (k === i ? { ...fk, ...d } : fk)));
  const remove = (i: number) => onChange(foreignKeys.filter((_, k) => k !== i));
  const add = () => onChange([...foreignKeys, { name: `FK_${foreignKeys.length + 1}`, columns: availableColumns.slice(0, 1), referencedSchema: 'dbo', referencedTable: '', referencedColumns: [''], onDelete: null, onUpdate: null }]);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
        <h3 style={{ fontSize: 12, fontFamily: 'Geist Mono, monospace', textTransform: 'uppercase', letterSpacing: '0.08em', color: 'var(--color-text-muted)', margin: 0 }}>Foreign keys</h3>
        <button type="button" onClick={add} disabled={availableColumns.length === 0} style={{ padding: '4px 10px', fontSize: 11, fontFamily: 'Geist, sans-serif', borderRadius: 6, border: '1px solid rgba(26,46,42,0.14)', background: 'var(--color-bg-elevated)', color: 'var(--color-text)', cursor: availableColumns.length === 0 ? 'not-allowed' : 'pointer', opacity: availableColumns.length === 0 ? 0.5 : 1 }}>
          + Add foreign key
        </button>
      </div>
      <div style={{ border: '1px solid rgba(26,46,42,0.08)', borderRadius: 6, overflow: 'hidden', background: 'var(--color-bg)' }}>
        <table style={{ width: '100%', borderCollapse: 'collapse' }}>
          <colgroup>
            <col style={{ width: '18%' }} /><col style={{ width: '16%' }} /><col style={{ width: '14%' }} />
            <col style={{ width: '16%' }} /><col style={{ width: '16%' }} />
            <col style={{ width: '10%' }} /><col style={{ width: '10%' }} /><col style={{ width: 60 }} />
          </colgroup>
          <thead>
            <tr>
              <th style={TH}>Name</th><th style={TH}>Columns (CSV)</th><th style={TH}>Ref schema</th>
              <th style={TH}>Ref table</th><th style={TH}>Ref columns (CSV)</th>
              <th style={TH}>On DELETE</th><th style={TH}>On UPDATE</th>
              <th style={TH} aria-label="Actions"> </th>
            </tr>
          </thead>
          <tbody>
            {foreignKeys.length === 0 && <tr><td colSpan={8} style={{ ...CELL, textAlign: 'center', color: 'var(--color-text-muted)', padding: '20px 8px' }}>No foreign keys.</td></tr>}
            {foreignKeys.map((fk, i) => (
              <tr key={i}>
                <td style={CELL}><input style={INPUT} value={fk.name} onChange={(e) => patch(i, { name: e.target.value })} aria-label="FK name" /></td>
                <td style={CELL}><input style={INPUT} value={fk.columns.join(', ')} onChange={(e) => patch(i, { columns: csv(e.target.value) })} aria-label="FK columns" placeholder="col1, col2" /></td>
                <td style={CELL}><input style={INPUT} value={fk.referencedSchema} onChange={(e) => patch(i, { referencedSchema: e.target.value })} aria-label="Referenced schema" /></td>
                <td style={CELL}><input style={INPUT} value={fk.referencedTable} onChange={(e) => patch(i, { referencedTable: e.target.value })} aria-label="Referenced table" /></td>
                <td style={CELL}><input style={INPUT} value={fk.referencedColumns.join(', ')} onChange={(e) => patch(i, { referencedColumns: csv(e.target.value) })} aria-label="Referenced columns" /></td>
                <td style={CELL}>
                  <select style={{ ...INPUT, fontFamily: 'Geist, sans-serif' }} value={fk.onDelete ?? ''} onChange={(e) => patch(i, { onDelete: e.target.value === '' ? null : e.target.value })} aria-label="On delete">
                    {ACTIONS.map((a) => <option key={a || 'default'} value={a}>{a || '(default)'}</option>)}
                  </select>
                </td>
                <td style={CELL}>
                  <select style={{ ...INPUT, fontFamily: 'Geist, sans-serif' }} value={fk.onUpdate ?? ''} onChange={(e) => patch(i, { onUpdate: e.target.value === '' ? null : e.target.value })} aria-label="On update">
                    {ACTIONS.map((a) => <option key={a || 'default'} value={a}>{a || '(default)'}</option>)}
                  </select>
                </td>
                <td style={{ ...CELL, textAlign: 'center' }}>
                  <button type="button" onClick={() => remove(i)} aria-label={`Remove ${fk.name}`} style={{ background: 'transparent', border: 'none', color: 'var(--color-text-muted)', cursor: 'pointer', fontSize: 14, padding: 2 }}>×</button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
