import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { commands } from '@/shared/api/tauri-bindings';
import type { CreateConnectionInput, UpdateConnectionInput } from '@/shared/types';

export const connectionKeys = {
  all: ['connections'] as const,
  list: () => [...connectionKeys.all, 'list'] as const,
} as const;

export function useConnections() {
  return useQuery({
    queryKey: connectionKeys.list(),
    queryFn: () => commands.listConnections(),
  });
}

export function useCreateConnection() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: CreateConnectionInput) => commands.createConnection(input),
    onSuccess: () => { void qc.invalidateQueries({ queryKey: connectionKeys.list() }); },
  });
}

export function useDeleteConnection() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => commands.deleteConnection(id),
    onSuccess: () => { void qc.invalidateQueries({ queryKey: connectionKeys.list() }); },
  });
}

export function useUpdateConnection() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: UpdateConnectionInput) => commands.updateConnection(input),
    onSuccess: () => { void qc.invalidateQueries({ queryKey: connectionKeys.list() }); },
  });
}

export function useTestConnection() {
  return useMutation({
    mutationFn: (input: CreateConnectionInput) => commands.testConnection(input),
  });
}
