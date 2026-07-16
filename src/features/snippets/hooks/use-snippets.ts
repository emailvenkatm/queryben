import { useQuery } from '@tanstack/react-query';
import { defaultRegistry } from '../providers/registry';
import type { Snippet, SnippetKernelId } from '../types';

export const snippetKeys = {
  all: ['snippets'] as const,
  list: (kernel: SnippetKernelId) => [...snippetKeys.all, kernel] as const,
} as const;

export function useSnippets(kernel: SnippetKernelId = 'mssql') {
  return useQuery<Snippet[]>({
    queryKey: snippetKeys.list(kernel),
    queryFn: () => defaultRegistry.listFor(kernel),
    staleTime: 60_000,
  });
}
