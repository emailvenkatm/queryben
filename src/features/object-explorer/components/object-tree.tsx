import { useEffect, useMemo, useRef, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { useSchemaTree } from '../api';
import { useActiveConnectionStore } from '@/shared/stores/active-connection';
import { SchemaSection } from './schema-section';
import {
  FirewallDialog,
  FirewallToast,
  loadPreferSubnet,
  generateRuleName,
  toSubnetRange,
  type FirewallBlockedPayload,
} from '@/features/firewall/index';
import {
  commands,
  formatAppErrorForDisplay,
  isAuthFailed,
  isFirewallBlocked,
} from '@/shared/api/tauri-bindings';
import { AZURE_ACCOUNT_KEY } from '@/features/azure-auth/index';

export function ObjectTree() {
  const [filter, setFilter] = useState('');
  const activeConnectionId = useActiveConnectionStore((s) => s.activeConnectionId);
  const { data: schema, isLoading, error, refetch } = useSchemaTree(activeConnectionId);

  const [firewallDismissed, setFirewallDismissed] = useState(false);
  const [firewallToast, setFirewallToast] = useState<string | null>(null);
  const [autoSilentActive, setAutoSilentActive] = useState(false);
  const autoFireKeyRef = useRef<string | null>(null);

  const rawFirewall = useMemo<FirewallBlockedPayload | null>(() => {
    if (!error || !isFirewallBlocked(error)) return null;
    return error.message;
  }, [error]);

  const firewallPayload = firewallDismissed || autoSilentActive ? null : rawFirewall;

  useEffect(() => { setFirewallDismissed(false); }, [error]);

  useEffect(() => {
    if (!rawFirewall?.connectionId) {
      autoFireKeyRef.current = null;
      return;
    }
    const key = `${rawFirewall.connectionId}:${rawFirewall.ip}`;
    if (autoFireKeyRef.current === key) return;
    autoFireKeyRef.current = key;

    let cancelled = false;
    void (async () => {
      const { connectionId, ip } = rawFirewall;
      if (!connectionId) return;
      let canSilent = false;
      try {
        canSilent = await commands.canAddRuleSilently(connectionId);
      } catch {
        canSilent = false;
      }
      if (cancelled) return;
      if (!canSilent) return;

      setAutoSilentActive(true);
      const preferSubnet = loadPreferSubnet();
      const range = preferSubnet ? toSubnetRange(ip) : { start: ip, end: ip };
      const ruleName = generateRuleName(preferSubnet);
      const toastLabel = preferSubnet
        ? `Added ${range.start}–${range.end} to Azure firewall, retrying…`
        : `Added ${ip} to Azure firewall, retrying…`;
      setFirewallToast(toastLabel);
      try {
        await commands.addFirewallRule(connectionId, range.start, range.end, ruleName);
        if (!cancelled) void refetch();
      } catch {
        if (!cancelled) {
          setFirewallToast(null);
          setAutoSilentActive(false);
          autoFireKeyRef.current = null;
        }
      }
    })();

    return () => { cancelled = true; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [rawFirewall]);

  useEffect(() => {
    if (!firewallToast) return;
    const t = setTimeout(() => {
      setFirewallToast(null);
      setAutoSilentActive(false);
    }, 2000);
    return () => clearTimeout(t);
  }, [firewallToast]);

  const qc = useQueryClient();
  const [authSigningIn, setAuthSigningIn] = useState(false);
  const [authHint, setAuthHint] = useState<string | null>(null);
  const authFailed = !!error && isAuthFailed(error);

  const handleAuthSignIn = async (): Promise<void> => {
    setAuthHint(null);
    setAuthSigningIn(true);
    try {
      await commands.azureSignIn();
      void qc.invalidateQueries({ queryKey: AZURE_ACCOUNT_KEY });
      void qc.invalidateQueries({ queryKey: ['azure', 'has-cached-token'] });
      void refetch();
    } catch (err) {
      setAuthHint(isAuthFailed(err) ? 'Sign-in cancelled' : 'Sign-in failed, try again');
    } finally {
      setAuthSigningIn(false);
    }
  };

  const errorMsg =
    !error || isFirewallBlocked(error) || authFailed ? null : formatAppErrorForDisplay(error);

  const bodyContent = !activeConnectionId ? (
    <div style={{ display: 'flex', padding: '20px 14px', textAlign: 'center' }}>
      <p style={{ fontSize: 11, color: 'rgba(244,239,231,0.4)', flex: 1 }}>No active connection</p>
    </div>
  ) : isLoading ? (
    <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '14px 14px', color: 'rgba(244,239,231,0.4)', fontSize: 11 }}>
      <span style={{ width: 10, height: 10, borderRadius: '50%', border: '1.5px solid rgba(244,239,231,0.15)', borderTopColor: 'rgba(244,239,231,0.55)', animation: 'qb-spin 0.8s linear infinite', display: 'inline-block' }} aria-hidden="true" />
      <span>Loading schema…</span>
      <style>{`@keyframes qb-spin { to { transform: rotate(360deg); } }`}</style>
    </div>
  ) : authFailed ? (
    <div style={{ padding: '14px 14px', display: 'flex', flexDirection: 'column', gap: 8 }}>
      <span style={{ fontSize: 11, color: 'rgba(244,239,231,0.75)' }}>Sign in to Azure to load this database.</span>
      <button
        type="button"
        onClick={() => { void handleAuthSignIn(); }}
        disabled={authSigningIn}
        style={{ background: 'var(--color-accent)', color: '#fff', border: 'none', borderRadius: 6, padding: '6px 12px', fontSize: 11, fontWeight: 500, cursor: authSigningIn ? 'wait' : 'pointer', opacity: authSigningIn ? 0.7 : 1, fontFamily: 'Geist, sans-serif', alignSelf: 'flex-start' }}
      >
        {authSigningIn ? 'Opening browser…' : 'Sign in to Azure'}
      </button>
      {authHint && <span style={{ fontSize: 10, color: 'rgba(244,239,231,0.55)' }}>{authHint}</span>}
    </div>
  ) : errorMsg || !schema ? (
    <div
      role="alert"
      style={{ margin: '10px 12px', padding: '10px 12px', display: 'flex', gap: 8, alignItems: 'flex-start', background: 'rgba(213, 138, 74, 0.08)', border: '1px solid rgba(213, 138, 74, 0.28)', borderRadius: 8 }}
    >
      <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true" style={{ flexShrink: 0, marginTop: 1 }}>
        <circle cx="7" cy="7" r="6" stroke="var(--color-error)" strokeWidth="1.2" />
        <path d="M7 4v3.5" stroke="var(--color-error)" strokeWidth="1.3" strokeLinecap="round" />
        <circle cx="7" cy="9.5" r="0.7" fill="var(--color-error)" />
      </svg>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 6, flex: 1, minWidth: 0 }}>
        <span style={{ fontSize: 11, color: 'var(--color-error)', wordBreak: 'break-word', lineHeight: 1.4 }}>
          {errorMsg ?? 'Failed to load schema.'}
        </span>
        <button
          type="button"
          onClick={() => { void refetch(); }}
          style={{ alignSelf: 'flex-start', background: 'transparent', border: '1px solid rgba(244,239,231,0.18)', color: 'rgba(244,239,231,0.75)', borderRadius: 5, padding: '3px 9px', fontSize: 10, fontWeight: 500, cursor: 'pointer', fontFamily: 'Geist, sans-serif' }}
        >
          Retry
        </button>
      </div>
    </div>
  ) : null;

  return (
    <>
      <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
        <div style={{ padding: '10px 14px 8px', borderBottom: '1px solid rgba(244,239,231,0.07)' }}>
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 8 }}>
            <span style={{ fontSize: 11, fontWeight: 600, color: 'rgba(244,239,231,0.5)', textTransform: 'uppercase', letterSpacing: '0.07em' }}>
              Explorer
            </span>
            <button
              type="button"
              aria-label="Add new query"
              style={{ background: 'none', border: 'none', color: 'rgba(244,239,231,0.4)', cursor: 'pointer', padding: 2 }}
            >
              <svg width="13" height="13" viewBox="0 0 13 13" fill="none" aria-hidden="true">
                <path d="M6.5 1.5v10M1.5 6.5h10" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
              </svg>
            </button>
          </div>
          <div style={{ display: 'flex', alignItems: 'center', gap: 7, background: 'rgba(244,239,231,0.07)', border: '1px solid rgba(244,239,231,0.10)', borderRadius: 7, padding: '6px 10px' }}>
            <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true">
              <circle cx="5" cy="5" r="3.5" stroke="rgba(244,239,231,0.35)" strokeWidth="1.2" />
              <path d="M8.5 8.5l2 2" stroke="rgba(244,239,231,0.35)" strokeWidth="1.2" strokeLinecap="round" />
            </svg>
            <input
              type="text"
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
              placeholder="Filter objects…"
              aria-label="Filter schema objects"
              style={{ background: 'transparent', border: 'none', outline: 'none', fontSize: 12, color: 'rgba(244,239,231,0.8)', fontFamily: 'Geist, sans-serif', flex: 1, minWidth: 0 }}
            />
          </div>
        </div>

        <nav aria-label="Database object explorer" style={{ flex: 1, overflowY: 'auto', padding: '6px 0' }}>
          {bodyContent ?? (
            <ul style={{ listStyle: 'none', margin: 0, padding: 0 }} role="tree">
              {schema?.schemas.map((s) => (
                <SchemaSection key={s.name} schema={s} filter={filter} />
              ))}
            </ul>
          )}
        </nav>
      </div>
      <FirewallToast message={firewallToast} />
      <FirewallDialog
        payload={firewallPayload}
        onClose={() => setFirewallDismissed(true)}
        onRetry={() => refetch()}
      />
    </>
  );
}
