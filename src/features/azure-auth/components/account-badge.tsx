import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@radix-ui/react-dropdown-menu';
import { useAzureAuth } from '../api';

export function AccountBadge() {
  const { isAuthenticated, account, signOut, signingOut } = useAzureAuth();

  if (!isAuthenticated || !account) return null;

  const displayName = account.name ?? account.username;
  const initials = displayName
    .split(' ')
    .slice(0, 2)
    .map((w) => w[0]?.toUpperCase() ?? '')
    .join('');

  const handleSignOut = async (): Promise<void> => {
    try {
      await signOut();
    } catch {
      // Keychain-only op; no user-facing recovery.
    }
  };

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          aria-label={`Signed in as ${displayName}, click for account options`}
          style={{
            width: 28,
            height: 28,
            borderRadius: '50%',
            background: 'var(--color-accent)',
            color: '#fff',
            border: 'none',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            fontSize: 11,
            fontWeight: 600,
            cursor: 'pointer',
            flexShrink: 0,
            fontFamily: 'Geist, sans-serif',
          }}
        >
          {initials}
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent
        align="end"
        sideOffset={6}
        style={{
          minWidth: 200,
          background: 'var(--color-bg-elevated)',
          border: '1px solid rgba(26,46,42,0.10)',
          borderRadius: 8,
          padding: 6,
          boxShadow: '0 8px 24px rgba(26,46,42,0.12)',
          fontSize: 12.5,
          zIndex: 60,
        }}
      >
        <DropdownMenuLabel style={{ padding: '6px 10px' }}>
          <p style={{ fontSize: 13, fontWeight: 500, margin: 0 }}>{displayName}</p>
          <p style={{ fontSize: 11, color: 'var(--color-text-muted)', margin: '2px 0 0', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
            {account.username}
          </p>
        </DropdownMenuLabel>
        <DropdownMenuSeparator style={{ borderTop: '1px solid rgba(26,46,42,0.08)', margin: '4px 0' }} />
        <DropdownMenuItem
          onClick={() => { void handleSignOut(); }}
          disabled={signingOut}
          style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '7px 10px', borderRadius: 5, fontSize: 12.5, cursor: 'pointer', color: 'var(--color-error)', outline: 'none' }}
        >
          <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true">
            <path d="M5 2H3a1 1 0 00-1 1v8a1 1 0 001 1h2M9 10l3-3-3-3M12 7H5" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
          {signingOut ? 'Signing out…' : 'Sign out'}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
