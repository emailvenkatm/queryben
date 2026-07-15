import type { CSSProperties } from 'react';

export const overlay: CSSProperties = {
  position: 'fixed',
  inset: 0,
  background: 'var(--color-bg)',
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  zIndex: 9999,
  fontFamily: "'Geist', 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
  color: 'var(--color-text)',
  WebkitFontSmoothing: 'antialiased',
};

export const windowShell: CSSProperties = {
  width: 760,
  minHeight: 560,
  background: 'var(--color-bg-elevated)',
  borderRadius: 14,
  border: '1px solid rgba(60,42,34,0.12)',
  boxShadow: '0 1px 2px rgba(42,29,23,0.06), 0 8px 32px rgba(42,29,23,0.10)',
  display: 'flex',
  flexDirection: 'column',
  overflow: 'hidden',
  position: 'relative',
};

export const titlebar: CSSProperties = {
  height: 44,
  background: 'var(--color-bg-sidebar)',
  display: 'flex',
  alignItems: 'center',
  padding: '0 16px',
  gap: 8,
  flexShrink: 0,
};

export const trafficLight = (color: string): CSSProperties => ({
  width: 12,
  height: 12,
  borderRadius: '50%',
  opacity: 0.7,
  background: color,
});

export const titleLabel: CSSProperties = {
  marginLeft: 'auto',
  marginRight: 'auto',
  color: 'rgba(238,223,200,0.45)',
  fontSize: 12,
  letterSpacing: '0.04em',
  fontWeight: 500,
};

export const skipCorner: CSSProperties = {
  position: 'absolute',
  top: 58,
  right: 24,
  fontSize: 12,
  color: 'var(--color-text-muted)',
  opacity: 0.6,
  cursor: 'pointer',
  background: 'none',
  border: 'none',
  padding: '4px 0',
  fontFamily: 'inherit',
};

export const footer: CSSProperties = {
  borderTop: '1px solid rgba(60,42,34,0.07)',
  padding: '14px 32px',
  display: 'flex',
  justifyContent: 'space-between',
  alignItems: 'center',
  flexShrink: 0,
};

export const footerText: CSSProperties = {
  fontSize: 11,
  color: 'var(--color-text-muted)',
  opacity: 0.6,
};

export const stepLabel: CSSProperties = {
  fontSize: 11,
  fontWeight: 600,
  letterSpacing: '0.1em',
  textTransform: 'uppercase',
  color: 'var(--color-accent)',
  marginBottom: 8,
};

export const screenTitle: CSSProperties = {
  fontSize: 26,
  fontWeight: 700,
  letterSpacing: '-0.018em',
  lineHeight: 1.15,
  color: 'var(--color-text)',
  margin: 0,
};

export const screenSub: CSSProperties = {
  fontSize: 14,
  color: 'var(--color-text-muted)',
  lineHeight: 1.55,
  maxWidth: 480,
  marginTop: 8,
};

export const btnPrimary: CSSProperties = {
  display: 'inline-flex',
  alignItems: 'center',
  gap: 8,
  padding: '11px 24px',
  background: 'var(--color-accent)',
  color: '#FDF6ED',
  fontSize: 14,
  fontWeight: 600,
  borderRadius: 8,
  border: 'none',
  cursor: 'pointer',
  fontFamily: 'inherit',
};

export const btnSecondary: CSSProperties = {
  display: 'inline-flex',
  alignItems: 'center',
  padding: '11px 22px',
  fontSize: 13,
  fontWeight: 500,
  color: 'var(--color-text-muted)',
  background: 'none',
  border: '1px solid rgba(60,42,34,0.12)',
  borderRadius: 8,
  cursor: 'pointer',
  fontFamily: 'inherit',
};

export const btnBack: CSSProperties = {
  display: 'inline-flex',
  alignItems: 'center',
  gap: 6,
  fontSize: 13,
  fontWeight: 500,
  color: 'var(--color-text-muted)',
  background: 'none',
  border: 'none',
  cursor: 'pointer',
  padding: '10px 0',
  fontFamily: 'inherit',
};

export const progressBar: CSSProperties = {
  display: 'flex',
  justifyContent: 'center',
  alignItems: 'center',
  gap: 8,
  padding: '20px 0 0',
};

export const dot = (state: 'active' | 'done' | 'inactive'): CSSProperties => {
  const base: CSSProperties = {
    width: 6,
    height: 6,
    borderRadius: '50%',
    background: 'rgba(60,42,34,0.22)',
    transition: 'all 0.25s cubic-bezier(0.34,1.56,0.64,1)',
  };
  if (state === 'active') return { ...base, width: 20, borderRadius: 3, background: 'var(--color-accent)' };
  if (state === 'done') return { ...base, background: 'var(--color-accent)', opacity: 0.4 };
  return base;
};
