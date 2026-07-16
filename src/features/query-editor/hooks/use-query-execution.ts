import { useRef } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useLogHistory } from '@/features/query-history';
import { queryApi } from '../api';
import type { QueryOutcome, QueryResult } from '@/shared/types';

const WATCHDOG_MS = 90_000;

export const resultKeys = {
  all: ['query-results'] as const,
  byTab: (tabId: string) => [...resultKeys.all, tabId] as const,
} as const;

function decorateOutcome(outcome: QueryOutcome): QueryOutcome {
  return {
    ...outcome,
    resultSets: outcome.resultSets.map((rs) => ({ ...rs, executionTimeMs: rs.durationMs })),
  };
}

export function primaryResult(outcome: QueryOutcome | undefined): QueryResult | undefined {
  return outcome?.resultSets[0];
}

interface ExecInput {
  connectionId: string;
  sql: string;
  tabId: string;
}

export function useQueryExecution() {
  const qc = useQueryClient();
  const logHistory = useLogHistory();
  const startedAt = useRef(new Map<string, number>());

  const executeMutation = useMutation({
    mutationFn: ({ connectionId, sql, tabId }: ExecInput) => {
      startedAt.current.set(tabId, Date.now());
      let watchdog: ReturnType<typeof setTimeout> | undefined;
      const watchdogPromise = new Promise<QueryOutcome>((_, reject) => {
        watchdog = setTimeout(() => {
          reject({ kind: 'Timeout', message: `Query timed out after ${WATCHDOG_MS / 1000}s — the connection may have dropped. Try again.` });
        }, WATCHDOG_MS);
      });
      const invokePromise = queryApi.execute(connectionId, sql).then(decorateOutcome);
      return Promise.race([invokePromise, watchdogPromise]).finally(() => {
        if (watchdog !== undefined) clearTimeout(watchdog);
      });
    },
    onSuccess: (data, vars) => {
      qc.setQueryData(resultKeys.byTab(vars.tabId), data);
    },
    onError: (err, vars) => {
      console.error('[execute] failed', { tabId: vars.tabId, err });
    },
    onSettled: (data, err, vars) => {
      const started = startedAt.current.get(vars.tabId);
      startedAt.current.delete(vars.tabId);
      const durationMs = started ? Date.now() - started : null;
      const rowCount = data ? data.resultSets.reduce((n, rs) => n + Number(rs.rowCount ?? 0), 0) : null;
      const errorMsg = err
        ? err instanceof Error ? err.message : typeof err === 'string' ? err : ((err as { message?: unknown })?.message !== undefined ? String((err as { message: unknown }).message) : JSON.stringify(err))
        : null;
      logHistory.mutate({ sql: vars.sql, connectionId: vars.connectionId, executedAt: new Date().toISOString(), rowCount, durationMs, error: errorMsg });
    },
  });

  const cancelMutation = useMutation({
    mutationFn: (queryId: string) => queryApi.cancel(queryId),
  });

  return {
    execute: (input: ExecInput) => executeMutation.mutateAsync(input),
    cancel: (queryId: string) => cancelMutation.mutate(queryId),
    isPending: executeMutation.isPending,
    pendingTabId: executeMutation.isPending ? (executeMutation.variables?.tabId ?? null) : null,
    error: executeMutation.error,
    activeQueryId: null as string | null,
  };
}

export function useTabResult(tabId: string) {
  return useQuery<QueryOutcome | undefined>({
    queryKey: resultKeys.byTab(tabId),
    queryFn: () => undefined,
    staleTime: Infinity,
    enabled: false,
  });
}
