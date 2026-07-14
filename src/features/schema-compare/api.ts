import { invoke } from '@/shared/api/tauri';
import type { DdlStatement, SchemaDiff, SchemaSnapshot } from './types';

export const schemaApi = {
  snapshot: (connectionId: string): Promise<SchemaSnapshot> =>
    invoke('schema_snapshot', { connectionId }),

  diff: (source: SchemaSnapshot, target: SchemaSnapshot): Promise<SchemaDiff> =>
    invoke('schema_diff', { source, target }),

  ddl: (diff: SchemaDiff): Promise<DdlStatement[]> =>
    invoke('schema_diff_ddl', { diff }),
};
