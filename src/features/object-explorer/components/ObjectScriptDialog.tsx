import { useEffect, useRef } from 'react';

interface Props {
  objectName: string;
  ddl: string;
  error: string | null;
  onClose: () => void;
}

export function ObjectScriptDialog({ objectName, ddl, error, onClose }: Props) {
  const dialogRef = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    dialogRef.current?.showModal();
    return () => dialogRef.current?.close();
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent): void => {
      if (e.key === 'Escape') onClose();
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [onClose]);

  return (
    <dialog
      ref={dialogRef}
      aria-label={`Script: ${objectName}`}
      onClose={onClose}
      style={{
        position: 'fixed',
        inset: 0,
        margin: 'auto',
        width: 680,
        maxWidth: '90vw',
        maxHeight: '80vh',
        borderRadius: 10,
        border: '1px solid rgba(26,46,42,0.14)',
        background: 'var(--color-bg-elevated)',
        color: 'var(--color-text)',
        boxShadow: '0 16px 48px rgba(60,42,34,0.18)',
        padding: 0,
        overflow: 'hidden',
        display: 'flex',
        flexDirection: 'column',
      }}
    >
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: '12px 16px',
          borderBottom: '1px solid rgba(26,46,42,0.10)',
          flexShrink: 0,
        }}
      >
        <span style={{ fontSize: 13, fontWeight: 600, fontFamily: 'Geist, sans-serif' }}>
          {objectName}
        </span>
        <button
          type="button"
          onClick={onClose}
          aria-label="Close"
          style={{
            background: 'none',
            border: 'none',
            cursor: 'pointer',
            color: 'var(--color-text-muted)',
            padding: 4,
            borderRadius: 4,
          }}
        >
          <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true">
            <path d="M2.5 2.5l9 9M11.5 2.5l-9 9" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
          </svg>
        </button>
      </div>

      <div style={{ flex: 1, overflow: 'auto', padding: '12px 16px' }}>
        {error ? (
          <div
            role="alert"
            style={{ color: 'var(--color-error)', fontSize: 12, fontFamily: 'Geist, sans-serif' }}
          >
            {error}
          </div>
        ) : (
          <pre
            style={{
              margin: 0,
              fontFamily: 'Geist Mono, monospace',
              fontSize: 12,
              lineHeight: 1.6,
              whiteSpace: 'pre-wrap',
              wordBreak: 'break-word',
              color: 'var(--color-text)',
            }}
          >
            {ddl}
          </pre>
        )}
      </div>
    </dialog>
  );
}
