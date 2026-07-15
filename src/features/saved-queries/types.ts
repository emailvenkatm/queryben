export interface SavedQuery {
  id: string;
  name: string;
  folder: string;
  sql: string;
  connectionId: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface SavedQueryFilter {
  search?: string;
  folder?: string;
  connectionId?: string;
}

export interface SaveQueryInput {
  name: string;
  folder?: string;
  sql: string;
  connectionId: string | null;
}
