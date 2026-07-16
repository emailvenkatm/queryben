import type { Snippet, SnippetKernelId, SnippetProvider } from '../types';
import { MssqlSnippetProvider } from './mssql';
import { UserSnippetProvider } from './user';

export class SnippetRegistry {
  private readonly builtins = new Map<SnippetKernelId, SnippetProvider>();
  private readonly user: UserSnippetProvider;

  constructor(builtins: SnippetProvider[], user: UserSnippetProvider) {
    for (const p of builtins) this.builtins.set(p.id, p);
    this.user = user;
  }

  async listFor(kernel: SnippetKernelId): Promise<Snippet[]> {
    const builtin = this.builtins.get(kernel);
    const [core, user] = await Promise.all([
      builtin ? builtin.list() : Promise.resolve([]),
      this.user.list(),
    ]);
    const byId = new Map<string, Snippet>();
    for (const s of core) byId.set(s.id, s);
    for (const s of user) byId.set(s.id, s);
    return Array.from(byId.values());
  }
}

export const defaultRegistry = new SnippetRegistry(
  [new MssqlSnippetProvider()],
  new UserSnippetProvider(),
);
