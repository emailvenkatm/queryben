import { useMutation } from '@tanstack/react-query';
import { schemaApi } from '../api';
import type { DdlStatement, SchemaDiff, SchemaSnapshot } from '../types';

export function useSchemaSnapshot() {
  return useMutation({
    mutationFn: (connectionId: string) => schemaApi.snapshot(connectionId),
  });
}

export function useSchemaDiff() {
  return useMutation({
    mutationFn: ({ source, target }: { source: SchemaSnapshot; target: SchemaSnapshot }) =>
      schemaApi.diff(source, target),
  });
}

export function useSchemaDdl() {
  return useMutation({
    mutationFn: (diff: SchemaDiff) => schemaApi.ddl(diff),
  });
}

export type { DdlStatement, SchemaDiff, SchemaSnapshot };
