import type { ImportPreview } from '../types';

interface Props {
  preview: ImportPreview;
}

export function StepPreview({ preview }: Props) {
  return (
    <div>
      <p style={{ fontSize: 12, color: 'var(--color-primary)', margin: '0 0 10px' }}>
        First {preview.rows.length} rows · {preview.headers.length} columns.
      </p>
      <div style={{ border: '1px solid rgba(42,87,81,0.1)', borderRadius: 8, overflow: 'auto', maxHeight: 320, background: '#fff' }}>
        <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 11, fontFamily: 'Geist Mono, monospace' }}>
          <thead style={{ background: 'rgba(42,87,81,0.04)', position: 'sticky', top: 0 }}>
            <tr>
              {preview.headers.map((h, i) => (
                <th key={h} style={{ padding: '8px 10px', textAlign: 'left', borderBottom: '1px solid rgba(42,87,81,0.1)', color: 'var(--color-primary)', fontWeight: 600, whiteSpace: 'nowrap' }}>
                  {h}
                  <div style={{ fontSize: 9, color: 'rgba(42,87,81,0.5)', fontWeight: 500, marginTop: 2 }}>
                    {preview.inferredTypes[i] ?? ''}
                  </div>
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {preview.rows.map((row, i) => (
              <tr key={i} style={{ borderBottom: '1px solid rgba(42,87,81,0.05)' }}>
                {preview.headers.map((_, j) => {
                  const v = row[j] ?? '';
                  return (
                    <td key={j} style={{ padding: '6px 10px', color: 'var(--color-primary)', whiteSpace: 'nowrap' }}>
                      {v.length > 60 ? `${v.slice(0, 60)}…` : v || <span style={{ opacity: 0.3 }}>∅</span>}
                    </td>
                  );
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
