import { useState } from 'react';
import { Loader2Icon, SearchIcon } from 'lucide-react';
import { cn } from '@/shared/lib/cn';
import { useAzureSqlDatabases, type AzureSqlDatabase } from '@/features/azure-auth/api';
import { NicknameColorFields } from '@/shared/ui/color-tag';
import type { ConnectionColor } from '@/shared/types';

interface DatabasePickerProps {
  serverId: string;
  onPick: (db: AzureSqlDatabase, labels: { nickname: string | null; color: ConnectionColor | null }) => void;
  accountId?: string | null;
}

function statusBadgeClass(status: string): string {
  if (status === 'Online') return 'bg-green-100 text-green-800';
  if (status === 'Restoring' || status === 'RecoveryPending' || status === 'Recovering') {
    return 'bg-amber-100 text-amber-800';
  }
  return 'bg-red-100 text-red-800';
}

export function DatabasePicker({ serverId, onPick, accountId }: DatabasePickerProps) {
  const [search, setSearch] = useState('');
  const [nickname, setNickname] = useState('');
  const [color, setColor] = useState<ConnectionColor | null>(null);
  const { data, isLoading, isError, error } = useAzureSqlDatabases(serverId, accountId ?? undefined);

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-12 text-muted-foreground gap-2">
        <Loader2Icon className="h-4 w-4 animate-spin" aria-hidden="true" />
        <span>Loading databases…</span>
      </div>
    );
  }

  if (isError) {
    return (
      <p className="py-8 text-center text-sm text-destructive" role="alert">
        Failed to load databases: {(error as Error).message}
      </p>
    );
  }

  const filtered = (data ?? []).filter((db) =>
    db.name.toLowerCase().includes(search.toLowerCase()),
  );

  return (
    <div className="flex flex-col gap-3">
      <NicknameColorFields
        nickname={nickname}
        color={color}
        onNicknameChange={setNickname}
        onColorChange={setColor}
        compact
      />
      <div className="relative">
        <SearchIcon className="absolute left-2.5 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" aria-hidden="true" />
        <input
          type="text"
          placeholder="Filter databases…"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="pl-8 h-8 text-sm w-full border border-border rounded-md px-3 bg-background focus:outline-none"
          aria-label="Filter databases"
        />
      </div>
      {filtered.length === 0 ? (
        <p className="py-8 text-center text-sm text-muted-foreground">
          {data?.length === 0 ? 'No databases on this server.' : 'No databases match.'}
        </p>
      ) : (
        <ul className="space-y-1 max-h-[320px] overflow-y-auto" role="listbox" aria-label="Azure SQL databases">
          {filtered.map((db) => (
            <li key={db.id}>
              <button
                type="button"
                role="option"
                aria-selected="false"
                onClick={() => onPick(db, { nickname: nickname.trim() || null, color })}
                className={cn(
                  'w-full text-left rounded-md px-3 py-2.5 text-sm',
                  'hover:bg-accent focus-visible:bg-accent focus-visible:outline-none transition-colors',
                )}
              >
                <div className="flex items-center justify-between gap-2">
                  <span className="font-mono font-medium truncate">{db.name}</span>
                  <span className={cn('text-xs shrink-0 px-1.5 py-0.5 rounded', statusBadgeClass(db.status ?? 'Offline'))}>
                    {db.status ?? 'Unknown'}
                  </span>
                </div>
                <p className="mt-0.5 text-xs text-muted-foreground">
                  {db.skuTier} · {db.skuName}
                </p>
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
