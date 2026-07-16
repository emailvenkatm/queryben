import { useQuery } from '@tanstack/react-query';
import { commands } from '@/shared/api/tauri-bindings';
import type { SchemaInfo } from '@/shared/types';

export const schemaKeys = {
  all: ['schema'] as const,
  byConnection: (connectionId: string) => [...schemaKeys.all, connectionId] as const,
} as const;

export function useSchemaTree(connectionId: string | null) {
  return useQuery<SchemaInfo>({
    queryKey: schemaKeys.byConnection(connectionId ?? ''),
    queryFn: () => {
      if (!connectionId) throw new Error('no connection selected');
      return commands.getSchema(connectionId);
    },
    enabled: connectionId !== null,
    staleTime: 5 * 60 * 1000,
  });
}
