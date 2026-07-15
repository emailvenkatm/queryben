import { invoke } from '@/shared/api/tauri';
import type { HistoryEntry, HistoryFilter } from './types';

export const historyApi = {
  list: (filter: HistoryFilter): Promise<HistoryEntry[]> =>
    invoke('list_query_history', { filter }),
  log: (entry: HistoryEntry): Promise<void> =>
    invoke('log_query_history', { entry }),
  clear: (olderThanDays?: number | null): Promise<number> =>
    invoke('clear_query_history', { olderThanDays: olderThanDays ?? null }),
};
