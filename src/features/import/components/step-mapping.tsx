import { SQL_TYPE_CHOICES } from '../types';
import type { ColumnMap } from '../types';

interface Props {
  mapping: ColumnMap[];
  onChange: (m: ColumnMap[]) => void;
}

const TH: React.CSSProperties = { padding: '8px 10px', textAlign: 'left', borderBottom: '1px solid rgba(42,87,81,0.1)', color: 'var(--color-primary)', fontWeight: 600, fontSize: 11, fontFamily: 'Geist, sans-serif' };
const TD: React.CSSProperties = { padding: '6px 10px', verticalAlign: 'top' };
const INPUT: React.CSSProperties = { width: '100%', padding: '5px 8px', fontSize: 12, fontFamily: 'Geist Mono, monospace', border: '1px solid rgba(42,87,81,0.14)', borderRadius: 4, background: '#fff', color: 'var(--color-primary)' };

export function StepMapping({ mapping, onChange }: Props) {
  const patch = (i: number, delta: Partial<ColumnMap>) => {
    onChange(mapping.map((m, idx) => (idx === i ? { ...m, ...delta } : m)));
  };

  return (
    <div>
      <p style={{ fontSize: 12, color: 'var(--color-primary)', margin: '0 0 10px' }}>
        Rename columns or change types. Uncheck rows to skip them.
      </p>
      <div style={{ border: '1px solid rgba(42,87,81,0.1)', borderRadius: 8, overflow: 'auto', maxHeight: 360, background: '#fff' }}>
        <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 12 }}>
          <thead style={{ background: 'rgba(42,87,81,0.04)', position: 'sticky', top: 0 }}>
            <tr>
              <th style={TH} />
              <th style={TH}>Source</th>
              <th style={TH}>Target</th>
              <th style={TH}>SQL type</th>
            </tr>
          </thead>
          <tbody>
            {mapping.map((m, i) => {
              const isCustom = !(SQL_TYPE_CHOICES as readonly string[]).includes(m.targetType);
              return (
                <tr key={i} style={{ borderBottom: '1px solid rgba(42,87,81,0.05)', opacity: m.include ? 1 : 0.5 }}>
                  <td style={{ ...TD, width: 34 }}>
                    <input type="checkbox" checked={m.include} onChange={(e) => patch(i, { include: e.target.checked })} aria-label={`Include ${m.sourceColumn}`} style={{ accentColor: 'var(--color-accent)' }} />
                  </td>
                  <td style={{ ...TD, fontFamily: 'Geist Mono, monospace', color: 'rgba(42,87,81,0.7)' }}>{m.sourceColumn}</td>
                  <td style={TD}>
                    <input type="text" value={m.targetColumn} onChange={(e) => patch(i, { targetColumn: e.target.value })} disabled={!m.include} style={INPUT} />
                  </td>
                  <td style={TD}>
                    <select
                      value={isCustom ? '__custom__' : m.targetType}
                      onChange={(e) => { if (e.target.value !== '__custom__') patch(i, { targetType: e.target.value }); }}
                      disabled={!m.include}
                      style={{ ...INPUT, marginBottom: isCustom ? 4 : 0 }}
                    >
                      {SQL_TYPE_CHOICES.map((t) => <option key={t} value={t}>{t}</option>)}
                      <option value="__custom__">Custom…</option>
                    </select>
                    {isCustom && (
                      <input type="text" value={m.targetType} onChange={(e) => patch(i, { targetType: e.target.value })} placeholder="e.g. VARCHAR(200)" disabled={!m.include} style={INPUT} />
                    )}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
}
