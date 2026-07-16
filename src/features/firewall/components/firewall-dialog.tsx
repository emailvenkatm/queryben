import { useState } from 'react';
import { commands, isFirewallBlocked, type AppErrorPayload } from '@/shared/api/tauri-bindings';
import { useAzureAuth } from '@/features/azure-auth/index';
import { loadPreferSubnet, savePreferSubnet, generateRuleName, toSubnetRange } from '../firewall-prefs';
import { FirewallScopePicker } from './firewall-scope-picker';

export interface FirewallBlockedPayload {
  ip: string;
  server: string;
  connectionId: string | null;
}

interface FirewallDialogProps {
  payload: FirewallBlockedPayload | null;
  onClose: () => void;
  onRetry: () => void | Promise<unknown>;
}

type BusyState = 'idle' | 'signing-in' | 'adding' | 'retrying';

const JADE = 'var(--color-primary)';
const AMBER = 'var(--color-accent)';
const AMBER_HOVER = 'var(--color-accent-hover)';
const mix = (color: string, pct: number) => `color-mix(in srgb, ${color} ${pct}%, transparent)`;

function tsqlSnippet(startIp: string, endIp: string, ruleName: string): string {
  return `EXEC sp_set_firewall_rule N'${ruleName}', '${startIp}', '${endIp}';`;
}

function isUserCancelled(err: unknown): boolean {
  if (typeof err !== 'object' || err === null) return false;
  const msg =
    'message' in err && typeof (err as { message?: unknown }).message === 'string'
      ? String((err as { message: string }).message).toLowerCase()
      : err instanceof Error
        ? err.message.toLowerCase()
        : '';
  return msg.includes('cancel') || msg.includes('timed out') || msg.includes('access_denied') || msg.includes('user_cancel');
}

function isRateLimited(err: unknown): err is Extract<AppErrorPayload, { kind: 'RateLimited' }> {
  return typeof err === 'object' && err !== null && (err as { kind?: unknown }).kind === 'RateLimited';
}

function primaryLabel(busy: BusyState): string {
  if (busy === 'signing-in') return 'Waiting for browser…';
  if (busy === 'adding') return 'Adding rule…';
  if (busy === 'retrying') return 'Retrying…';
  return 'Add rule (sign in to Azure)';
}

export function FirewallDialog({ payload, onClose, onRetry }: FirewallDialogProps) {
  const [busy, setBusy] = useState<BusyState>('idle');
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [useSubnet, setUseSubnet] = useState<boolean>(() => loadPreferSubnet());
  const { isAuthenticated, signIn } = useAzureAuth();

  if (!payload) return null;

  const subnet = toSubnetRange(payload.ip);

  const handleScopeChange = (val: boolean): void => {
    setUseSubnet(val);
    savePreferSubnet(val);
  };

  const handleAddRule = async (): Promise<void> => {
    if (!payload.connectionId) return;
    setError(null);
    savePreferSubnet(useSubnet);
    try {
      if (!isAuthenticated) {
        setBusy('signing-in');
        await signIn();
      }
      setBusy('adding');
      const startIp = useSubnet ? subnet.start : payload.ip;
      const endIp = useSubnet ? subnet.end : payload.ip;
      const ruleName = generateRuleName(useSubnet);
      await commands.addFirewallRule(payload.connectionId, startIp, endIp, ruleName);
      setBusy('retrying');
      const retry = onRetry();
      if (retry) await retry;
      onClose();
    } catch (err: unknown) {
      if (isUserCancelled(err)) {
        setBusy('idle');
        return;
      }
      if (isRateLimited(err)) {
        const retryAfter = err.message.retryAfterSeconds;
        setError(retryAfter ? `Azure throttling — try again in ~${retryAfter}s.` : 'Azure throttling — try again shortly.');
        setBusy('idle');
        return;
      }
      const msg = isFirewallBlocked(err)
        ? `Still blocked after adding rule for ${err.message.ip}. Wait a minute or check the Azure portal.`
        : err instanceof Error
          ? err.message
          : typeof err === 'object' && err !== null && 'message' in err
            ? String((err as { message: unknown }).message)
            : String(err);
      setError(msg);
      setBusy('idle');
    }
  };

  const handleCopyTsql = async (): Promise<void> => {
    try {
      const startIp = useSubnet ? subnet.start : payload.ip;
      const endIp = useSubnet ? subnet.end : payload.ip;
      const ruleName = generateRuleName(useSubnet);
      await navigator.clipboard.writeText(tsqlSnippet(startIp, endIp, ruleName));
      setCopied(true);
      setTimeout(() => setCopied(false), 1800);
    } catch {
      setError('Copy failed — select the T-SQL manually and copy it.');
    }
  };

  const primaryDisabled = busy !== 'idle' || !payload.connectionId;

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="firewall-dialog-title"
      style={{ position: 'fixed', inset: 0, zIndex: 1000, display: 'flex', alignItems: 'center', justifyContent: 'center', background: 'rgba(26,46,42,0.45)', padding: 24, fontFamily: 'Geist, sans-serif' }}
      onClick={(e) => { if (e.target === e.currentTarget && busy === 'idle') onClose(); }}
    >
      <div style={{ width: 480, maxWidth: '100%', background: 'var(--color-bg)', border: `1px solid ${mix(JADE, 13)}`, borderRadius: 12, boxShadow: '0 20px 60px rgba(0,0,0,0.35)', overflow: 'hidden' }}>
        <div style={{ padding: '20px 24px 16px', borderBottom: `1px solid ${mix(JADE, 7)}` }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
            <div aria-hidden="true" style={{ width: 28, height: 28, borderRadius: 8, background: mix(AMBER, 13), color: AMBER, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
              <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
                <path d="M8 1.5 L14 4v4c0 3.5-2.4 6.4-6 6.5-3.6-.1-6-3-6-6.5V4l6-2.5z" stroke="currentColor" strokeWidth="1.4" fill="none" />
              </svg>
            </div>
            <h2 id="firewall-dialog-title" style={{ margin: 0, fontSize: 15, fontWeight: 600, color: JADE, letterSpacing: '-0.01em' }}>
              Add your IP to Azure firewall
            </h2>
          </div>
        </div>

        <div style={{ padding: '18px 24px 20px' }}>
          <p style={{ margin: 0, fontSize: 13, lineHeight: 1.55, color: JADE }}>
            Azure blocked{' '}
            <code style={{ fontFamily: 'Geist Mono, monospace', background: mix(JADE, 6), padding: '1px 6px', borderRadius: 4, fontSize: 12 }}>
              {payload.ip}
            </code>{' '}
            on{' '}
            <code style={{ fontFamily: 'Geist Mono, monospace', background: mix(JADE, 6), padding: '1px 6px', borderRadius: 4, fontSize: 12 }}>
              {payload.server}
            </code>.{' '}
            {isAuthenticated
              ? 'QueryBen will add a firewall rule for you.'
              : 'Sign in to Azure once and QueryBen will add the firewall rule.'}
          </p>

          <FirewallScopePicker
            ip={payload.ip}
            subnetStart={subnet.start}
            subnetEnd={subnet.end}
            useSubnet={useSubnet}
            disabled={busy !== 'idle'}
            onChange={handleScopeChange}
          />

          {busy === 'signing-in' && (
            <div
              role="status"
              aria-live="polite"
              style={{ marginTop: 12, padding: '10px 12px', background: mix(JADE, 4), border: `1px solid ${mix(JADE, 13)}`, borderRadius: 7, color: JADE, fontSize: 12, lineHeight: 1.5, display: 'flex', alignItems: 'center', gap: 10 }}
            >
              <span aria-hidden="true" style={{ width: 12, height: 12, borderRadius: '50%', border: `1.5px solid ${mix(JADE, 19)}`, borderTopColor: JADE, animation: 'qb-spin 0.8s linear infinite', flexShrink: 0 }} />
              <span>Waiting for you to sign in. Close the browser tab to cancel.</span>
            </div>
          )}

          {error && (
            <div role="alert" style={{ marginTop: 12, padding: '9px 12px', background: 'rgba(220,38,38,0.08)', border: '1px solid rgba(192,57,43,0.25)', borderRadius: 7, color: 'var(--color-error)', fontSize: 12, lineHeight: 1.5, fontFamily: 'Geist Mono, monospace' }}>
              {error}
            </div>
          )}

          <div style={{ marginTop: 14, display: 'flex', gap: 12, alignItems: 'center', fontSize: 12 }}>
            <button type="button" onClick={() => { void handleCopyTsql(); }} style={{ background: 'transparent', border: 'none', padding: 0, color: JADE, textDecoration: 'underline', cursor: 'pointer', fontFamily: 'Geist, sans-serif', fontSize: 12 }}>
              {copied ? 'Copied' : 'Copy T-SQL'}
            </button>
            <span aria-hidden="true" style={{ color: mix(JADE, 33) }}>·</span>
            <a href="https://portal.azure.com/#@/resource/subscriptions/" target="_blank" rel="noreferrer" style={{ color: JADE, textDecoration: 'underline', fontFamily: 'Geist, sans-serif', fontSize: 12 }}>
              Open Azure portal
            </a>
          </div>
        </div>

        <div style={{ padding: '14px 24px', borderTop: `1px solid ${mix(JADE, 7)}`, display: 'flex', justifyContent: 'flex-end', gap: 10, background: mix(JADE, 2) }}>
          <button
            type="button"
            onClick={onClose}
            disabled={busy !== 'idle' && busy !== 'signing-in'}
            style={{ background: 'transparent', border: `1px solid ${mix(JADE, 19)}`, color: JADE, padding: '7px 14px', borderRadius: 7, fontSize: 12, fontFamily: 'Geist, sans-serif', fontWeight: 500, cursor: busy !== 'idle' && busy !== 'signing-in' ? 'not-allowed' : 'pointer', opacity: busy !== 'idle' && busy !== 'signing-in' ? 0.6 : 1 }}
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={() => { void handleAddRule(); }}
            disabled={primaryDisabled}
            style={{ background: busy !== 'idle' ? AMBER_HOVER : AMBER, border: 'none', color: 'var(--color-text)', padding: '7px 16px', borderRadius: 7, fontSize: 12, fontFamily: 'Geist, sans-serif', fontWeight: 600, cursor: primaryDisabled ? 'wait' : 'pointer', opacity: primaryDisabled && busy === 'idle' ? 0.5 : 1, display: 'inline-flex', alignItems: 'center', gap: 7, minWidth: 200, justifyContent: 'center' }}
          >
            {busy !== 'idle' && (
              <span aria-hidden="true" style={{ width: 10, height: 10, borderRadius: '50%', border: '1.5px solid rgba(26,18,4,0.3)', borderTopColor: 'var(--color-text)', animation: 'qb-spin 0.8s linear infinite' }} />
            )}
            {primaryLabel(busy)}
          </button>
        </div>
      </div>
    </div>
  );
}
