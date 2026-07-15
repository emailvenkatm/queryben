import type { CSSProperties } from 'react';
import { btnPrimary, screenSub } from './wizard-styles';

interface Props {
  onGetStarted: () => void;
  onSkipAll: () => void;
}

const content: CSSProperties = {
  flex: 1,
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'center',
  justifyContent: 'center',
  padding: '40px 64px 48px',
  textAlign: 'center',
};

const btnLarge: CSSProperties = { ...btnPrimary, padding: '13px 32px' };

const btnSkip: CSSProperties = {
  fontSize: 13,
  color: 'var(--color-text-muted)',
  background: 'none',
  border: 'none',
  cursor: 'pointer',
  padding: '4px 0',
  fontFamily: 'inherit',
};

export function WelcomeStep({ onGetStarted, onSkipAll }: Props) {
  return (
    <>
      {/* Radial bleed — hardcoded per design-notes.md "gradient bleed" exception */}
      <div style={{ position: 'absolute', inset: 0, pointerEvents: 'none', overflow: 'hidden', borderRadius: 14 }} aria-hidden>
        <div style={{ position: 'absolute', bottom: -40, right: -40, width: 320, height: 320, background: 'radial-gradient(circle at 70% 70%, rgba(196,106,60,0.07) 0%, transparent 60%)', borderRadius: '50%' }} />
      </div>

      <div style={content}>
        <svg width={56} height={56} viewBox="0 0 56 56" fill="none" style={{ marginBottom: 28 }} aria-hidden>
          <circle cx="28" cy="28" r="26" stroke="var(--color-text)" strokeWidth="1.5" opacity="0.15" />
          <ellipse cx="28" cy="18" rx="14" ry="5" fill="none" stroke="var(--color-text)" strokeWidth="1.8" />
          <path d="M14 18 L14 38 Q14 43 28 43 Q42 43 42 38 L42 18" fill="none" stroke="var(--color-text)" strokeWidth="1.8" />
          <path d="M14 25 Q14 30 28 30 Q42 30 42 25" fill="none" stroke="var(--color-text)" strokeWidth="1.2" opacity="0.35" />
          <path d="M14 31.5 Q14 36.5 28 36.5 Q42 36.5 42 31.5" fill="none" stroke="var(--color-text)" strokeWidth="1.2" opacity="0.18" />
          <circle cx="28" cy="18" r="3" fill="var(--color-accent)" />
        </svg>

        <div style={{ fontSize: 11, fontWeight: 600, letterSpacing: '0.12em', textTransform: 'uppercase', color: 'var(--color-accent)', marginBottom: 14 }}>
          Open source · Apache 2.0
        </div>

        <h1 style={{ fontSize: 36, fontWeight: 700, lineHeight: 1.12, letterSpacing: '-0.02em', color: 'var(--color-text)', margin: 0, maxWidth: 520 }}>
          The SQL client for people
          <br />
          who miss{' '}
          <em style={{ fontStyle: 'normal', color: 'var(--color-accent)' }}>Azure Data Studio.</em>
        </h1>

        <p style={{ ...screenSub, fontSize: 15, maxWidth: 420, marginBottom: 40, textAlign: 'center' }}>
          QueryBen picks up where ADS left off&mdash;Azure SSO, firewall rules that fix themselves,
          notebooks, and a query plan you can actually read. Nothing to activate, no paid tier.
        </p>

        <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 12 }}>
          <button type="button" onClick={onGetStarted} style={btnLarge}>
            Get started
            <svg width={14} height={14} viewBox="0 0 14 14" fill="none" aria-hidden>
              <path d="M2 7h10M8 3l4 4-4 4" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
          </button>
          <button type="button" onClick={onSkipAll} style={btnSkip}>
            Skip setup and open the editor
          </button>
        </div>
      </div>
    </>
  );
}
