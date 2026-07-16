import { useState } from 'react';
import { Loader2Icon, SearchIcon, UserPlusIcon } from 'lucide-react';
import { useAzureAccounts, useAzureAuth, useAzureSubscriptions, type AccountRegistryEntry, type AzureSubscription } from '@/features/azure-auth/api';

interface SubscriptionPickerProps {
  onPick: (sub: AzureSubscription) => void;
  activeAccountId: string | null;
  onSelectAccount: (accountId: string | null) => void;
}

function stateBadgeStyle(state: string): React.CSSProperties {
  if (state === 'Enabled') return { background: '#dcfce7', color: '#166534' };
  if (state === 'Warned' || state === 'PastDue') return { background: '#fef9c3', color: '#854d0e' };
  return { background: '#fee2e2', color: '#991b1b' };
}

interface AccountChipProps {
  entry: AccountRegistryEntry;
  active: boolean;
  onClick: () => void;
}

function AccountChip({ entry, active, onClick }: AccountChipProps) {
  const label = entry.displayName?.trim() || entry.username;
  return (
    <button
      type="button"
      onClick={onClick}
      aria-pressed={active}
      title={entry.username}
      style={{
        display: 'inline-flex',
        alignItems: 'center',
        gap: 6,
        borderRadius: 999,
        padding: '4px 10px',
        fontSize: 12,
        border: `1px solid ${active ? 'var(--color-accent)' : 'rgba(26,46,42,0.15)'}`,
        background: active ? 'rgba(213,138,74,0.08)' : 'transparent',
        color: active ? 'var(--color-accent)' : 'var(--color-text-muted)',
        fontWeight: active ? 500 : 400,
        cursor: 'pointer',
        maxWidth: 220,
        overflow: 'hidden',
        textOverflow: 'ellipsis',
        whiteSpace: 'nowrap',
        fontFamily: 'Geist, sans-serif',
      }}
    >
      {label}
    </button>
  );
}

interface AccountChipRowProps {
  accounts: AccountRegistryEntry[];
  activeAccountId: string | null;
  onSelectAccount: (id: string | null) => void;
  onAddAccount: () => void;
  signingIn: boolean;
}

function AccountChipRow({ accounts, activeAccountId, onSelectAccount, onAddAccount, signingIn }: AccountChipRowProps) {
  return (
    <div role="group" aria-label="Signed-in Azure accounts" style={{ display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: 6 }}>
      {accounts.map((a) => (
        <AccountChip key={a.accountId} entry={a} active={a.accountId === activeAccountId} onClick={() => onSelectAccount(a.accountId)} />
      ))}
      <button
        type="button"
        onClick={onAddAccount}
        disabled={signingIn}
        aria-label="Add another Azure account"
        style={{
          display: 'inline-flex',
          alignItems: 'center',
          gap: 4,
          borderRadius: 999,
          padding: '4px 10px',
          fontSize: 12,
          border: '1px dashed rgba(26,46,42,0.20)',
          background: 'transparent',
          color: 'var(--color-accent)',
          cursor: signingIn ? 'not-allowed' : 'pointer',
          opacity: signingIn ? 0.6 : 1,
          fontFamily: 'Geist, sans-serif',
        }}
      >
        {signingIn ? (
          <Loader2Icon className="h-3 w-3 animate-spin" aria-hidden="true" />
        ) : (
          <UserPlusIcon style={{ width: 12, height: 12 }} aria-hidden="true" />
        )}
        {signingIn ? 'Waiting for browser…' : 'Add account'}
      </button>
    </div>
  );
}

const inputStyle: React.CSSProperties = {
  width: '100%',
  padding: '6px 10px 6px 30px',
  fontSize: 13,
  background: 'var(--color-bg)',
  border: '1px solid rgba(26,46,42,0.15)',
  borderRadius: 6,
  color: 'var(--color-text)',
  outline: 'none',
  boxSizing: 'border-box',
  fontFamily: 'Geist, sans-serif',
};

export function SubscriptionPicker({ onPick, activeAccountId, onSelectAccount }: SubscriptionPickerProps) {
  const [search, setSearch] = useState('');
  const accountsQuery = useAzureAccounts();
  const accounts = accountsQuery.data ?? [];
  const { data, isLoading, isError, error } = useAzureSubscriptions(activeAccountId ?? undefined);
  const { signIn, signingIn, signInError } = useAzureAuth();

  const handleAddAccount = async (): Promise<void> => {
    try {
      const newAccount = await signIn();
      onSelectAccount(newAccount.homeAccountId);
    } catch { /* surfaced via signInError */ }
  };

  if (isLoading) {
    return (
      <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
        <AccountChipRow accounts={accounts} activeAccountId={activeAccountId} onSelectAccount={onSelectAccount} onAddAccount={() => { void handleAddAccount(); }} signingIn={signingIn} />
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 8, padding: '48px 0', color: 'var(--color-text-muted)', fontSize: 13 }}>
          <Loader2Icon style={{ width: 16, height: 16 }} aria-hidden="true" />
          Loading subscriptions…
        </div>
      </div>
    );
  }

  if (isError) {
    return (
      <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
        <AccountChipRow accounts={accounts} activeAccountId={activeAccountId} onSelectAccount={onSelectAccount} onAddAccount={() => { void handleAddAccount(); }} signingIn={signingIn} />
        <p role="alert" style={{ padding: '32px 0', textAlign: 'center', fontSize: 13, color: 'var(--color-error)' }}>
          Failed to load subscriptions: {(error as Error).message}
        </p>
      </div>
    );
  }

  const filtered = (data ?? []).filter((s) =>
    s.displayName.toLowerCase().includes(search.toLowerCase()) || s.subscriptionId.includes(search),
  );

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
      <AccountChipRow accounts={accounts} activeAccountId={activeAccountId} onSelectAccount={onSelectAccount} onAddAccount={() => { void handleAddAccount(); }} signingIn={signingIn} />

      {signInError ? (
        <p role="alert" style={{ fontSize: 12, color: 'var(--color-error)' }}>
          {(signInError as Error).message ?? 'Sign-in failed'}
        </p>
      ) : null}

      <div style={{ position: 'relative' }}>
        <SearchIcon style={{ position: 'absolute', left: 8, top: '50%', transform: 'translateY(-50%)', width: 14, height: 14, color: 'var(--color-text-muted)' }} aria-hidden="true" />
        <input type="text" placeholder="Filter subscriptions…" value={search} onChange={(e) => setSearch(e.target.value)} style={inputStyle} aria-label="Filter subscriptions" />
      </div>

      {filtered.length === 0 ? (
        <p style={{ padding: '32px 0', textAlign: 'center', fontSize: 13, color: 'var(--color-text-muted)' }}>
          {data?.length === 0 ? "This account doesn't have any Azure subscriptions available." : 'Nothing matches that search.'}
        </p>
      ) : (
        <ul role="listbox" aria-label="Azure subscriptions" style={{ listStyle: 'none', margin: 0, padding: 0, display: 'flex', flexDirection: 'column', gap: 2, maxHeight: 320, overflowY: 'auto' }}>
          {filtered.map((sub) => (
            <li key={sub.subscriptionId}>
              <button
                type="button"
                role="option"
                aria-selected="false"
                onClick={() => onPick(sub)}
                style={{ width: '100%', textAlign: 'left', padding: '9px 12px', borderRadius: 6, border: 'none', background: 'transparent', cursor: 'pointer', fontSize: 13, fontFamily: 'Geist, sans-serif' }}
                onMouseEnter={(e) => { (e.currentTarget as HTMLElement).style.background = 'rgba(26,46,42,0.04)'; }}
                onMouseLeave={(e) => { (e.currentTarget as HTMLElement).style.background = 'transparent'; }}
              >
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 8 }}>
                  <span style={{ fontWeight: 500, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', color: 'var(--color-text)' }}>{sub.displayName}</span>
                  <span style={{ ...stateBadgeStyle(sub.state), fontSize: 11, padding: '1px 6px', borderRadius: 4, flexShrink: 0 }}>{sub.state}</span>
                </div>
                <div style={{ marginTop: 2, fontSize: 11, color: 'var(--color-text-muted)', fontFamily: 'Geist Mono, monospace', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                  {sub.subscriptionId}
                </div>
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
