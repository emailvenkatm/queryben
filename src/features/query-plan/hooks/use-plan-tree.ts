import { useMemo } from 'react';
import { useMutation } from '@tanstack/react-query';
import { invoke } from '@/shared/api/tauri';
import type { PlanNode, QueryPlan } from '../types';

// Plans are one-shot: user hits Explain, we fetch, render. No caching because
// the SQL or connection could differ between clicks, and stale plans are misleading.
export function useQueryPlan() {
  const mutation = useMutation<QueryPlan, unknown, { connectionId: string; sql: string }>({
    mutationFn: ({ connectionId, sql }) =>
      invoke('get_query_plan', { connectionId, sql }),
  });

  return {
    plan: mutation.data ?? null,
    isLoading: mutation.isPending,
    error: mutation.error,
    fetchPlan: mutation.mutate,
    reset: mutation.reset,
  };
}

// Walk the tree collecting nodes column-major (depth-first left), then reverse
// so leaf columns render on the left — ADS/SSMS convention.
function collectColumns(node: PlanNode, depth: number, acc: PlanNode[][]): void {
  if (!acc[depth]) acc[depth] = [];
  acc[depth].push(node);
  for (const c of node.children) collectColumns(c, depth + 1, acc);
}

function maxCost(node: PlanNode): number {
  let m = node.estimatedCost ?? 0;
  for (const c of node.children) m = Math.max(m, maxCost(c));
  return m;
}

export function usePlanTree(plan: QueryPlan | null) {
  return useMemo(() => {
    if (!plan) return { columns: [], globalMax: 0 };
    const acc: PlanNode[][] = [];
    collectColumns(plan.root, 0, acc);
    return {
      columns: acc.reverse(),
      globalMax: maxCost(plan.root),
    };
  }, [plan]);
}
