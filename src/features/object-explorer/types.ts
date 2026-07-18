export type { SchemaInfo, SchemaNode, TableInfo, ProcedureInfo } from '@/shared/types';

export type ObjectContextKind = 'table' | 'view' | 'procedure' | 'function' | 'schema';

export interface ObjectContextTarget {
  kind: ObjectContextKind;
  schema: string;
  name: string;
}

// Kept in-feature until the object-scripter surface stabilizes; then it
// moves to that feature's index and object-explorer re-exports.
export type ScriptAction =
  | 'create'
  | 'alter'
  | 'drop'
  | 'dropAndCreate'
  | 'selectTop'
  | 'insertTemplate';
