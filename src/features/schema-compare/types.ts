export type ObjectKind = 'table' | 'view' | 'procedure' | 'function' | 'index';

export interface ColumnSpec {
  name: string;
  sqlType: string;
  isNullable: boolean;
  isIdentity: boolean;
  isComputed: boolean;
  defaultExpression: string | null;
  ordinal: number;
}

export interface IndexSpec {
  name: string;
  isUnique: boolean;
  isPrimaryKey: boolean;
  columns: string[];
}

export interface SchemaObject {
  kind: ObjectKind;
  schema: string;
  name: string;
  qualifiedName: string;
  columns: ColumnSpec[];
  indexes: IndexSpec[];
  body: string | null;
}

export interface SchemaSnapshot {
  label: string;
  capturedAt: string;
  connectionId: string;
  engine: string;
  objects: SchemaObject[];
}

export interface ObjectChange {
  kind: ObjectKind;
  qualifiedName: string;
  source: SchemaObject | null;
  target: SchemaObject | null;
  reasons: string[];
}

export interface SchemaDiff {
  sourceLabel: string;
  targetLabel: string;
  added: ObjectChange[];
  dropped: ObjectChange[];
  changed: ObjectChange[];
  unchangedCount: number;
}

export interface DdlStatement {
  objectKind: ObjectKind;
  objectName: string;
  kind: string;
  sql: string;
}
