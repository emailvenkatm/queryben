import type { Snippet, SnippetProvider } from '../types';
import bundled from '../mssql-snippets.json';

export class MssqlSnippetProvider implements SnippetProvider {
  public readonly id = 'mssql' as const;
  async list(): Promise<Snippet[]> {
    return bundled as Snippet[];
  }
}
