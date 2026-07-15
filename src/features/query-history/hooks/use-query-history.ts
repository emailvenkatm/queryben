import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { historyApi } from '../api';
import type { HistoryFilter, LogHistoryInput } from '../types';

export const historyKeys = {
  all: ['query-history'] as const,
  list: (filter: HistoryFilter) => [...historyKeys.all, 'list', filter] as const,
} as const;

export function useHistoryList(filter: HistoryFilter = {}) {
  return useQuery({
    queryKey: historyKeys.list(filter),
    queryFn: () => historyApi.list(filter),
  });
}

export function useLogHistory() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: LogHistoryInput) => {
      const entry = { id: crypto.randomUUID(), ...input };
      return historyApi.log(entry);
    },
    onSuccess: () => { void qc.invalidateQueries({ queryKey: historyKeys.all }); },
    onError: (err) => { console.warn('[history] log failed', err); },
  });
}

export function useClearHistory() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (olderThanDays?: number | null) => historyApi.clear(olderThanDays),
    onSuccess: () => { void qc.invalidateQueries({ queryKey: historyKeys.all }); },
  });
}
