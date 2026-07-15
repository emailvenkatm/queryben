export interface DesignColumn {
  name: string;
  sqlType: string;
  isNullable: boolean;
  isIdentity: boolean;
  isComputed: boolean;
  computedExpression: string | null;
  defaultExpression: string | null;
  ordinal: number;
}

export interface DesignIndex {
  name: string;
  isUnique: boolean;
  columns: string[];
}

export interface DesignForeignKey {
  name: string;
  columns: string[];
  referencedSchema: string;
  referencedTable: string;
  referencedColumns: string[];
  onDelete: string | null;
  onUpdate: string | null;
}

export interface TableDesign {
  schema: string;
  name: string;
  columns: DesignColumn[];
  primaryKey: string[];
  pkName: string | null;
  indexes: DesignIndex[];
  foreignKeys: DesignForeignKey[];
}

export interface DdlStatement {
  kind: string;
  label: string;
  sql: string;
}

export interface ApplyResult {
  committed: boolean;
  rowsAffected: number;
  statementCount: number;
  durationMs: number;
  failedStatementIndex: number | null;
  errorMessage: string | null;
}
