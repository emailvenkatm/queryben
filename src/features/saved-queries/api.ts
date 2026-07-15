import { invoke } from '@/shared/api/tauri';
import type { SavedQuery, SavedQueryFilter, SaveQueryInput } from './types';

export const savedApi = {
  list: (filter: SavedQueryFilter): Promise<SavedQuery[]> =>
    invoke('list_saved_queries', { filter }),
  save: (input: SaveQueryInput): Promise<SavedQuery> =>
    invoke('save_query', { name: input.name, folder: input.folder ?? null, sql: input.sql, connectionId: input.connectionId }),
  delete: (id: string): Promise<void> =>
    invoke('delete_saved_query', { id }),
  rename: (id: string, name: string): Promise<SavedQuery> =>
    invoke('rename_saved_query', { id, name }),
};
