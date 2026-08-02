// Cross-cutting types that mirror Rust structs. When specta regenerates
// tauri-bindings.ts, cross-check here; don't duplicate shapes.

export type AuthMode =
  | 'sqlAuth'
  | 'aadToken'
  | 'aadPassword'
  | 'aadInteractive'
  | 'aadManagedIdentity';

export type Environment = 'production' | 'staging' | 'development' | 'local';

export type ConnectionColor =
  | 'cream'
  | 'amber'
  | 'jade'
  | 'rose'
  | 'violet'
  | 'graphite';

export const CONNECTION_COLOR_HEX: Record<ConnectionColor, string> = {
  cream: '#EEDFC8',
  amber: '#C46A3C',
  jade: '#2A5751',
  rose: '#B87A7A',
  violet: '#7A6BAB',
  graphite: '#5C5651',
};

export const CONNECTION_COLORS: ConnectionColor[] = [
  'cream',
  'amber',
  'jade',
  'rose',
  'violet',
  'graphite',
];

export const NICKNAME_MAX_LEN = 40;

export interface Connection {
  id: string;
  name: string;
  server: string;
  database: string;
  port?: number;
  username?: string;
  authMode: AuthMode;
  createdAt: string;
  lastUsed?: string;
  accountId?: string | null;
  nickname?: string | null;
  color?: ConnectionColor | null;
  environment?: Environment;
}

export interface CreateConnectionInput {
  name: string;
  server: string;
  database: string;
  port?: number;
  username?: string;
  password?: string;
  authMode: AuthMode;
  nickname?: string | null;
  color?: ConnectionColor | null;
}

export interface UpdateConnectionInput {
  id: string;
  nickname?: string | null;
  color?: ConnectionColor | null;
  clearNickname?: boolean;
  clearColor?: boolean;
}

// Returns the label to show as the primary heading. Nickname wins when set.
export function connectionDisplayName(conn: Connection): string {
  const nick = (conn.nickname ?? '').trim();
  return nick.length > 0 ? nick : conn.server;
}

export type ColumnType = 'string' | 'number' | 'boolean' | 'datetime' | 'null' | 'unknown';

export interface ResultColumn {
  name: string;
  columnType: ColumnType;
  nullable: boolean;
}

export type CellValue = string | number | boolean | null;

export interface QueryResult {
  columns: ResultColumn[];
  rows: CellValue[][];
  rowCount: number;
  durationMs: number;
  truncated: boolean;
  executionTimeMs?: number;
  queryId?: string;
  affectedRows?: number;
}

export interface QueryOutcome {
  resultSets: QueryResult[];
  totalDurationMs: number;
  error: string | null;
}

export interface SchemaInfo {
  connectionId: string;
  schemas: SchemaNode[];
}

export interface SchemaNode {
  name: string;
  tables: TableInfo[];
  views: TableInfo[];
  procedures: ProcedureInfo[];
  functions: ProcedureInfo[];
}

export interface TableInfo {
  schema: string;
  name: string;
  rowCount?: number;
  columnCount?: number;
}

export interface ProcedureInfo {
  schema: string;
  name: string;
}

export interface TableColumn {
  name: string;
  sqlType: string;
  isNullable: boolean;
  isIdentity: boolean;
  isComputed: boolean;
  // rowversion / timestamp: server maintains it on every write, so any
  // client-supplied value gets rejected the same way IDENTITY does.
  isRowversion: boolean;
  // Rolled up from the flags above so the browse grid can gate cell edits on
  // a single boolean. Keeps the raw flags around so the tooltip can name the
  // exact reason a cell is read-only.
  isEditable: boolean;
  defaultExpression: string | null;
  ordinal: number;
}

export interface TableMetadata {
  schema: string;
  name: string;
  isEditable: boolean;
  primaryKey: string[];
  columns: TableColumn[];
}

export interface TransactionResult {
  committed: boolean;
  rowsAffected: number;
  statementCount: number;
  durationMs: number;
  failedStatementIndex: number | null;
  errorMessage: string | null;
}

export interface QueryTab {
  id: string;
  connectionId: string;
  title: string;
  sql: string;
  isDirty: boolean;
  createdAt: string;
  browseTable?: { schema: string; name: string };
}

export interface UiPrefs {
  editorFontSize: number;
  editorWordWrap: boolean;
  resultsMaxRows: number;
  connectionTimeoutSec: number;
  autoUpdateEnabled: boolean;
  theme: 'light';
}
