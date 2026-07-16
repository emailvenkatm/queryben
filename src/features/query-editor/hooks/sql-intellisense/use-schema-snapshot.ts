import { useMemo } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import type { SchemaInfo, TableMetadata } from '@/shared/types';

export interface SnapshotColumn {
  name: string;
  type: string;
}

export interface SnapshotTable {
  schema: string;
  name: string;
  columns: SnapshotColumn[];
}

export interface SqlSchemaSnapshot {
  tables: SnapshotTable[];
  allColumns: SnapshotColumn[];
}

const EMPTY: SqlSchemaSnapshot = { tables: [], allColumns: [] };

// TODO: import schemaKeys from @/features/object-explorer when that feature lands
const schemaKeys = {
  byConnection: (id: string) => ['schema', id] as const,
};

export function useSchemaSnapshot(connectionId: string | null): SqlSchemaSnapshot {
  const qc = useQueryClient();
  return useMemo(() => {
    if (!connectionId) return EMPTY;
    const info = qc.getQueryData<SchemaInfo>(schemaKeys.byConnection(connectionId));
    if (!info) return EMPTY;

    const metaMap = new Map<string, TableMetadata>();
    for (const entry of qc.getQueryCache().findAll()) {
      const key = entry.queryKey;
      if (!Array.isArray(key) || key[0] !== 'table-metadata') continue;
      const data = entry.state.data as TableMetadata | undefined;
      if (!data) continue;
      metaMap.set(`${data.schema}.${data.name}`, data);
    }

    const tables: SnapshotTable[] = [];
    for (const schema of info.schemas) {
      for (const t of [...schema.tables, ...schema.views]) {
        const meta = metaMap.get(`${t.schema}.${t.name}`);
        tables.push({
          schema: t.schema,
          name: t.name,
          columns: meta ? meta.columns.map((c) => ({ name: c.name, type: c.sqlType })) : [],
        });
      }
    }

    const seen = new Set<string>();
    const allColumns: SnapshotColumn[] = [];
    for (const t of tables) {
      for (const c of t.columns) {
        if (seen.has(c.name)) continue;
        seen.add(c.name);
        allColumns.push(c);
      }
    }

    return { tables, allColumns };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [connectionId]);
}
