import type { OpKind } from '../types';

function IScan() {
  return <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true"><rect x="2" y="3" width="10" height="8" rx="1.2" stroke="currentColor" strokeWidth="1.2" /><path d="M2 6h10M2 9h10" stroke="currentColor" strokeWidth="1" opacity="0.5" /></svg>;
}
function IIndexScan() {
  return <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true"><rect x="2" y="3" width="10" height="8" rx="1.2" stroke="currentColor" strokeWidth="1.2" /><path d="M4 5h6M4 7.5h6M4 10h4" stroke="currentColor" strokeWidth="1" /></svg>;
}
function IIndexSeek() {
  return <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true"><circle cx="6" cy="6" r="3.5" stroke="currentColor" strokeWidth="1.2" /><path d="M8.5 8.5l3 3" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" /></svg>;
}
function ISort() {
  return <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true"><path d="M3 4h8M3 7h5M3 10h3" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" /><path d="M12 3v8m0 0l-1.5-1.5M12 11l1.5-1.5" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" /></svg>;
}
function IHash() {
  return <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true"><path d="M5 2v10M9 2v10M2 5h10M2 9h10" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" /></svg>;
}
function INestedLoops() {
  return <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true"><circle cx="5" cy="7" r="3" stroke="currentColor" strokeWidth="1.2" /><circle cx="9" cy="7" r="3" stroke="currentColor" strokeWidth="1.2" /></svg>;
}
function IMergeJoin() {
  return <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true"><path d="M2 3l4 4-4 4M12 3l-4 4 4 4" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" fill="none" /></svg>;
}
function ICompute() {
  return <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true"><path d="M2 3h4l3 8h3" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" /><path d="M9 5l3-3" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" /></svg>;
}
function IFilter() {
  return <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true"><path d="M2 3h10l-4 5v3l-2 1V8L2 3z" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round" fill="none" /></svg>;
}
function IAggregate() {
  return <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true"><path d="M3 2h8L6.5 7 11 12H3l4-5-4-5z" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round" fill="none" /></svg>;
}
function IParallelism() {
  return <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true"><path d="M4 2v10M7 2v10M10 2v10" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" /></svg>;
}
function ISpool() {
  return <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true"><ellipse cx="7" cy="4" rx="4.5" ry="1.5" stroke="currentColor" strokeWidth="1.2" /><path d="M2.5 4v6c0 0.8 2 1.5 4.5 1.5s4.5-0.7 4.5-1.5V4" stroke="currentColor" strokeWidth="1.2" fill="none" /></svg>;
}
function IUnknown() {
  return <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true"><circle cx="7" cy="7" r="5" stroke="currentColor" strokeWidth="1.2" /></svg>;
}

export function PlanIcon({ kind }: { kind: OpKind }) {
  switch (kind) {
    case 'tableScan': return <IScan />;
    case 'indexScan': return <IIndexScan />;
    case 'indexSeek': return <IIndexSeek />;
    case 'sort': return <ISort />;
    case 'hashMatch': return <IHash />;
    case 'nestedLoops': return <INestedLoops />;
    case 'mergeJoin': return <IMergeJoin />;
    case 'computeScalar': return <ICompute />;
    case 'filter': return <IFilter />;
    case 'aggregate': return <IAggregate />;
    case 'parallelism': return <IParallelism />;
    case 'spool': return <ISpool />;
    default: return <IUnknown />;
  }
}

export function WarnIcon() {
  return (
    <svg width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden="true">
      <path d="M5 1l4 7.5H1L5 1z" stroke="currentColor" strokeWidth="1.1" strokeLinejoin="round" fill="none" />
      <path d="M5 4v2" stroke="currentColor" strokeWidth="1.1" strokeLinecap="round" />
      <circle cx="5" cy="7.3" r="0.5" fill="currentColor" />
    </svg>
  );
}
