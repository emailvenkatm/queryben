import type { CSSProperties } from 'react';
import { btnBack, btnPrimary, stepLabel } from './wizard-styles';

interface Props {
  onOpenBrowser: () => void;
  onBack: () => void;
  pending: boolean;
}

const content: CSSProperties = { flex: 1, display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 0 };

const panelLeft: CSSProperties = {
  padding: '40px 40px 44px 52px',
  display: 'flex',
  flexDirection: 'column',
  borderRight: '1px solid rgba(60,42,34,0.07)',
};

const step = (active: boolean): CSSProperties => ({
  width: 28, height: 28, borderRadius: '50%',
  background: active ? 'var(--color-accent)' : 'var(--color-bg)',
  border: `1.5px solid ${active ? 'var(--color-accent)' : 'rgba(60,42,34,0.12)'}`,
  display: 'flex', alignItems: 'center', justifyContent: 'center',
  fontSize: 11, fontWeight: 700,
  color: active ? '#FDF6ED' : 'var(--color-text-muted)',
  flexShrink: 0,
});

export function AzureSignInStep({ onOpenBrowser, onBack, pending }: Props) {
  return (
    <div style={content}>
      <div style={panelLeft}>
        <div style={stepLabel}>Step 4 · Azure sign-in</div>
        <h2 style={{ fontSize: 22, fontWeight: 700, letterSpacing: '-0.015em', lineHeight: 1.2, color: 'var(--color-text)', marginBottom: 12, marginTop: 0 }}>
          We'll open your browser&mdash;sign in there.
        </h2>
        <p style={{ fontSize: 13.5, color: 'var(--color-text-muted)', lineHeight: 1.6, marginBottom: 28 }}>
          QueryBen hands off to your default browser. Your credentials never touch this app.
        </p>

        <div style={{ display: 'flex', flexDirection: 'column', flex: 1, gap: 0 }}>
          {[
            { n: 1, active: true, title: 'Click "Open browser to sign in"', detail: 'QueryBen opens your default browser to login.microsoftonline.com.' },
            { n: 2, active: false, title: 'Sign in with Microsoft', detail: 'MFA, conditional access, and SSO all work normally.' },
            { n: 3, active: false, title: 'Return here automatically', detail: 'The browser redirects to a local callback. Token picked up, you\'re done.' },
          ].map(({ n, active, title, detail }, i, arr) => (
            <div key={n} style={{ display: 'flex', gap: 16, alignItems: 'flex-start' }}>
              <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', flexShrink: 0, width: 28 }}>
                <div style={step(active)}>{n}</div>
                {i < arr.length - 1 && <div style={{ width: 1.5, height: 32, background: 'rgba(60,42,34,0.12)', margin: '2px 0' }} />}
              </div>
              <div style={{ paddingTop: 4, paddingBottom: 24 }}>
                <div style={{ fontSize: 13, fontWeight: 600, color: 'var(--color-text)', marginBottom: 3 }}>{title}</div>
                <div style={{ fontSize: 12, color: 'var(--color-text-muted)', lineHeight: 1.5 }}>{detail}</div>
              </div>
            </div>
          ))}
        </div>

        <div style={{ background: 'rgba(196,106,60,0.08)', border: '1px solid rgba(196,106,60,0.18)', borderRadius: 8, padding: '12px 14px', fontSize: 12, color: 'var(--color-text-muted)', lineHeight: 1.5 }}>
          <strong style={{ color: 'var(--color-text)', fontWeight: 600 }}>Multiple accounts:</strong>{' '}
          Add more from Settings &rsaquo; Accounts after setup.
        </div>
      </div>

      <div style={{ padding: '40px 40px 44px', display: 'flex', flexDirection: 'column', justifyContent: 'center', alignItems: 'center' }}>
        {/* Browser mockup */}
        <div style={{ width: '100%', maxWidth: 270, background: '#E8D8C0', border: '1px solid rgba(60,42,34,0.12)', borderRadius: 10, overflow: 'hidden', marginBottom: 24, boxShadow: '0 4px 16px rgba(42,29,23,0.09)' }} aria-hidden>
          <div style={{ background: '#DDD0BA', height: 36, display: 'flex', alignItems: 'center', padding: '0 12px', gap: 6 }}>
            {(['rgba(255,95,87,0.5)', 'rgba(255,189,46,0.5)', 'rgba(40,200,64,0.5)'] as const).map((c) => (
              <div key={c} style={{ width: 9, height: 9, borderRadius: '50%', background: c }} />
            ))}
            <div style={{ flex: 1, marginLeft: 8, height: 22, background: 'rgba(255,255,255,0.4)', borderRadius: 4, display: 'flex', alignItems: 'center', padding: '0 10px', fontSize: 10, color: 'var(--color-text-muted)', fontFamily: 'monospace' }}>
              login.microsoftonline.com
            </div>
          </div>
          <div style={{ padding: '20px 18px 22px', display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 10 }}>
            <div style={{ display: 'grid', gridTemplateColumns: '12px 12px', gap: 3 }}>
              {(['#F25022', '#7FBA00', '#00A4EF', '#FFB900'] as const).map((c) => (
                <div key={c} style={{ width: 12, height: 12, background: c, borderRadius: 1 }} />
              ))}
            </div>
            <div style={{ fontSize: 12, fontWeight: 700, color: 'var(--color-text)', textAlign: 'center' }}>Sign in to Microsoft</div>
            <div style={{ fontSize: 9.5, opacity: 0.6, color: 'var(--color-text-muted)', textAlign: 'center' }}>
              {pending ? 'Waiting for auth callback…' : 'Waiting to hand off…'}
            </div>
          </div>
        </div>

        <div style={{ width: '100%', maxWidth: 270, display: 'flex', flexDirection: 'column', gap: 10 }}>
          <button type="button" onClick={onOpenBrowser} style={{ ...btnPrimary, justifyContent: 'center', padding: '13px 24px', width: '100%' }}>
            <svg width={16} height={16} viewBox="0 0 16 16" fill="none" aria-hidden>
              <path d="M3 8c0-2.76 2.24-5 5-5h.5a4.5 4.5 0 0 0 0 9H8c-2.76 0-5-2.24-5-5z" stroke="currentColor" strokeWidth="1.3" opacity="0.6" />
              <path d="M14 4l-3 8-2-4-4-2 8-2z" stroke="currentColor" strokeWidth="1.3" strokeLinejoin="round" />
            </svg>
            Open browser to sign in
          </button>
          <button type="button" onClick={onBack} style={{ ...btnBack, alignSelf: 'flex-start' }}>
            <svg width={14} height={14} viewBox="0 0 14 14" fill="none" aria-hidden>
              <path d="M9 2L4 7l5 5" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
            Back
          </button>
          <p style={{ fontSize: 11, color: 'var(--color-text-muted)', textAlign: 'center', lineHeight: 1.5, opacity: 0.7, margin: 0 }}>
            Tokens are stored in your system keychain.
          </p>
        </div>
      </div>
    </div>
  );
}
