import { useState } from 'react';
import { Loader2Icon, SearchIcon } from 'lucide-react';
import { cn } from '@/shared/lib/cn';
import { useAzureSqlServers, type AzureSqlServer } from '@/features/azure-auth/api';

interface SqlServerPickerProps {
  subscriptionId: string;
  onPick: (server: AzureSqlServer) => void;
  accountId?: string | null;
}

export function SqlServerPicker({ subscriptionId, onPick, accountId }: SqlServerPickerProps) {
  const [search, setSearch] = useState('');
  const { data, isLoading, isError, error } = useAzureSqlServers(subscriptionId, accountId ?? undefined);

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-12 text-muted-foreground gap-2">
        <Loader2Icon className="h-4 w-4 animate-spin" aria-hidden="true" />
        <span>Loading SQL servers…</span>
      </div>
    );
  }

  if (isError) {
    return (
      <p className="py-8 text-center text-sm text-destructive" role="alert">
        Failed to load SQL servers: {(error as Error).message}
      </p>
    );
  }

  const filtered = (data ?? []).filter((s) =>
    s.name.toLowerCase().includes(search.toLowerCase()) ||
    s.resourceGroup.toLowerCase().includes(search.toLowerCase()),
  );

  return (
    <div className="flex flex-col gap-3">
      <div className="relative">
        <SearchIcon className="absolute left-2.5 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" aria-hidden="true" />
        <input
          type="text"
          placeholder="Filter servers…"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="pl-8 h-8 text-sm w-full border border-border rounded-md px-3 bg-background focus:outline-none"
          aria-label="Filter SQL servers"
        />
      </div>
      {filtered.length === 0 ? (
        <p className="py-8 text-center text-sm text-muted-foreground">
          {data?.length === 0 ? 'No SQL servers in this subscription.' : 'No servers match.'}
        </p>
      ) : (
        <ul className="space-y-1 max-h-[320px] overflow-y-auto" role="listbox" aria-label="Azure SQL servers">
          {filtered.map((server) => (
            <li key={server.id}>
              <button
                type="button"
                role="option"
                aria-selected="false"
                onClick={() => onPick(server)}
                className={cn(
                  'w-full text-left rounded-md px-3 py-2.5 text-sm',
                  'hover:bg-accent focus-visible:bg-accent focus-visible:outline-none transition-colors',
                )}
              >
                <div className="flex items-center justify-between gap-2">
                  <span className="font-mono font-medium truncate">{server.name}</span>
                  {server.version ? (
                    <span className="text-xs shrink-0 border border-border rounded px-1.5 py-0.5">v{server.version}</span>
                  ) : null}
                </div>
                <p className="mt-0.5 text-xs text-muted-foreground truncate">
                  {server.resourceGroup} · {server.location}
                </p>
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
