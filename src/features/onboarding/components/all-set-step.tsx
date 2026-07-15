import type { CSSProperties } from 'react';
import { btnBack, btnPrimary } from './wizard-styles';

interface Props {
  connectionCount: number;
  signedInEmail: string | null;
  onOpenEditor: () => void;
  onBack: () => void;
}

const content: CSSProperties = { flex: 1, display: 'grid', gridTemplateColumns: '1.05fr 0.95fr' };

const panelLeft: CSSProperties = {
  padding: '44px 36px 44px 52px',
  display: 'flex', flexDirection: 'column', justifyContent: 'center',
  borderRight: '1px solid rgba(60,42,34,0.07)', position: 'relative', overflow: 'hidden',
};

const TOUR_ITEMS = [
  { title: 'Notebooks', desc: 'Mix SQL cells with markdown. Runs against any connected server.' },
  { title: 'Query plan visualizer', desc: 'Actual vs estimated rows, cost breakdown, index recommendations.' },
  { title: 'AI query assistant', desc: 'Schema-aware. Ask in plain language, get T-SQL back. Opt-in.' },
  { title: 'Snippets', desc: 'ADS saved queries are already here. Search by prefix with #.' },
];

function summary(connectionCount: number, email: string | null): string {
  const parts: string[] = [];
  if (connectionCount > 0) parts.push(connectionCount === 1 ? '1 connection imported' : `${connectionCount} connections imported`);
  if (email !== null) parts.push(`${email} signed in`);
  return parts.length > 0 ? `${parts.join(', ')}.` : '';
}

export function AllSetStep({ connectionCount, signedInEmail, onOpenEditor, onBack }: Props) {
  const summaryText = summary(connectionCount, signedInEmail);
  return (
    <div style={content}>
      <div style={panelLeft}>
        <div aria-hidden style={{ position: 'absolute', inset: 0, pointerEvents: 'none' }}>
          {[
            { w: 6, h: 6, bg: 'var(--color-accent)', op: 0.25, top: '22%', left: '15%' },
            { w: 4, h: 4, bg: 'var(--color-text)', op: 0.12, top: '35%', left: '78%' },
            { w: 8, h: 8, bg: 'var(--color-accent)', op: 0.14, top: '65%', left: '82%' },
            { w: 5, h: 5, bg: 'var(--color-text)', op: 0.1, top: '75%', left: '12%' },
          ].map((d, i) => (
            <div key={i} style={{ position: 'absolute', borderRadius: '50%', width: d.w, height: d.h, background: d.bg, opacity: d.op, top: d.top, left: d.left }} />
          ))}
        </div>

        <div style={{ width: 52, height: 52, borderRadius: '50%', background: 'linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent) 100%)', display: 'flex', alignItems: 'center', justifyContent: 'center', marginBottom: 24, position: 'relative', zIndex: 1 }}>
          <svg width={26} height={26} viewBox="0 0 26 26" fill="none" aria-hidden>
            <path d="M5 13l6 6 10-10" stroke="#FDF6ED" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
        </div>

        <h2 style={{ fontSize: 28, fontWeight: 800, letterSpacing: '-0.022em', lineHeight: 1.1, color: 'var(--color-text)', marginBottom: 12, marginTop: 0, position: 'relative', zIndex: 1 }}>
          You're in.<br />Good to go.
        </h2>
        <p style={{ fontSize: 14, color: 'var(--color-text-muted)', lineHeight: 1.6, marginBottom: 32, position: 'relative', zIndex: 1, maxWidth: 300 }}>
          {summaryText} The editor is waiting&mdash;the tour on the right is optional and always reachable from Help.
        </p>

        <button type="button" onClick={onOpenEditor} style={{ ...btnPrimary, padding: '14px 28px', fontSize: 15, fontWeight: 700, borderRadius: 9, alignSelf: 'flex-start', position: 'relative', zIndex: 1 }}>
          Open the editor
          <svg width={15} height={15} viewBox="0 0 15 15" fill="none" aria-hidden>
            <path d="M2 7.5h11M8.5 3l4 4.5-4 4.5" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
        </button>

        <button type="button" onClick={onBack} style={{ ...btnBack, alignSelf: 'flex-start', position: 'relative', zIndex: 1, marginTop: 10 }}>
          <svg width={14} height={14} viewBox="0 0 14 14" fill="none" aria-hidden>
            <path d="M9 2L4 7l5 5" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
          Back
        </button>
      </div>

      <div style={{ padding: '36px 40px 44px 36px', display: 'flex', flexDirection: 'column' }}>
        <div style={{ fontSize: 11, fontWeight: 700, letterSpacing: '0.1em', textTransform: 'uppercase', color: 'var(--color-text-muted)', opacity: 0.7, marginBottom: 14, display: 'flex', alignItems: 'center', gap: 8 }}>
          30-second tour
          <span style={{ fontSize: 10, fontWeight: 600, color: 'var(--color-text-muted)', background: 'rgba(60,42,34,0.07)', padding: '2px 8px', borderRadius: 20 }}>optional</span>
          <div style={{ flex: 1, height: 1, background: 'rgba(60,42,34,0.12)' }} />
        </div>

        <div style={{ display: 'flex', flexDirection: 'column', gap: 4, flex: 1 }}>
          {TOUR_ITEMS.map((item) => (
            <div key={item.title} style={{ display: 'flex', alignItems: 'flex-start', gap: 12, padding: '12px 14px', borderRadius: 9, border: '1px solid transparent' }}>
              <div style={{ width: 32, height: 32, borderRadius: 8, background: 'rgba(60,42,34,0.07)', flexShrink: 0, marginTop: 1 }} aria-hidden />
              <div>
                <div style={{ fontSize: 13, fontWeight: 600, color: 'var(--color-text)', marginBottom: 2 }}>{item.title}</div>
                <div style={{ fontSize: 11.5, color: 'var(--color-text-muted)', lineHeight: 1.45 }}>{item.desc}</div>
              </div>
            </div>
          ))}
        </div>

        <div style={{ marginTop: 14, fontSize: 11, color: 'var(--color-text-muted)', opacity: 0.6, lineHeight: 1.5 }}>
          This tour lives at <strong style={{ fontWeight: 600 }}>Help &rsaquo; Feature tour</strong> whenever you want it.
        </div>
      </div>
    </div>
  );
}
