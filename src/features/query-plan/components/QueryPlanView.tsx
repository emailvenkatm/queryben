import { formatAppErrorForDisplay } from '@/shared/api/errors';
import type { QueryPlan } from '../types';
import { usePlanTree } from '../hooks/use-plan-tree';
import { PlanNodeCard } from './PlanNodeCard';
import { WarnIcon } from './plan-icons';

interface Props {
  plan: QueryPlan | null;
  isLoading: boolean;
  error: unknown;
  onClose: () => void;
}

export function QueryPlanView({ plan, isLoading, error, onClose }: Props) {
  const { columns, globalMax } = usePlanTree(plan);

  return (
    <aside
      role="dialog"
      aria-label="Query execution plan"
      style={{
        position: 'absolute', top: 0, right: 0, bottom: 0,
        width: '55%', minWidth: 420, maxWidth: 900,
        background: 'var(--color-bg, #FAF7F2)',
        borderLeft: '1px solid rgba(26,46,42,0.15)',
        boxShadow: '-4px 0 20px rgba(26,46,42,0.08)',
        display: 'flex', flexDirection: 'column',
        zIndex: 50,
      }}
    >
      <header style={{ padding: '10px 14px', borderBottom: '1px solid rgba(26,46,42,0.10)', display: 'flex', alignItems: 'center', gap: 10, flexShrink: 0 }}>
        <span style={{ fontSize: 13, fontWeight: 600, fontFamily: 'Geist, sans-serif', color: 'var(--color-text)' }}>Execution plan</span>
        {plan && (
          <span style={{ fontSize: 11, padding: '2px 7px', borderRadius: 3, background: plan.isActual ? 'rgba(42,87,81,0.12)' : 'rgba(213,138,74,0.15)', color: plan.isActual ? 'var(--color-primary, #2A5751)' : '#8A5A2E', fontFamily: 'Geist Mono, monospace' }}>
            {plan.isActual ? 'actual' : 'estimated'}
          </span>
        )}
        <div style={{ flex: 1 }} />
        <button
          type="button"
          onClick={onClose}
          aria-label="Close plan"
          style={{ border: 'none', background: 'transparent', padding: 4, cursor: 'pointer', color: 'var(--color-text-muted)', display: 'flex', alignItems: 'center' }}
        >
          <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true">
            <path d="M3 3l8 8M11 3l-8 8" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
          </svg>
        </button>
      </header>

      {plan?.warnings && plan.warnings.length > 0 && (
        <div role="alert" style={{ padding: '8px 14px', background: 'rgba(213,138,74,0.10)', borderBottom: '1px solid rgba(180,83,9,0.20)', display: 'flex', flexDirection: 'column', gap: 4, flexShrink: 0 }}>
          {plan.warnings.map((w, i) => (
            <div key={i} style={{ display: 'flex', alignItems: 'center', gap: 6, color: '#8A5A2E', fontSize: 12, fontFamily: 'Geist, sans-serif' }}>
              <WarnIcon />
              <span>{w.message}</span>
            </div>
          ))}
        </div>
      )}

      <div style={{ flex: 1, overflow: 'auto', padding: 16 }}>
        {isLoading && (
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100%', color: 'var(--color-text-muted)', fontSize: 12, fontFamily: 'Geist, sans-serif' }}>
            Capturing plan...
          </div>
        )}

        {error != null && !isLoading && (
          <div role="alert" style={{ padding: 12, background: 'rgba(220,38,38,0.06)', border: '1px solid rgba(192,57,43,0.25)', borderRadius: 6, color: 'var(--color-error, #C0392B)', fontSize: 12, fontFamily: 'Geist Mono, monospace', whiteSpace: 'pre-wrap', wordBreak: 'break-word' }}>
            {formatAppErrorForDisplay(error)}
          </div>
        )}

        {plan && !isLoading && (
          <div style={{ display: 'flex', flexDirection: 'row', gap: 32, alignItems: 'stretch', minWidth: 'max-content' }}>
            {columns.map((col, ci) => (
              <div key={ci} style={{ display: 'flex', flexDirection: 'column', gap: 14, justifyContent: 'center' }}>
                {col.map((n) => <PlanNodeCard key={n.id} node={n} maxCost={globalMax} />)}
              </div>
            ))}
          </div>
        )}

        {!plan && !isLoading && error == null && (
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100%', color: 'var(--color-text-muted)', fontSize: 12, fontFamily: 'Geist, sans-serif' }}>
            Press Explain to capture the plan.
          </div>
        )}
      </div>
    </aside>
  );
}
