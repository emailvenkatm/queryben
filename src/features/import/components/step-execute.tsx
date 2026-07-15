import type { ImportOptions, ImportResult } from '../types';

interface Props {
  options: ImportOptions;
  onChange: (o: ImportOptions) => void;
  targetSchema: string;
  targetTable: string;
  onSchemaChange: (v: string) => void;
  onTableChange: (v: string) => void;
  connectionMissing: boolean;
  result: ImportResult | null;
  isRunning: boolean;
}

const INPUT: React.CSSProperties = { width: '100%', padding: '7px 10px', fontSize: 12, fontFamily: 'Geist Mono, monospace', border: '1px solid rgba(42,87,81,0.14)', borderRadius: 4, background: '#fff', color: 'var(--color-primary)' };

export function StepExecute({ options, onChange, targetSchema, targetTable, onSchemaChange, onTableChange, connectionMissing, result, isRunning }: Props) {
  return (
    <div>
      <div style={{ display: 'flex', gap: 10, marginBottom: 14 }}>
        <label style={{ display: 'flex', flexDirection: 'column', gap: 4, flex: 1 }}>
          <span style={{ fontSize: 11, color: 'var(--color-primary)', fontWeight: 600 }}>Target schema</span>
          <input type="text" value={targetSchema} onChange={(e) => onSchemaChange(e.target.value)} disabled={isRunning} style={INPUT} />
        </label>
        <label style={{ display: 'flex', flexDirection: 'column', gap: 4, flex: 2 }}>
          <span style={{ fontSize: 11, color: 'var(--color-primary)', fontWeight: 600 }}>Target table</span>
          <input type="text" value={targetTable} onChange={(e) => onTableChange(e.target.value)} disabled={isRunning} style={INPUT} />
        </label>
      </div>

      <fieldset style={{ border: '1px solid rgba(42,87,81,0.1)', borderRadius: 8, padding: 4, marginBottom: 12 }}>
        <legend style={{ padding: '0 6px', fontSize: 11, color: 'var(--color-primary)', fontWeight: 600 }}>Options</legend>
        <label style={{ display: 'flex', gap: 10, padding: '8px 10px', cursor: 'pointer', borderRadius: 6 }}>
          <input type="checkbox" checked={options.skipHeaderRow} onChange={(e) => onChange({ ...options, skipHeaderRow: e.target.checked })} disabled={isRunning} style={{ marginTop: 3, accentColor: 'var(--color-accent)' }} />
          <div>
            <div style={{ fontSize: 13, fontWeight: 500, color: 'var(--color-primary)' }}>Skip header row</div>
            <div style={{ fontSize: 11, color: 'rgba(42,87,81,0.55)', marginTop: 2 }}>Treat the first row as column names, not data.</div>
          </div>
        </label>
        <label style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '8px 10px' }}>
          <span style={{ fontSize: 13, fontWeight: 500, color: 'var(--color-primary)', minWidth: 100 }}>Batch size</span>
          <input type="number" min={1} max={10000} value={options.batchSize ?? 500} onChange={(e) => onChange({ ...options, batchSize: Number(e.target.value) || 500 })} disabled={isRunning} style={{ ...INPUT, width: 100 }} />
          <span style={{ fontSize: 11, color: 'rgba(42,87,81,0.55)' }}>rows per batch</span>
        </label>
      </fieldset>

      {connectionMissing && <div style={{ fontSize: 12, color: 'var(--color-error)', marginBottom: 8 }}>Open a connection first.</div>}

      {isRunning && (
        <div role="status" style={{ fontSize: 12, color: '#fff', background: 'var(--color-primary)', padding: '10px 12px', borderRadius: 8, display: 'flex', alignItems: 'center', gap: 8 }}>
          <span aria-hidden style={{ width: 10, height: 10, borderRadius: '50%', background: 'var(--color-accent)' }} />
          Importing rows…
        </div>
      )}

      {result && (
        <div role="status" style={{ fontSize: 12, color: result.rowsFailed === 0 ? 'var(--color-success)' : 'var(--color-warning)', background: result.rowsFailed === 0 ? 'rgba(46,125,50,0.10)' : 'rgba(213,138,74,0.10)', border: `1px solid ${result.rowsFailed === 0 ? 'rgba(46,125,50,0.25)' : 'rgba(213,138,74,0.30)'}`, padding: '10px 12px', borderRadius: 8 }}>
          <div style={{ fontWeight: 600, marginBottom: 4 }}>
            {result.rowsFailed === 0 ? `Imported ${result.rowsInserted.toLocaleString()} rows.` : `Imported ${result.rowsInserted.toLocaleString()} rows; ${result.rowsFailed} failed.`}
          </div>
          {result.errors.length > 0 && (
            <details style={{ marginTop: 6 }}>
              <summary style={{ cursor: 'pointer', color: 'var(--color-primary)' }}>{result.errors.length} row error{result.errors.length === 1 ? '' : 's'}</summary>
              <ul style={{ margin: '6px 0 0', paddingLeft: 20, color: 'var(--color-primary)', fontFamily: 'Geist Mono, monospace', fontSize: 11 }}>
                {result.errors.slice(0, 10).map((e, i) => <li key={i}>row {e.row}: {e.message}</li>)}
                {result.errors.length > 10 && <li>… and {result.errors.length - 10} more</li>}
              </ul>
            </details>
          )}
        </div>
      )}
    </div>
  );
}
