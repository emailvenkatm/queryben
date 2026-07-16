export type { SchemaInfo, SchemaNode, TableInfo, ProcedureInfo } from '@/shared/types';

export type ObjectContextKind = 'table' | 'view' | 'procedure' | 'function' | 'schema';

export interface ObjectContextTarget {
  kind: ObjectContextKind;
  schema: string;
  name: string;
}

// Stub — object-scripter is outside this agent's slice.
// TODO wire to object-scripter feature when ported.
export type ScriptAction =
  | 'create'
  | 'alter'
  | 'drop'
  | 'dropAndCreate'
  | 'selectTop'
  | 'insertTemplate';
