import { useAi } from '../hooks/use-ai';
import { AiInput } from './AiInput';
import { AiMessageList } from './AiMessageList';

export interface AiPanelProps {
  connectionId: string | null;
  open: boolean;
  onClose: () => void;
  onInsertSql: (sql: string) => void;
  width?: number;
}

export function AiPanel({ connectionId, open, onClose, onInsertSql, width = 360 }: AiPanelProps) {
  const { messages, send, reset, isPending, sessionError } = useAi({ connectionId });

  if (!open) return null;

  return (
    <aside
      aria-label="AI query assistant"
      style={{
        width,
        flexShrink: 0,
        display: 'flex',
        flexDirection: 'column',
        borderLeft: '1px solid rgba(26,46,42,0.10)',
        background: 'var(--color-bg)',
        height: '100%',
        overflow: 'hidden',
      }}
    >
      <header
        style={{
          padding: '9px 12px',
          borderBottom: '1px solid rgba(26,46,42,0.08)',
          display: 'flex',
          alignItems: 'center',
          gap: 8,
          flexShrink: 0,
        }}
      >
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true">
          <path d="M7 1v2M7 11v2M1 7h2M11 7h2M2.8 2.8l1.4 1.4M9.8 9.8l1.4 1.4M2.8 11.2l1.4-1.4M9.8 4.2l1.4-1.4" stroke="var(--color-accent)" strokeWidth="1.3" strokeLinecap="round" />
          <circle cx="7" cy="7" r="1.5" fill="var(--color-accent)" />
        </svg>
        <strong style={{ fontSize: 12, fontWeight: 600, color: 'var(--color-text)', fontFamily: 'Geist, sans-serif' }}>
          AI Query Assistant
        </strong>
        <div style={{ flex: 1 }} />
        {messages.length > 0 && (
          <button
            type="button"
            onClick={reset}
            aria-label="Clear conversation"
            style={{
              background: 'transparent',
              border: 'none',
              padding: '3px 6px',
              cursor: 'pointer',
              fontSize: 11,
              color: 'var(--color-text-muted)',
              borderRadius: 4,
              fontFamily: 'Geist, sans-serif',
            }}
          >
            Clear
          </button>
        )}
        <button
          type="button"
          onClick={onClose}
          aria-label="Close AI panel"
          style={{
            background: 'transparent',
            border: 'none',
            padding: '3px 6px',
            cursor: 'pointer',
            color: 'var(--color-text-muted)',
            borderRadius: 4,
            display: 'flex',
            alignItems: 'center',
          }}
        >
          <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true">
            <path d="M3 3l6 6M9 3l-6 6" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
          </svg>
        </button>
      </header>
      <AiMessageList
        messages={messages}
        isPending={isPending}
        sessionError={sessionError}
        onInsertSql={onInsertSql}
      />
      <AiInput
        onSubmit={(p) => void send(p)}
        disabled={!connectionId}
        isPending={isPending}
      />
    </aside>
  );
}
