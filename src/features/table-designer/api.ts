import { useMutation, useQuery } from '@tanstack/react-query';
import { invoke } from '@tauri-apps/api/core';
import type { ApplyResult, DdlStatement, TableDesign } from './types';

const load = (connectionId: string, schema: string, name: string): Promise<TableDesign> =>
  invoke('load_table_design', { connectionId, schema, name });

const generate = (connectionId: string, current: TableDesign | null, next: TableDesign): Promise<DdlStatement[]> =>
  invoke('generate_table_ddl', { connectionId, current, next });

const apply = (connectionId: string, statements: string[]): Promise<ApplyResult> =>
  invoke('apply_table_ddl', { connectionId, statements });

export const tableDesignerKeys = {
  all: ['table-designer'] as const,
  load: (connectionId: string, schema: string, name: string) =>
    ['table-designer', 'load', connectionId, schema, name] as const,
};

export function useLoadTableDesign(
  connectionId: string | null,
  schema: string | null,
  name: string | null,
) {
  return useQuery({
    queryKey: tableDesignerKeys.load(connectionId ?? '', schema ?? '', name ?? ''),
    queryFn: () => {
      if (!connectionId || !schema || !name) throw new Error('missing connection/schema/name');
      return load(connectionId, schema, name);
    },
    enabled: Boolean(connectionId && schema && name),
    staleTime: 30_000,
  });
}

export function useGenerateTableDdl() {
  return useMutation({
    mutationFn: ({ connectionId, current, next }: { connectionId: string; current: TableDesign | null; next: TableDesign }) =>
      generate(connectionId, current, next),
  });
}

export function useApplyTableDdl() {
  return useMutation({
    mutationFn: ({ connectionId, statements }: { connectionId: string; statements: string[] }) =>
      apply(connectionId, statements),
  });
}
