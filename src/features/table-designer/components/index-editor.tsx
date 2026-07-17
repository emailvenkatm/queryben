import type { DesignIndex } from '../types';

interface Props {
  indexes: DesignIndex[];
  availableColumns: string[];
  onChange: (next: DesignIndex[]) => void;
}

const CELL: React.CSSProperties = { padding: '4px 8px', borderBottom: '1px solid rgba(26,46,42,0.06)', fontSize: 12, fontFamily: 'Geist, sans-serif', color: 'var(--color-text)' };
const TH: React.CSSProperties = { ...CELL, fontFamily: 'Geist Mono, monospace', fontSize: 10, textTransform: 'uppercase', letterSpacing: '0.06em', color: 'var(--color-text-muted)', background: 'var(--color-bg-elevated)', textAlign: 'left', fontWeight: 600 };
const INPUT: React.CSSProperties = { width: '100%', padding: '3px 6px', fontFamily: 'Geist Mono, monospace', fontSize: 12, border: '1px solid rgba(26,46,42,0.14)', borderRadius: 4, background: 'var(--color-bg)', color: 'var(--color-text)' };

export function IndexEditor({ indexes, availableColumns, onChange }: Props) {
  const patch = (i: number, d: Partial<DesignIndex>) =>
    onChange(indexes.map((ix, k) => (k === i ? { ...ix, ...d } : ix)));
  const remove = (i: number) => onChange(indexes.filter((_, k) => k !== i));
  const add = () => onChange([...indexes, { name: `IX_${indexes.length + 1}`, isUnique: false, columns: availableColumns.slice(0, 1) }]);
  const toggleCol = (i: number, col: string) => {
    const cols = indexes[i]?.columns ?? [];
    patch(i, { columns: cols.includes(col) ? cols.filter((c) => c !== col) : [...cols, col] });
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
        <h3 style={{ fontSize: 12, fontFamily: 'Geist Mono, monospace', textTransform: 'uppercase', letterSpacing: '0.08em', color: 'var(--color-text-muted)', margin: 0 }}>Non-primary indexes</h3>
        <button type="button" onClick={add} disabled={availableColumns.length === 0} style={{ padding: '4px 10px', fontSize: 11, fontFamily: 'Geist, sans-serif', borderRadius: 6, border: '1px solid rgba(26,46,42,0.14)', background: 'var(--color-bg-elevated)', color: 'var(--color-text)', cursor: availableColumns.length === 0 ? 'not-allowed' : 'pointer', opacity: availableColumns.length === 0 ? 0.5 : 1 }}>
          + Add index
        </button>
      </div>
      <div style={{ border: '1px solid rgba(26,46,42,0.08)', borderRadius: 6, overflow: 'hidden', background: 'var(--color-bg)' }}>
        <table style={{ width: '100%', borderCollapse: 'collapse' }}>
          <colgroup>
            <col style={{ width: '25%' }} /><col style={{ width: 70 }} /><col /><col style={{ width: 60 }} />
          </colgroup>
          <thead>
            <tr>
              <th style={TH}>Name</th><th style={TH}>Unique</th><th style={TH}>Columns</th>
              <th style={TH} aria-label="Actions"> </th>
            </tr>
          </thead>
          <tbody>
            {indexes.length === 0 && <tr><td colSpan={4} style={{ ...CELL, textAlign: 'center', color: 'var(--color-text-muted)', padding: '20px 8px' }}>No secondary indexes.</td></tr>}
            {indexes.map((ix, i) => (
              <tr key={i}>
                <td style={CELL}><input style={INPUT} value={ix.name} onChange={(e) => patch(i, { name: e.target.value })} aria-label="Index name" /></td>
                <td style={{ ...CELL, textAlign: 'center' }}><input type="checkbox" checked={ix.isUnique} onChange={(e) => patch(i, { isUnique: e.target.checked })} aria-label="Unique" /></td>
                <td style={CELL}>
                  <div style={{ display: 'flex', flexWrap: 'wrap', gap: 4 }}>
                    {availableColumns.length === 0 && <span style={{ color: 'var(--color-text-muted)', fontSize: 11 }}>Add columns first.</span>}
                    {availableColumns.map((col) => {
                      const active = ix.columns.includes(col);
                      return (
                        <button key={col} type="button" onClick={() => toggleCol(i, col)} style={{ padding: '2px 8px', fontFamily: 'Geist Mono, monospace', fontSize: 11, borderRadius: 4, border: `1px solid ${active ? 'var(--color-accent)' : 'rgba(26,46,42,0.14)'}`, background: active ? 'var(--color-accent)' : 'var(--color-bg-elevated)', color: active ? '#fff' : 'var(--color-text)', cursor: 'pointer' }}>
                          {col}{active ? ` (${ix.columns.indexOf(col) + 1})` : ''}
                        </button>
                      );
                    })}
                  </div>
                </td>
                <td style={{ ...CELL, textAlign: 'center' }}>
                  <button type="button" onClick={() => remove(i)} aria-label={`Remove ${ix.name}`} style={{ background: 'transparent', border: 'none', color: 'var(--color-text-muted)', cursor: 'pointer', fontSize: 14, padding: 2 }}>×</button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
