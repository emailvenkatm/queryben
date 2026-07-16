export type SnippetKernelId = 'mssql' | 'postgres' | 'mysql';

export interface Snippet {
  id: string;
  name: string;
  description: string;
  language: 'sql';
  body: string;
  scope?: string;
  tags: string[];
}

export interface SnippetProvider {
  id: SnippetKernelId;
  list(): Promise<Snippet[]>;
}
