import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { notebookApi } from '../api';
import type { Cell, CellKind, Notebook } from '../types';

export const keys = {
  all: ['notebooks'] as const,
  list: () => [...keys.all, 'list'] as const,
  detail: (id: string) => [...keys.all, id] as const,
} as const;

export function useNotebookList() {
  return useQuery({ queryKey: keys.list(), queryFn: () => notebookApi.list() });
}

export function useNotebook(id: string | null) {
  return useQuery({
    queryKey: keys.detail(id ?? ''),
    queryFn: () => notebookApi.read(id!),
    enabled: id !== null,
  });
}

export function useSaveNotebook() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, notebook }: { id: string; notebook: Notebook }) =>
      notebookApi.write(id, notebook),
    onSuccess: (_d, { id, notebook }) => {
      qc.setQueryData(keys.detail(id), notebook);
      void qc.invalidateQueries({ queryKey: keys.list() });
    },
  });
}

export function useRenameNotebook() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, newName }: { id: string; newName: string }) =>
      notebookApi.rename(id, newName),
    onSuccess: (summary, { id }) => {
      qc.removeQueries({ queryKey: keys.detail(id) });
      void qc.invalidateQueries({ queryKey: keys.detail(summary.id) });
      void qc.invalidateQueries({ queryKey: keys.list() });
    },
  });
}

export function useRunCell() {
  return useMutation({
    mutationFn: ({ kind, source, connectionId }: {
      kind: CellKind;
      source: string;
      connectionId: string;
    }) => notebookApi.runCell(kind, source, connectionId),
  });
}

export function emptyNotebook(title = 'Untitled'): Notebook {
  return {
    nbformat: 4,
    nbformat_minor: 5,
    metadata: { title, connectionId: null, kernel: 'sql', createdAt: null, updatedAt: null },
    cells: [newCell('sql')],
  };
}

export function nextUntitledName(existing: readonly string[]): string {
  const taken = new Set(existing.map((n) => n.trim().toLowerCase()));
  for (let n = 1; n < 10_000; n++) {
    const candidate = `Untitled ${n}`;
    if (!taken.has(candidate.toLowerCase())) return candidate;
  }
  return `Untitled ${Date.now()}`;
}

export function newCell(kind: CellKind, source = ''): Cell {
  return { id: crypto.randomUUID(), kind, source, executionCount: null };
}
