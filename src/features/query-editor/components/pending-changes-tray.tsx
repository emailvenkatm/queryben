import { useMemo, useRef, useState } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { invoke } from '@/shared/api/tauri';
import { usePendingChangesStore } from '@/shared/stores/pending-changes';
import { resultKeys } from '../hooks/use-query-execution';
import { StatementCard } from './statement-card';

interface PendingChangesTrayProps {
  tabId: string;
  connectionId: string;
  onCommitted: (committedCount: number) => void;
}

interface TransactionResult {
  committed: boolean;
  failedStatementIndex?: number;
  errorMessage?: string;
}

export function PendingChangesTray({ tabId, connectionId, onCommitted }: PendingChangesTrayProps) {
  const allChanges = usePendingChangesStore((s) => s.changes);
  const changes = useMemo(() => allChanges.filter((c) => c.tabId === tabId), [allChanges, tabId]);
  const unstage = usePendingChangesStore((s) => s.unstage);
  const clearForTab = usePendingChangesStore((s) => s.clearForTab);

  const [expanded, setExpanded] = useState(false);
  const [failedIndex, setFailedIndex] = useState<number | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const failedCardRef = useRef<HTMLDivElement>(null);
  const qc = useQueryClient();

  const commitMutation = useMutation({
    mutationFn: () =>
      invoke<TransactionResult>('execute_transaction', {
        connectionId,
        statements: changes.map((c) => c.sql),
      }),
    onSuccess: (result) => {
      if (result.committed) {
        setFailedIndex(null);
        setErrorMessage(null);
        const committedCount = changes.length;
        clearForTab(tabId);
        // Remove the cached result so refetch starts clean.
        qc.removeQueries({ queryKey: resultKeys.byTab(tabId) });
        setTimeout(() => { onCommitted(committedCount); }, 150);
        setExpanded(false);
      } else {
        const idx = result.failedStatementIndex ?? null;
        setFailedIndex(idx);
        setErrorMessage(result.errorMessage ?? 'One of the changes failed. Nothing was saved.');
        setExpanded(true);
        requestAnimationFrame(() => { failedCardRef.current?.scrollIntoView({ block: 'center' }); });
      }
    },
    onError: (err) => {
      setFailedIndex(null);
      setErrorMessage(err instanceof Error ? err.message : String(err));
      setExpanded(true);
    },
  });

  if (changes.length === 0) return null;

  const commitLabel = changes.length === 1 ? 'Commit 1 change' : `Commit ${changes.length} changes`;

  return (
    <div
      style={{ background: 'var(--color-bg-sidebar)', borderTop: '2px solid var(--color-accent)', flexShrink: 0, height: expanded ? 280 : 48, display: 'flex', flexDirection: 'column', overflow: 'hidden', transition: 'height 140ms ease' }}
      role="region"
      aria-label="Pending changes"
    >
      <div style={{ display: 'flex', alignItems: 'center', padding: '0 16px', gap: 12, height: 48, flexShrink: 0, borderBottom: expanded ? '1px solid rgba(244,239,231,0.08)' : 'none' }}>
        <button
          type="button"
          onClick={() => setExpanded((v) => !v)}
          aria-expanded={expanded}
          aria-controls="pending-changes-body"
          style={{ display: 'flex', alignItems: 'center', gap: 8, background: 'transparent', border: 'none', cursor: 'pointer', padding: 0, color: 'var(--color-bg)', fontFamily: 'Geist, sans-serif', fontSize: 12, fontWeight: 600 }}
        >
          <svg width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden="true" style={{ transform: expanded ? 'rotate(180deg)' : 'rotate(0deg)', transition: 'transform 120ms' }}>
            <path d="M2 3l3 3 3-3" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
          <span>SQL Preview</span>
          <span style={{ background: 'rgba(213,138,74,0.25)', border: '1px solid rgba(213,138,74,0.4)', color: 'var(--color-accent)', borderRadius: 4, padding: '1px 6px', fontSize: 11, fontFamily: 'Geist Mono, monospace', fontWeight: 600 }}>
            {changes.length} pending
          </span>
        </button>

        {errorMessage && (
          <span role="alert" style={{ fontFamily: 'Geist Mono, monospace', fontSize: 11, color: '#fca5a5', marginLeft: 8, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', maxWidth: 400 }}>
            {errorMessage}
          </span>
        )}

        <div style={{ flex: 1 }} />

        <button
          type="button"
          onClick={() => { clearForTab(tabId); setFailedIndex(null); setErrorMessage(null); setExpanded(false); }}
          disabled={commitMutation.isPending}
          style={{ display: 'inline-flex', alignItems: 'center', gap: 5, padding: '6px 12px', fontSize: 12, fontWeight: 500, borderRadius: 6, border: '1px solid rgba(220,38,38,0.2)', background: 'rgba(220,38,38,0.08)', color: 'rgba(252,165,165,0.9)', cursor: commitMutation.isPending ? 'default' : 'pointer', fontFamily: 'Geist, sans-serif' }}
        >
          Discard all
        </button>

        <button
          type="button"
          onClick={() => commitMutation.mutate()}
          disabled={commitMutation.isPending}
          style={{ display: 'inline-flex', alignItems: 'center', gap: 6, padding: '6px 14px', fontSize: 12, fontWeight: 600, borderRadius: 6, border: '1px solid var(--color-accent)', background: commitMutation.isPending ? 'rgba(213,138,74,0.5)' : 'var(--color-accent)', color: '#fff', cursor: commitMutation.isPending ? 'default' : 'pointer', fontFamily: 'Geist, sans-serif' }}
        >
          {commitMutation.isPending ? 'Committing…' : commitLabel}
        </button>
      </div>

      {expanded && (
        <div id="pending-changes-body" style={{ flex: 1, overflow: 'auto', padding: '12px 16px', display: 'flex', flexDirection: 'column', gap: 10 }}>
          {changes.map((change, idx) => {
            const isFailed = idx === failedIndex;
            return (
              <div key={change.id} ref={isFailed ? failedCardRef : undefined} style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
                {isFailed && errorMessage && (
                  <div role="alert" style={{ padding: '8px 12px', borderRadius: 5, background: 'rgba(220,38,38,0.12)', border: '1px solid rgba(220,38,38,0.30)', display: 'flex', flexDirection: 'column', gap: 4 }}>
                    <span style={{ fontFamily: 'Geist Mono, monospace', fontSize: 11, color: '#fca5a5' }}>
                      Statement #{idx + 1} failed: {errorMessage}
                    </span>
                    {/converting date/i.test(errorMessage) && (
                      <span style={{ fontFamily: 'Geist, sans-serif', fontSize: 11, color: 'rgba(252,165,165,0.7)' }}>
                        Use ISO format for dates: YYYY-MM-DD (e.g. 2025-10-12).
                      </span>
                    )}
                  </div>
                )}
                <StatementCard change={change} isFailed={isFailed} onRevert={() => unstage(change.id)} />
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
