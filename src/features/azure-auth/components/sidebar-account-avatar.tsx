import { useEffect, useRef, useState } from 'react';
import { useAzureAccounts, useAzureAuth } from '../api';

function computeInitials(email: string | null | undefined, displayName: string | null | undefined): string {
  const name = (displayName ?? '').trim();
  if (name.includes(' ')) {
    const parts = name.split(/\s+/).slice(0, 2);
    return parts.map((p) => p[0]?.toUpperCase() ?? '').join('') || '?';
  }
  const local = (email ?? '').split('@')[0] ?? '';
  if (!local) return '?';
  const dotSplit = local.split(/[._-]+/).filter(Boolean);
  if (dotSplit.length >= 2 && dotSplit[0] && dotSplit[1]) {
    return ((dotSplit[0][0] ?? '') + (dotSplit[1][0] ?? '')).toUpperCase();
  }
  return local.slice(0, 2).toUpperCase();
}

function IconUser() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <circle cx="8" cy="6" r="2.6" stroke="currentColor" strokeWidth="1.3" />
      <path d="M3 13.2c0-2.4 2.3-3.9 5-3.9s5 1.5 5 3.9" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
    </svg>
  );
}

function IconCheck() {
  return (
    <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true">
      <path d="M2.5 6.2l2.3 2.3L9.5 3.8" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

function IconChevron() {
  return (
    <svg width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden="true">
      <path d="M3.5 2.5L6 5l-2.5 2.5" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

const menuBtnStyle: React.CSSProperties = {
  display: 'block',
  width: '100%',
  padding: '7px 10px',
  background: 'transparent',
  border: 'none',
  borderRadius: 5,
  color: 'var(--color-text)',
  cursor: 'pointer',
  textAlign: 'left',
  fontSize: 12.5,
  fontFamily: 'inherit',
};

export function SidebarAccountAvatar() {
  const { account, signIn, signOutAccount, signingOut } = useAzureAuth();
  const accountsQuery = useAzureAccounts();
  const accounts = accountsQuery.data ?? [];

  const [open, setOpen] = useState(false);
  const [switcherOpen, setSwitcherOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement | null>(null);
  const buttonRef = useRef<HTMLButtonElement | null>(null);

  const close = () => {
    setOpen(false);
    setSwitcherOpen(false);
  };

  useEffect(() => {
    if (!open) return;
    const onDocDown = (e: MouseEvent) => {
      if (!containerRef.current?.contains(e.target as Node)) close();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') { close(); buttonRef.current?.focus(); }
    };
    document.addEventListener('mousedown', onDocDown);
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('mousedown', onDocDown);
      document.removeEventListener('keydown', onKey);
    };
  }, [open]);

  const isSignedIn = !!account;
  const initials = isSignedIn ? computeInitials(account.username, account.name) : '';
  const email = account?.username ?? '';
  const displayLabel = isSignedIn ? `Signed in as ${email}` : 'Sign in to Azure';

  const handleAddAccount = async (): Promise<void> => {
    close();
    try { await signIn(); } catch { /* wizard surfaces error */ }
  };

  const handleSignOut = async (): Promise<void> => {
    if (!account) return;
    close();
    try { await signOutAccount(account.homeAccountId); } catch { /* keychain op */ }
  };

  const popover = open ? (
    <div
      role="menu"
      aria-label="Account menu"
      style={{ position: 'absolute', bottom: 'calc(100% + 8px)', left: 0, minWidth: 232, background: 'var(--color-bg-elevated)', border: '1px solid rgba(213,138,74,0.15)', borderRadius: 8, boxShadow: '0 8px 24px rgba(60,42,34,0.12)', padding: 6, zIndex: 60, color: 'var(--color-text)', fontSize: 12.5 }}
    >
      {isSignedIn ? (
        <>
          <div style={{ padding: '8px 10px 6px' }}>
            <div style={{ fontSize: 10.5, textTransform: 'uppercase', letterSpacing: '0.06em', color: 'var(--color-text-muted)', marginBottom: 3 }}>Signed in as</div>
            <div style={{ fontSize: 12.5, color: 'var(--color-text)', wordBreak: 'break-all', lineHeight: 1.35 }}>{email}</div>
          </div>
          <div style={{ borderTop: '1px solid var(--color-border)', margin: '4px 0' }} />
          {accounts.length > 1 && (
            <div>
              <button
                type="button"
                role="menuitem"
                aria-haspopup="menu"
                aria-expanded={switcherOpen}
                onClick={() => setSwitcherOpen((v) => !v)}
                style={{ ...menuBtnStyle, display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}
              >
                <span>Switch account</span>
                <span style={{ transform: switcherOpen ? 'rotate(90deg)' : 'none', transition: 'transform 120ms', color: 'var(--color-text-muted)' }}>
                  <IconChevron />
                </span>
              </button>
              {switcherOpen && (
                <div role="menu" aria-label="Signed-in accounts" style={{ padding: '2px 0 4px' }}>
                  {accounts.map((a) => {
                    const isActive = a.accountId === account.homeAccountId;
                    return (
                      <div
                        key={a.accountId}
                        role="menuitemradio"
                        aria-checked={isActive}
                        style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '6px 10px 6px 20px', fontSize: 12, color: isActive ? 'var(--color-text)' : 'var(--color-text-muted)' }}
                      >
                        <span style={{ width: 12, display: 'inline-flex', color: 'var(--color-accent)' }}>
                          {isActive ? <IconCheck /> : null}
                        </span>
                        <span style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                          {a.displayName ?? a.username}
                        </span>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          )}
          <button type="button" role="menuitem" onClick={() => { void handleAddAccount(); }} style={menuBtnStyle}>
            Add another account
          </button>
          <div style={{ borderTop: '1px solid var(--color-border)', margin: '4px 0' }} />
          <button
            type="button"
            role="menuitem"
            onClick={() => { void handleSignOut(); }}
            disabled={signingOut}
            style={{ ...menuBtnStyle, color: 'var(--color-error)', opacity: signingOut ? 0.6 : 1, cursor: signingOut ? 'default' : 'pointer' }}
          >
            {signingOut ? 'Signing out…' : 'Sign out'}
          </button>
        </>
      ) : (
        <button type="button" role="menuitem" onClick={() => { void handleAddAccount(); }} style={{ ...menuBtnStyle, padding: '9px 10px' }}>
          Sign in with Azure
        </button>
      )}
    </div>
  ) : null;

  return (
    <div ref={containerRef} style={{ padding: '10px 16px 12px', position: 'relative' }}>
      <button
        ref={buttonRef}
        type="button"
        onClick={() => setOpen((o) => !o)}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={displayLabel}
        title={displayLabel}
        style={{
          width: 28, height: 28, borderRadius: '50%',
          background: isSignedIn ? 'var(--color-accent)' : 'rgba(244,239,231,0.08)',
          color: isSignedIn ? 'var(--color-text-inverse)' : 'rgba(244,239,231,0.55)',
          border: isSignedIn ? 'none' : '1px solid rgba(244,239,231,0.18)',
          display: 'flex', alignItems: 'center', justifyContent: 'center',
          fontSize: 11, fontWeight: 600, letterSpacing: '0.02em',
          fontFamily: 'inherit', cursor: 'pointer', padding: 0, position: 'relative',
        }}
      >
        {isSignedIn ? initials : (
          <>
            <IconUser />
            <span
              aria-hidden="true"
              style={{ position: 'absolute', right: -1, bottom: -1, width: 10, height: 10, borderRadius: '50%', background: 'var(--color-accent)', color: 'var(--color-text-inverse)', display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: 9, fontWeight: 700, lineHeight: 1, border: '1.5px solid var(--color-bg-sidebar)' }}
            >
              +
            </span>
          </>
        )}
      </button>
      {popover}
    </div>
  );
}
