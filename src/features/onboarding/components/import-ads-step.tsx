import type { CSSProperties } from 'react';
import type { AdsDetectionSummary, AdsImportSummary } from '@/shared/api/tauri-bindings';
import { btnBack, btnPrimary, btnSecondary, screenSub, screenTitle, stepLabel } from './wizard-styles';

interface Props {
  detection: AdsDetectionSummary;
  importing: boolean;
  onImport: () => Promise<AdsImportSummary | null>;
  onSkip: () => void;
  onBack: () => void;
}

const content: CSSProperties = { flex: 1, display: 'flex', flexDirection: 'column', padding: '36px 64px 44px' };

export function ImportAdsStep({ detection, importing, onImport, onSkip, onBack }: Props) {
  const totalItems =
    detection.connectionCount +
    (detection.msalAccountEmail !== null ? 1 : 0) +
    (detection.snippetCount > 0 ? 1 : 0);
  const versionLabel = detection.version !== null
    ? `Azure Data Studio ${detection.version} detected`
    : 'Azure Data Studio detected';

  return (
    <div style={content}>
      <div style={{ marginBottom: 28 }}>
        <div style={stepLabel}>Step 2 · Import</div>
        <h2 style={screenTitle}>
          Azure Data Studio found&mdash;
          <br />bring your connections over.
        </h2>
        <p style={screenSub}>
          Found at{' '}
          <code style={{ fontFamily: 'Geist Mono, monospace', fontSize: 13, color: 'var(--color-text)' }}>
            {detection.installPath}
          </code>
          . One click and you're in.
        </p>
      </div>

      <div style={{ background: 'rgba(196,106,60,0.08)', border: '1px solid rgba(196,106,60,0.3)', borderRadius: 10, padding: '16px 20px', display: 'flex', alignItems: 'flex-start', gap: 14, marginBottom: 20 }}>
        <div style={{ width: 36, height: 36, borderRadius: 8, background: 'var(--color-accent)', display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0 }}>
          <svg width={18} height={18} viewBox="0 0 18 18" fill="none" aria-hidden>
            <rect x="2" y="3" width="14" height="12" rx="2" stroke="#FDF6ED" strokeWidth="1.4" />
            <path d="M5 7h8M5 10h5" stroke="#FDF6ED" strokeWidth="1.4" strokeLinecap="round" />
          </svg>
        </div>
        <div>
          <div style={{ fontSize: 13, fontWeight: 600, color: 'var(--color-text)', marginBottom: 3 }}>{versionLabel}</div>
          <div style={{ fontSize: 12, color: 'var(--color-text-muted)' }}>
            <strong style={{ color: 'var(--color-text)', fontWeight: 500 }}>
              {detection.connectionCount} {detection.connectionCount === 1 ? 'connection' : 'connections'}
            </strong>{' '}found
            {detection.msalAccountEmail !== null && (
              <> &middot; <strong style={{ color: 'var(--color-text)', fontWeight: 500 }}>1 MSAL session</strong> as <strong style={{ color: 'var(--color-text)', fontWeight: 500 }}>{detection.msalAccountEmail}</strong></>
            )}
            {detection.snippetCount > 0 && (
              <> &middot; <strong style={{ color: 'var(--color-text)', fontWeight: 500 }}>{detection.snippetCount} saved {detection.snippetCount === 1 ? 'query' : 'queries'}</strong></>
            )}
          </div>
        </div>
      </div>

      <div style={{ background: 'var(--color-bg)', border: '1px solid rgba(60,42,34,0.12)', borderRadius: 10, overflow: 'hidden', marginBottom: 24 }}>
        <div style={{ padding: '12px 20px', borderBottom: '1px solid rgba(60,42,34,0.12)', fontSize: 11, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase', color: 'var(--color-text-muted)', display: 'flex', justifyContent: 'space-between' }}>
          What will be imported
          <span style={{ fontSize: 11, fontWeight: 500, color: 'var(--color-accent)', letterSpacing: 0, textTransform: 'none' }}>{totalItems} items</span>
        </div>
        {detection.connectionCount > 0 && (
          <div style={{ padding: '13px 20px', display: 'flex', alignItems: 'center', gap: 14, borderBottom: '1px solid rgba(60,42,34,0.07)' }}>
            <div style={{ flex: 1 }}>
              <div style={{ fontSize: 13, fontWeight: 500, color: 'var(--color-text)', marginBottom: 2 }}>
                {detection.connectionCount === 1 ? '1 connection from ADS' : `${detection.connectionCount} connections from ADS`}
              </div>
              <div style={{ fontSize: 11.5, color: 'var(--color-text-muted)' }}>Server + database + auth mode</div>
            </div>
          </div>
        )}
        {detection.msalAccountEmail !== null && (
          <div style={{ padding: '13px 20px', display: 'flex', alignItems: 'center', gap: 14, borderBottom: '1px solid rgba(60,42,34,0.07)' }}>
            <div style={{ flex: 1 }}>
              <div style={{ fontSize: 13, fontWeight: 500, color: 'var(--color-text)', marginBottom: 2 }}>{detection.msalAccountEmail}</div>
              <div style={{ fontSize: 11.5, color: 'var(--color-text-muted)' }}>Active MSAL token &middot; no re-sign-in needed</div>
            </div>
          </div>
        )}
        {detection.snippetCount > 0 && (
          <div style={{ padding: '13px 20px', display: 'flex', alignItems: 'center', gap: 14 }}>
            <div style={{ flex: 1 }}>
              <div style={{ fontSize: 13, fontWeight: 500, color: 'var(--color-text)', marginBottom: 2 }}>
                {detection.snippetCount} saved {detection.snippetCount === 1 ? 'query' : 'queries'}
              </div>
              <div style={{ fontSize: 11.5, color: 'var(--color-text-muted)' }}>Imported as local snippets</div>
            </div>
          </div>
        )}
      </div>

      <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginTop: 'auto' }}>
        <button type="button" onClick={onBack} style={btnBack}>
          <svg width={14} height={14} viewBox="0 0 14 14" fill="none" aria-hidden>
            <path d="M9 2L4 7l5 5" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
          Back
        </button>
        <div style={{ flex: 1 }} />
        <button type="button" onClick={onSkip} style={btnSecondary} disabled={importing}>Skip this step</button>
        <button type="button" onClick={() => { void onImport(); }} style={{ ...btnPrimary, opacity: importing ? 0.6 : 1 }} disabled={importing}>
          {importing ? 'Importing…' : 'Import from Azure Data Studio'}
          {!importing && (
            <svg width={14} height={14} viewBox="0 0 14 14" fill="none" aria-hidden>
              <path d="M2 7h10M8 3l4 4-4 4" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
          )}
        </button>
      </div>
    </div>
  );
}
