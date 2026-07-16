import { invoke } from '@/shared/api/tauri';
import type { Snippet, SnippetKernelId, SnippetProvider } from '../types';

export class UserSnippetProvider implements SnippetProvider {
  public readonly id: SnippetKernelId;

  constructor(id: SnippetKernelId = 'mssql') {
    this.id = id;
  }

  async list(): Promise<Snippet[]> {
    let raw: string | null;
    try { raw = await invoke<string | null>('read_user_snippets_file'); }
    catch { return []; }
    if (!raw) return [];
    try {
      const parsed: unknown = JSON.parse(raw);
      if (!Array.isArray(parsed)) return [];
      return parsed.filter(isSnippet);
    } catch { return []; }
  }
}

function isSnippet(value: unknown): value is Snippet {
  if (typeof value !== 'object' || value === null) return false;
  const v = value as Record<string, unknown>;
  return typeof v.id === 'string' && typeof v.name === 'string' && typeof v.description === 'string' && typeof v.body === 'string' && Array.isArray(v.tags);
}
