import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { savedApi } from '../api';
import type { SavedQuery, SavedQueryFilter, SaveQueryInput } from '../types';

export const savedKeys = {
  all: ['saved-queries'] as const,
  list: (filter: SavedQueryFilter) => [...savedKeys.all, 'list', filter] as const,
} as const;

export function useSavedList(filter: SavedQueryFilter = {}) {
  return useQuery({
    queryKey: savedKeys.list(filter),
    queryFn: () => savedApi.list(filter),
  });
}

export function useSaveQuery() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: SaveQueryInput) => savedApi.save(input),
    onSuccess: () => { void qc.invalidateQueries({ queryKey: savedKeys.all }); },
  });
}

export function useDeleteQuery() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => savedApi.delete(id),
    onSuccess: () => { void qc.invalidateQueries({ queryKey: savedKeys.all }); },
  });
}

export function useRenameQuery() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, name }: { id: string; name: string }) => savedApi.rename(id, name),
    onSuccess: () => { void qc.invalidateQueries({ queryKey: savedKeys.all }); },
  });
}

export function useDuplicateQuery() {
  const save = useSaveQuery();
  return {
    ...save,
    mutateAsync: (source: SavedQuery) =>
      save.mutateAsync({ name: `${source.name} (copy)`, folder: source.folder, sql: source.sql, connectionId: source.connectionId }),
  };
}
