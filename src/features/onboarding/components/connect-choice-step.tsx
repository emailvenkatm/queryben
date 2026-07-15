import type { CSSProperties } from 'react';
import { btnBack, screenSub, screenTitle, stepLabel } from './wizard-styles';

interface Props {
  onAzure: () => void;
  onSqlAuth: () => void;
  onSkip: () => void;
  onBack: () => void;
}

const content: CSSProperties = { flex: 1, display: 'flex', flexDirection: 'column', padding: '36px 52px 44px' };

const cardBase = (primary: boolean): CSSProperties => ({
  border: `1.5px solid ${primary ? 'rgba(196,106,60,0.28)' : 'rgba(60,42,34,0.11)'}`,
  borderRadius: 12,
  padding: '24px 22px',
  cursor: 'pointer',
  background: 'var(--color-bg-elevated)',
  display: 'flex',
  flexDirection: 'column',
  position: 'relative',
  overflow: 'hidden',
  textAlign: 'left',
  color: 'inherit',
  fontFamily: 'inherit',
});

const cardIcon = (primary: boolean): CSSProperties => ({
  width: 40, height: 40, borderRadius: 10,
  background: primary ? 'var(--color-accent)' : 'rgba(60,42,34,0.07)',
  display: 'flex', alignItems: 'center', justifyContent: 'center', marginBottom: 16,
});

function Check({ useAccent = false, dim = false }: { useAccent?: boolean; dim?: boolean }) {
  return (
    <svg width={12} height={12} viewBox="0 0 12 12" fill="none" aria-hidden style={{ flexShrink: 0, marginTop: 1 }}>
      <path d="M2 6l3 3 5-5" stroke={useAccent ? 'var(--color-accent)' : 'var(--color-text-muted)'} strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" opacity={dim ? 0.5 : 1} />
    </svg>
  );
}

function Arrow() {
  return (
    <svg width={13} height={13} viewBox="0 0 13 13" fill="none" aria-hidden>
      <path d="M2 6.5h9M7 2.5l4 4-4 4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

export function ConnectChoiceStep({ onAzure, onSqlAuth, onSkip, onBack }: Props) {
  return (
    <div style={content}>
      <div style={{ marginBottom: 28 }}>
        <div style={stepLabel}>Step 3 · Connection</div>
        <h2 style={screenTitle}>Connect your first database.</h2>
        <p style={screenSub}>Pick how you want to connect. You can add more from the sidebar at any time.</p>
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: 14, marginBottom: 28, flex: 1, alignContent: 'start' }}>
        <button type="button" onClick={onAzure} style={cardBase(true)} aria-label="Sign in with Azure SSO">
          <div style={cardIcon(true)}>
            <svg width={22} height={22} viewBox="0 0 22 22" fill="none" aria-hidden>
              <path d="M6.5 16H5a4 4 0 0 1 0-8h.5A5.5 5.5 0 0 1 16.5 9h.5a3.5 3.5 0 0 1 0 7H6.5" stroke="#FDF6ED" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
          </div>
          <span style={{ position: 'absolute', top: 14, right: 14, fontSize: 10, fontWeight: 700, letterSpacing: '0.06em', textTransform: 'uppercase', color: 'var(--color-accent)', background: 'rgba(196,106,60,0.12)', padding: '3px 8px', borderRadius: 20 }}>
            Recommended
          </span>
          <div style={{ fontSize: 15, fontWeight: 650, color: 'var(--color-text)', marginBottom: 8 }}>Azure SSO</div>
          <div style={{ fontSize: 12.5, color: 'var(--color-text-muted)', lineHeight: 1.55, flex: 1 }}>Sign in with your Microsoft account. Works with Entra ID, conditional access, and MFA.</div>
          <ul style={{ listStyle: 'none', marginTop: 12, padding: 0, display: 'flex', flexDirection: 'column', gap: 5 }}>
            <li style={{ display: 'flex', alignItems: 'flex-start', gap: 7, fontSize: 11.5, color: 'var(--color-text-muted)' }}><Check useAccent /> No password to store</li>
            <li style={{ display: 'flex', alignItems: 'flex-start', gap: 7, fontSize: 11.5, color: 'var(--color-text-muted)' }}><Check useAccent /> Auto firewall rule for your IP</li>
            <li style={{ display: 'flex', alignItems: 'flex-start', gap: 7, fontSize: 11.5, color: 'var(--color-text-muted)' }}><Check useAccent /> Multiple accounts</li>
          </ul>
          <div style={{ marginTop: 18, display: 'flex', alignItems: 'center', gap: 6, fontSize: 12, fontWeight: 600, color: 'var(--color-accent)' }}>
            Choose Azure SSO <Arrow />
          </div>
        </button>

        <button type="button" onClick={onSqlAuth} style={cardBase(false)} aria-label="Connect with SQL Server auth">
          <div style={cardIcon(false)}>
            <svg width={22} height={22} viewBox="0 0 22 22" fill="none" aria-hidden>
              <rect x="4" y="6" width="14" height="12" rx="2" stroke="var(--color-text-muted)" strokeWidth="1.4" />
              <path d="M8 6V5a3 3 0 0 1 6 0v1" stroke="var(--color-text-muted)" strokeWidth="1.4" strokeLinecap="round" />
              <circle cx="11" cy="12" r="1.5" fill="var(--color-text-muted)" />
            </svg>
          </div>
          <div style={{ fontSize: 15, fontWeight: 650, color: 'var(--color-text)', marginBottom: 8 }}>SQL Server auth</div>
          <div style={{ fontSize: 12.5, color: 'var(--color-text-muted)', lineHeight: 1.55, flex: 1 }}>Username and password. Good for on-prem, Docker, or local dev.</div>
          <ul style={{ listStyle: 'none', marginTop: 12, padding: 0, display: 'flex', flexDirection: 'column', gap: 5 }}>
            <li style={{ display: 'flex', alignItems: 'flex-start', gap: 7, fontSize: 11.5, color: 'var(--color-text-muted)' }}><Check /> Stored in system keychain</li>
            <li style={{ display: 'flex', alignItems: 'flex-start', gap: 7, fontSize: 11.5, color: 'var(--color-text-muted)' }}><Check /> Works offline</li>
          </ul>
          <div style={{ marginTop: 18, display: 'flex', alignItems: 'center', gap: 6, fontSize: 12, fontWeight: 600, color: 'var(--color-accent)' }}>
            Use SQL auth <Arrow />
          </div>
        </button>

        <button type="button" onClick={onSkip} style={cardBase(false)} aria-label="Skip for now">
          <div style={cardIcon(false)}>
            <svg width={22} height={22} viewBox="0 0 22 22" fill="none" aria-hidden>
              <path d="M11 4v14M4 11h14" stroke="var(--color-text-muted)" strokeWidth="1.4" strokeLinecap="round" opacity="0.4" />
              <circle cx="11" cy="11" r="7" stroke="var(--color-text-muted)" strokeWidth="1.4" opacity="0.3" />
            </svg>
          </div>
          <div style={{ fontSize: 15, fontWeight: 650, color: 'var(--color-text)', marginBottom: 8 }}>Skip for now</div>
          <div style={{ fontSize: 12.5, color: 'var(--color-text-muted)', lineHeight: 1.55, flex: 1 }}>Open the editor first. Add connections when you're ready.</div>
          <ul style={{ listStyle: 'none', marginTop: 12, padding: 0, display: 'flex', flexDirection: 'column', gap: 5 }}>
            <li style={{ display: 'flex', alignItems: 'flex-start', gap: 7, fontSize: 11.5, color: 'var(--color-text-muted)' }}><Check dim /> Explore the interface first</li>
          </ul>
          <div style={{ marginTop: 'auto', display: 'flex', alignItems: 'center', gap: 6, fontSize: 12, fontWeight: 600, color: 'var(--color-text-muted)' }}>
            Skip this step <Arrow />
          </div>
        </button>
      </div>

      <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
        <button type="button" onClick={onBack} style={btnBack}>
          <svg width={14} height={14} viewBox="0 0 14 14" fill="none" aria-hidden>
            <path d="M9 2L4 7l5 5" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
          Back
        </button>
      </div>
    </div>
  );
}
