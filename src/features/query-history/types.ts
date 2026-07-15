export interface HistoryEntry {
  id: string;
  sql: string;
  connectionId: string | null;
  executedAt: string;
  rowCount: number | null;
  durationMs: number | null;
  error: string | null;
}

export interface HistoryFilter {
  search?: string;
  connectionId?: string;
  limit?: number;
}

export interface LogHistoryInput {
  sql: string;
  connectionId: string | null;
  executedAt: string;
  rowCount: number | null;
  durationMs: number | null;
  error: string | null;
}
