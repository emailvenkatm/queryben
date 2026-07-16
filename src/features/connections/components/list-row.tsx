import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { commands, isAuthFailed } from '@/shared/api/tauri-bindings';
import { useAzureAccounts } from '@/features/azure-auth/api';
import { AZURE_ACCOUNT_KEY } from '@/features/azure-auth/api';
import { ConnectionDot } from '@/shared/ui/color-tag';
import { formatRelativeTime } from '@/shared/lib/formatters';
import { connectionDisplayName, type Connection, type Environment } from '@/shared/types';

const ENV_STYLES: Record<Environment, { bg: string; color: string; border: string }> = {
  production:  { bg: 'var(--color-code-bg)', color: 'var(--color-error)',   border: '#f5c6c6' },
  staging:     { bg: 'var(--color-code-bg)', color: 'var(--color-warning)', border: '#ffe082' },
  development: { bg: 'var(--color-code-bg)', color: 'var(--color-success)', border: '#a5d6a7' },
  local:       { bg: 'var(--color-code-bg)', color: 'var(--color-success)', border: '#a5d6a7' },
};

const ENV_LABEL: Record<Environment, string> = {
  production: 'PROD',
  staging: 'STAGING',
  development: 'DEV',
  local: 'LOCAL',
};

const AUTH_LABEL: Record<string, string> = {
  sqlAuth: 'SQL Auth',
  aadPassword: 'AAD Password',
  aadInteractive: 'AAD Interactive',
  aadToken: 'AAD Token',
  aadManagedIdentity: 'Managed Identity',
};

function EnvBadge({ env }: { env: Environment | undefined }) {
  if (!env) return null;
  const s = ENV_STYLES[env] ?? ENV_STYLES.development;
  return (
    <span style={{ background: s.bg, color: s.color, border: `1px solid ${s.border}`, fontSize: 10, fontWeight: 600, padding: '1px 7px', borderRadius: 4, letterSpacing: '0.03em', flexShrink: 0, fontFamily: 'Geist Mono, monospace' }}>
      {ENV_LABEL[env] ?? env}
    </span>
  );
}

function DbIcon({ color = 'var(--color-primary)' }: { color?: string }) {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <ellipse cx="8" cy="5" rx="5" ry="2" stroke={color} strokeWidth="1.3" />
      <path d="M3 5v6c0 1.105 2.239 2 5 2s5-.895 5-2V5" stroke={color} strokeWidth="1.3" />
      <path d="M3 8c0 1.105 2.239 2 5 2s5-.895 5-2" stroke={color} strokeWidth="1.2" />
    </svg>
  );
}

function useHasCachedAzureToken(conn: Connection): { cached: boolean; loading: boolean } {
  const enabled = conn.authMode === 'aadToken';
  const query = useQuery({
    queryKey: ['azure', 'has-cached-token', conn.id],
    queryFn: () => commands.hasCachedAzureToken(conn.id),
    enabled,
    staleTime: 30_000,
    retry: false,
  });
  if (!enabled) return { cached: true, loading: false };
  if (query.data === undefined) return { cached: true, loading: query.isLoading };
  return { cached: query.data, loading: false };
}

interface ListRowProps {
  conn: Connection;
  onConnect: (conn: Connection) => void;
  onEdit: (conn: Connection) => void;
}

export function ListRow({ conn, onConnect, onEdit }: ListRowProps) {
  const isAzure = conn.server.includes('.database.windows.net') || conn.server.includes('.azuresynapse.net');
  const iconColor = conn.environment === 'production' ? 'var(--color-accent)' : 'var(--color-primary)';
  const iconBg = isAzure || conn.environment === 'production'
    ? 'color-mix(in srgb, var(--color-accent) 12%, transparent)'
    : 'color-mix(in srgb, var(--color-primary) 8%, transparent)';

  const qc = useQueryClient();
  const { cached: tokenCached, loading: probing } = useHasCachedAzureToken(conn);
  const needsSignIn = conn.authMode === 'aadToken' && !tokenCached && !probing;

  const accountsQuery = useAzureAccounts();
  const boundAccount = conn.accountId
    ? (accountsQuery.data ?? []).find((a) => a.accountId === conn.accountId)
    : undefined;
  const asLabel = boundAccount?.username ?? null;

  const [signingIn, setSigningIn] = useState(false);
  const [hint, setHint] = useState<string | null>(null);

  const buttonLabel = signingIn ? 'Opening browser…' : needsSignIn ? 'Sign in & connect' : 'Connect';

  const runConnect = async (): Promise<void> => {
    if (signingIn) return;
    setHint(null);
    if (needsSignIn) {
      setSigningIn(true);
      try {
        await commands.azureSignIn();
        qc.setQueryData(['azure', 'has-cached-token', conn.id], true);
        void qc.invalidateQueries({ queryKey: AZURE_ACCOUNT_KEY });
      } catch (err) {
        setHint(isAuthFailed(err) ? 'Sign-in cancelled' : 'Sign-in failed, try again');
        setSigningIn(false);
        return;
      }
      setSigningIn(false);
    }
    onConnect(conn);
  };

  return (
    <div
      className="group"
      role="button"
      tabIndex={0}
      onClick={() => { void runConnect(); }}
      onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); void runConnect(); } }}
      style={{ display: 'flex', alignItems: 'center', padding: '10px 8px', borderRadius: 8, cursor: 'pointer', gap: 12, transition: 'background 120ms' }}
      onMouseEnter={(e) => { (e.currentTarget as HTMLElement).style.background = 'rgba(26,46,42,0.04)'; }}
      onMouseLeave={(e) => { (e.currentTarget as HTMLElement).style.background = 'transparent'; }}
    >
      <div style={{ position: 'relative', width: 32, height: 32, background: iconBg, borderRadius: 8, display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0 }}>
        <DbIcon color={iconColor} />
        {conn.color && (
          <span style={{ position: 'absolute', top: -2, left: -2 }}>
            <ConnectionDot color={conn.color} size={10} />
          </span>
        )}
      </div>

      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <span style={{ fontSize: 13, fontWeight: conn.nickname ? 600 : 500, color: 'var(--color-text)', fontFamily: conn.nickname ? 'Geist, sans-serif' : 'Geist Mono, monospace', letterSpacing: conn.nickname ? '-0.01em' : 'normal' }}>
            {connectionDisplayName(conn)}
          </span>
          <EnvBadge env={conn.environment} />
        </div>
        {conn.nickname && (
          <div style={{ fontSize: 11, color: 'var(--color-text-muted)', marginTop: 2, fontFamily: 'Geist Mono, monospace' }}>{conn.server}</div>
        )}
        <div style={{ fontSize: 12, color: 'var(--color-text-muted)', marginTop: 2, display: 'flex', alignItems: 'center', gap: 6 }}>
          <span style={{ fontSize: 11, fontFamily: 'Geist Mono, monospace' }}>{conn.database}</span>
          <span style={{ color: 'var(--color-border)' }}>·</span>
          <span>{AUTH_LABEL[conn.authMode] ?? conn.authMode}</span>
          {asLabel && (
            <>
              <span style={{ color: 'var(--color-border)' }}>·</span>
              <span title={`Signed in as ${asLabel}`}>
                as: <span style={{ color: 'var(--color-text)', fontFamily: 'Geist Mono, monospace' }}>{asLabel}</span>
              </span>
            </>
          )}
          {needsSignIn && (
            <>
              <span style={{ color: 'var(--color-border)' }}>·</span>
              <span style={{ color: 'var(--color-primary)', fontWeight: 500 }}>Sign in required</span>
            </>
          )}
          {hint && (
            <>
              <span style={{ color: 'var(--color-border)' }}>·</span>
              <span style={{ color: 'var(--color-warning)', fontWeight: 500 }}>{hint}</span>
            </>
          )}
        </div>
      </div>

      {conn.lastUsed && (
        <div style={{ textAlign: 'right', flexShrink: 0 }}>
          <div style={{ fontSize: 12, color: 'var(--color-text-muted)' }}>{formatRelativeTime(conn.lastUsed)}</div>
        </div>
      )}

      <div className={needsSignIn ? '' : 'group-hover:opacity-100 opacity-0'} style={{ display: 'flex', gap: 4, transition: 'opacity 120ms ease' }}>
        <button
          type="button"
          onClick={(e) => { e.stopPropagation(); void runConnect(); }}
          disabled={signingIn}
          style={{ background: needsSignIn ? 'var(--color-accent)' : 'rgba(26,46,42,0.06)', color: needsSignIn ? '#fff' : 'var(--color-text)', border: 'none', borderRadius: 6, padding: '5px 10px', fontSize: 12, fontWeight: needsSignIn ? 500 : 400, cursor: signingIn ? 'wait' : 'pointer', opacity: signingIn ? 0.7 : 1, fontFamily: 'Geist, sans-serif' }}
        >
          {buttonLabel}
        </button>
        <button
          type="button"
          aria-label="Edit connection"
          onClick={(e) => { e.stopPropagation(); onEdit(conn); }}
          style={{ background: 'transparent', border: 'none', borderRadius: 6, padding: 5, cursor: 'pointer', color: 'var(--color-text-muted)' }}
        >
          <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true">
            <path d="M9.5 2.5l2 2L5 11l-3 1 1-3 6.5-6.5z" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round" fill="none" />
          </svg>
        </button>
      </div>
    </div>
  );
}
