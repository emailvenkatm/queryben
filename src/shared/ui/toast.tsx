import { useState, useEffect, createContext, useContext } from 'react';

type ToastVariant = 'error' | 'warning' | 'success' | 'info';

interface Toast {
  id: string;
  title: string;
  message?: string;
  variant: ToastVariant;
  duration?: number;
}

interface ToastContextValue {
  toasts: Toast[];
  show: (toast: Omit<Toast, 'id'>) => void;
  dismiss: (id: string) => void;
}

const ToastContext = createContext<ToastContextValue | null>(null);

export function useToast(): ToastContextValue {
  const ctx = useContext(ToastContext);
  if (!ctx) throw new Error('useToast must be used within ToastProvider');
  return ctx;
}

export function ToastProvider({ children }: { children: React.ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);

  const show = (toast: Omit<Toast, 'id'>): void => {
    const id = crypto.randomUUID();
    setToasts((prev) => [...prev, { ...toast, id }]);
    setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== id));
    }, toast.duration ?? 5000);
  };

  const dismiss = (id: string): void => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  };

  return (
    <ToastContext.Provider value={{ toasts, show, dismiss }}>
      {children}
      <ToastList toasts={toasts} onDismiss={dismiss} />
    </ToastContext.Provider>
  );
}

// Border/title use rgba() derived from the palette so tinted borders work in
// all themes. Info uses standard Microsoft blue — we don't have a semantic
// "info" token and users expect it.
const VARIANT_STYLES: Record<ToastVariant, { dot: string; border: string; title: string }> = {
  error:   { dot: 'var(--color-error)',   border: 'rgba(192,57,43,0.15)',  title: 'var(--color-error)' },
  warning: { dot: 'var(--color-warning)', border: 'rgba(213,138,74,0.20)', title: 'var(--color-warning)' },
  success: { dot: 'var(--color-success)', border: 'rgba(46,125,50,0.15)',  title: 'var(--color-success)' },
  info:    { dot: '#0078D4',              border: 'rgba(0,120,212,0.15)',  title: '#1565c0' },
};

function ToastList({
  toasts,
  onDismiss,
}: {
  toasts: Toast[];
  onDismiss: (id: string) => void;
}) {
  if (toasts.length === 0) return <></>;
  return (
    <div
      role="region"
      aria-label="Notifications"
      aria-live="polite"
      style={{ position: 'fixed', bottom: 32, right: 24, zIndex: 100, display: 'flex', flexDirection: 'column', gap: 10, maxWidth: 420 }}
    >
      {toasts.map((t) => (
        <ToastItem key={t.id} toast={t} onDismiss={onDismiss} />
      ))}
    </div>
  );
}

function ToastItem({
  toast,
  onDismiss,
}: {
  toast: Toast;
  onDismiss: (id: string) => void;
}) {
  const s = VARIANT_STYLES[toast.variant];
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    requestAnimationFrame(() => setVisible(true));
  }, []);

  return (
    <div
      role="alert"
      style={{
        background: 'var(--color-bg-elevated)',
        border: `1px solid ${s.border}`,
        borderRadius: 10,
        boxShadow: '0 8px 24px rgba(26,46,42,0.10)',
        padding: '14px 16px',
        display: 'flex',
        gap: 12,
        alignItems: 'flex-start',
        opacity: visible ? 1 : 0,
        transform: visible ? 'translateY(0)' : 'translateY(8px)',
        transition: 'opacity 200ms ease, transform 200ms ease',
        minWidth: 300,
      }}
    >
      <span
        style={{ width: 8, height: 8, borderRadius: '50%', background: s.dot, flexShrink: 0, marginTop: 3 }}
        aria-hidden="true"
      />
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ fontSize: 13, fontWeight: 600, color: s.title, marginBottom: toast.message ? 3 : 0 }}>
          {toast.title}
        </div>
        {toast.message && (
          <div style={{ fontSize: 12, color: 'var(--color-text-muted)', lineHeight: 1.5 }}>
            {toast.message}
          </div>
        )}
      </div>
      <button
        type="button"
        onClick={() => onDismiss(toast.id)}
        aria-label="Dismiss notification"
        style={{ background: 'none', border: 'none', color: 'var(--color-text-muted)', cursor: 'pointer', padding: 2, flexShrink: 0 }}
      >
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true">
          <path d="M1 1l10 10M11 1L1 11" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
        </svg>
      </button>
    </div>
  );
}
