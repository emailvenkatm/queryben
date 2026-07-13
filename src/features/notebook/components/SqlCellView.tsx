import { useState } from 'react';
import { formatAppErrorForDisplay } from '@/shared/api/errors';
import type { Cell, CellRunResult, QueryOutcome, ResultSet } from '../types';
import { useRunCell } from '../hooks/use-notebook';

// TODO: swap textarea for MonacoEditor from @/features/query-editor once it publishes index.ts
// TODO: swap ResultSetRow for SingleResultGrid from @/features/query-editor

interface Props {
  cell: Cell;
  connectionId: string | null;
  onChange: (source: string) => void;
}

export function SqlCellView({ cell, connectionId, onChange }: Props) {
  const [outcome, setOutcome] = useState<QueryOutcome | null>(null);
  const [error, setError] = useState<string | null>(null);
  const runMutation = useRunCell();

  async function run() {
    if (!connectionId) {
      setError('Pick a connection from the toolbar before running this cell.');
      return;
    }
    setError(null);
    try {
      const result: CellRunResult = await runMutation.mutateAsync({
        kind: 'sql',
        source: cell.source,
        connectionId,
      });
      if (result.kind === 'sql') setOutcome(result.outcome);
    } catch (err) {
      setError(formatAppErrorForDisplay(err));
    }
  }

  const running = runMutation.isPending;

  return (
    <div style={{ background: 'var(--color-bg)' }}>
      <div style={{ display: 'flex', alignItems: 'stretch', minHeight: 120 }}>
        <RunGutter running={running} onRun={() => void run()} disabled={!connectionId} />
        <div style={{ flex: 1, minWidth: 0, borderLeft: '1px solid rgba(26,46,42,0.08)' }}>
          <textarea
            value={cell.source}
            onChange={(e) => onChange(e.target.value)}
            onKeyDown={(e) => {
              if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
                e.preventDefault();
                void run();
              }
            }}
            placeholder="-- write SQL here"
            style={{
              display: 'block',
              width: '100%',
              height: 140,
              padding: '10px 14px',
              fontFamily: 'Geist Mono, monospace',
              fontSize: 13,
              lineHeight: 1.5,
              color: 'var(--color-text)',
              background: 'var(--color-bg)',
              border: 'none',
              borderBottom: '1px solid rgba(26,46,42,0.08)',
              outline: 'none',
              resize: 'none',
              boxSizing: 'border-box',
            }}
          />
          {running && (
            <div style={{ padding: 12, fontSize: 12, color: 'var(--color-text-muted)' }}>
              Running...
            </div>
          )}
          {!running && error && <CellError message={error} label="Cell failed" />}
          {!running && !error && outcome && <OutcomeView outcome={outcome} />}
        </div>
      </div>
    </div>
  );
}

function RunGutter({
  running,
  onRun,
  disabled,
}: {
  running: boolean;
  onRun: () => void;
  disabled: boolean;
}) {
  return (
    <div
      style={{
        width: 48,
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        paddingTop: 8,
        background: 'var(--color-bg)',
      }}
    >
      <button
        type="button"
        onClick={onRun}
        disabled={disabled || running}
        aria-label="Run cell"
        title="Run cell (Cmd/Ctrl+Enter)"
        style={{
          width: 28,
          height: 28,
          borderRadius: '50%',
          border: 'none',
          background: disabled || running ? 'rgba(213,138,74,0.35)' : 'var(--color-accent)',
          color: '#fff',
          cursor: disabled || running ? 'default' : 'pointer',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
        }}
      >
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
          <path d="M2.5 1.5l6 3.5-6 3.5z" fill="currentColor" />
        </svg>
      </button>
    </div>
  );
}

function CellError({ message, label }: { message: string; label: string }) {
  return (
    <div
      role="alert"
      style={{
        padding: '10px 14px',
        background: 'rgba(220,38,38,0.06)',
        color: 'var(--color-error)',
        fontSize: 12,
        fontFamily: 'Geist Mono, monospace',
        whiteSpace: 'pre-wrap',
        wordBreak: 'break-word',
      }}
    >
      <strong style={{ fontFamily: 'Geist, sans-serif', fontWeight: 600, marginRight: 6 }}>
        {label}:
      </strong>
      {message}
    </div>
  );
}

function ResultSetRow({ rs, index, total }: { rs: ResultSet; index: number; total: number }) {
  return (
    <div>
      {total > 1 && (
        <div
          style={{
            padding: '6px 14px',
            fontSize: 11,
            color: 'var(--color-primary)',
            background: 'rgba(42,87,81,0.04)',
            borderTop: '1px solid rgba(26,46,42,0.08)',
            borderBottom: '1px solid rgba(26,46,42,0.08)',
            fontFamily: 'Geist, sans-serif',
            fontWeight: 600,
          }}
        >
          Result {index + 1} - {rs.rowCount.toLocaleString()} rows - {rs.durationMs} ms
        </div>
      )}
      {rs.rows.length === 0 && rs.columns.length === 0 ? (
        <div style={{ padding: 12, fontSize: 12, color: 'var(--color-text-muted)' }}>
          Statement completed. No rows returned.
        </div>
      ) : (
        <div
          style={{
            padding: '8px 14px',
            fontSize: 12,
            color: 'var(--color-text-muted)',
            fontFamily: 'Geist Mono, monospace',
          }}
        >
          {rs.rowCount.toLocaleString()} row{rs.rowCount === 1 ? '' : 's'} - {rs.durationMs} ms
        </div>
      )}
    </div>
  );
}

function OutcomeView({ outcome }: { outcome: QueryOutcome }) {
  if (outcome.error && outcome.resultSets.length === 0) {
    return <CellError message={outcome.error} label="Query failed" />;
  }
  return (
    <div style={{ maxHeight: 360, overflow: 'auto' }}>
      {outcome.resultSets.map((rs, idx) => (
        <ResultSetRow key={idx} rs={rs} index={idx} total={outcome.resultSets.length} />
      ))}
      {outcome.error && <CellError message={outcome.error} label="Query failed" />}
      <div
        style={{
          padding: '4px 14px',
          fontSize: 10,
          color: 'var(--color-text-muted)',
          fontFamily: 'Geist Mono, monospace',
          background: 'var(--color-bg-elevated)',
          borderTop: '1px solid rgba(26,46,42,0.06)',
        }}
      >
        {outcome.totalDurationMs} ms total
      </div>
    </div>
  );
}
