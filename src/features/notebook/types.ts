export type CellKind = 'sql' | 'markdown';

export interface Cell {
  id: string;
  kind: CellKind;
  source: string;
  executionCount: number | null;
}

export interface NotebookMeta {
  title: string | null;
  connectionId: string | null;
  kernel: string;
  createdAt: string | null;
  updatedAt: string | null;
}

export interface Notebook {
  nbformat: number;
  nbformat_minor: number;
  metadata: NotebookMeta;
  cells: Cell[];
}

export interface NotebookSummary {
  id: string;
  name: string;
  path: string;
  modifiedAt: string | null;
}

export type CellRunResult =
  | { kind: 'sql'; outcome: QueryOutcome }
  | { kind: 'markdown' };

export interface ResultSet {
  columns: string[];
  rows: unknown[][];
  rowCount: number;
  durationMs: number;
}

export interface QueryOutcome {
  resultSets: ResultSet[];
  totalDurationMs: number;
  error: string | null;
}
