export interface FirewallToastProps {
  message: string | null;
}

export function FirewallToast({ message }: FirewallToastProps) {
  if (!message) return null;
  return (
    <div
      role="status"
      aria-live="polite"
      style={{
        position: 'fixed',
        top: 12,
        right: 16,
        zIndex: 1100,
        background: 'var(--color-primary-hover)',
        color: '#fff',
        padding: '7px 14px',
        borderRadius: 7,
        fontSize: 12,
        fontFamily: 'Geist, sans-serif',
        fontWeight: 500,
        boxShadow: '0 2px 10px rgba(26,46,42,0.2)',
        display: 'flex',
        alignItems: 'center',
        gap: 7,
      }}
    >
      <svg width="13" height="13" viewBox="0 0 13 13" fill="none" aria-hidden="true">
        <circle cx="6.5" cy="6.5" r="6" stroke="rgba(255,255,255,0.5)" strokeWidth="1" />
        <path d="M3.5 6.5l2 2 4-4" stroke="#fff" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
      </svg>
      {message}
    </div>
  );
}
