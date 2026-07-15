import type { PlanNode } from '../types';
import { PlanIcon, WarnIcon } from './plan-icons';

interface Props {
  node: PlanNode;
  maxCost: number;
}

function fmtRows(n: number | null): string {
  if (n == null) return '-';
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M rows`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K rows`;
  return `${Math.round(n)} rows`;
}

function fmtCost(n: number | null): string {
  if (n == null) return '';
  if (n < 0.001) return '< 0.001';
  return n.toFixed(3);
}

export function PlanNodeCard({ node, maxCost }: Props) {
  const costPct = node.estimatedCost != null && maxCost > 0
    ? Math.max(2, Math.min(100, (node.estimatedCost / maxCost) * 100))
    : 0;

  const hasActual = node.estimatedRows != null && node.actualRows != null && node.estimatedRows > 0;
  const skewed = hasActual && Math.abs((node.actualRows as number) - (node.estimatedRows as number)) / (node.estimatedRows as number) > 1;

  return (
    <div
      style={{
        display: 'flex', flexDirection: 'column', gap: 6,
        padding: '10px 12px',
        background: '#fff',
        border: '1px solid rgba(26,46,42,0.12)',
        borderRadius: 8,
        minWidth: 180, maxWidth: 240,
        fontSize: 12, fontFamily: 'Geist, sans-serif',
        boxShadow: '0 1px 2px rgba(26,46,42,0.04)',
      }}
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: 7, color: 'var(--color-primary, #2A5751)' }}>
        <PlanIcon kind={node.opKind} />
        <span style={{ fontWeight: 600, color: 'var(--color-text)', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis', flex: 1 }} title={node.name}>
          {node.name}
        </span>
      </div>

      {node.object && (
        <div style={{ fontFamily: 'Geist Mono, monospace', fontSize: 11, color: 'var(--color-text-muted)', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }} title={node.object}>
          {node.object}
        </div>
      )}

      <span
        style={{ fontFamily: 'Geist Mono, monospace', fontSize: 11, color: skewed ? '#C0392B' : 'var(--color-text-muted)' }}
        title={hasActual ? `est ${fmtRows(node.estimatedRows)} / actual ${fmtRows(node.actualRows)}` : 'estimated rows'}
      >
        {hasActual ? `${fmtRows(node.actualRows)} / ${fmtRows(node.estimatedRows)}` : fmtRows(node.estimatedRows)}
      </span>

      {node.estimatedCost != null && (
        <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
          <div style={{ flex: 1, height: 4, background: 'rgba(26,46,42,0.08)', borderRadius: 2, overflow: 'hidden' }} aria-label={`Cost ${fmtCost(node.estimatedCost)}`}>
            <div style={{ width: `${costPct}%`, height: '100%', background: 'var(--color-accent, #D58A4A)' }} />
          </div>
          <span style={{ fontFamily: 'Geist Mono, monospace', fontSize: 10, color: 'var(--color-text-muted)' }}>
            {fmtCost(node.estimatedCost)}
          </span>
        </div>
      )}

      {node.warnings.length > 0 && (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
          {node.warnings.map((w, i) => (
            <div key={i} style={{ display: 'flex', alignItems: 'center', gap: 5, color: '#B45309', fontSize: 11 }} title={w.message}>
              <WarnIcon />
              <span style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{w.message}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
