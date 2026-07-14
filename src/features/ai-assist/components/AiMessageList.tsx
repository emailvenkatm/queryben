import { useEffect, useRef } from 'react';
import type { AiMessage } from '../types';

interface Props {
  messages: AiMessage[];
  isPending: boolean;
  sessionError: string | null;
  onInsertSql: (sql: string) => void;
}

const fenceRe = /```(?:sql)?\s*\n[\s\S]*?```/i;

function AssistantBody({ message, onInsertSql }: { message: AiMessage; onInsertSql: (sql: string) => void }) {
  if (!message.sqlBlock) {
    return (
      <p style={{ margin: 0, whiteSpace: 'pre-wrap', wordBreak: 'break-word', fontSize: 13, lineHeight: 1.5 }}>
        {message.content}
      </p>
    );
  }

  const match = fenceRe.exec(message.content);
  const pre = match ? message.content.slice(0, match.index).trim() : '';
  const post = match ? message.content.slice(match.index + match[0].length).trim() : '';

  return (
    <div>
      {pre && (
        <p style={{ margin: '0 0 8px 0', whiteSpace: 'pre-wrap', wordBreak: 'break-word', fontSize: 13, lineHeight: 1.5 }}>
          {pre}
        </p>
      )}
      <pre
        style={{
          margin: 0,
          padding: '8px 10px',
          fontFamily: 'Geist Mono, monospace',
          fontSize: 12,
          lineHeight: 1.45,
          background: 'rgba(26,46,42,0.06)',
          border: '1px solid rgba(26,46,42,0.10)',
          borderRadius: 5,
          overflowX: 'auto',
          whiteSpace: 'pre',
          color: 'var(--color-text)',
        }}
      >
        {message.sqlBlock}
      </pre>
      <button
        type="button"
        onClick={() => onInsertSql(message.sqlBlock!)}
        style={{
          marginTop: 6,
          display: 'inline-flex',
          alignItems: 'center',
          gap: 5,
          background: 'var(--color-primary, #2A5751)',
          color: '#fff',
          padding: '4px 10px',
          fontSize: 11,
          fontWeight: 600,
          border: 'none',
          borderRadius: 4,
          cursor: 'pointer',
          fontFamily: 'Geist, sans-serif',
        }}
      >
        <svg width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden="true">
          <path d="M5 1v6M2.5 5.5L5 8l2.5-2.5" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
        Insert into editor
      </button>
      {post && (
        <p style={{ margin: '8px 0 0 0', whiteSpace: 'pre-wrap', wordBreak: 'break-word', fontSize: 12, lineHeight: 1.5, color: 'var(--color-text-muted)' }}>
          {post}
        </p>
      )}
    </div>
  );
}

export function AiMessageList({ messages, isPending, sessionError, onInsertSql }: Props) {
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages, isPending]);

  return (
    <div
      ref={scrollRef}
      style={{ flex: 1, overflowY: 'auto', padding: 12, display: 'flex', flexDirection: 'column', gap: 10 }}
    >
      {messages.length === 0 && !sessionError && (
        <div style={{ margin: 'auto', maxWidth: 240, textAlign: 'center', color: 'var(--color-text-muted)', fontSize: 12 }}>
          <p style={{ margin: '0 0 6px 0', fontWeight: 500, color: 'var(--color-text)' }}>Ask a data question</p>
          <p style={{ margin: 0, lineHeight: 1.5 }}>
            The assistant sees your active connection schema. Try "top 10 orders by revenue this month".
          </p>
        </div>
      )}
      {sessionError && (
        <div
          role="alert"
          style={{
            padding: '8px 10px',
            background: 'rgba(220,38,38,0.06)',
            border: '1px solid rgba(192,57,43,0.25)',
            color: 'var(--color-error, #c0392b)',
            fontSize: 12,
            borderRadius: 5,
            fontFamily: 'Geist, sans-serif',
          }}
        >
          <strong style={{ fontWeight: 600, marginRight: 4 }}>Assistant unavailable:</strong>
          {sessionError}
        </div>
      )}
      {messages.map((m) => (
        <div
          key={m.id}
          style={{
            alignSelf: m.role === 'user' ? 'flex-end' : 'flex-start',
            maxWidth: '90%',
            background: m.role === 'user' ? 'var(--color-primary, #2A5751)' : 'rgba(26,46,42,0.04)',
            color: m.role === 'user' ? '#fff' : 'var(--color-text)',
            padding: '8px 10px',
            borderRadius: 8,
            border: m.role === 'assistant' ? '1px solid rgba(26,46,42,0.08)' : 'none',
          }}
        >
          {m.role === 'user' ? (
            <p style={{ margin: 0, whiteSpace: 'pre-wrap', wordBreak: 'break-word', fontSize: 13, lineHeight: 1.5 }}>
              {m.content}
            </p>
          ) : (
            <AssistantBody message={m} onInsertSql={onInsertSql} />
          )}
        </div>
      ))}
      {isPending && (
        <div style={{ alignSelf: 'flex-start', padding: '8px 10px', fontSize: 12, color: 'var(--color-text-muted)', fontStyle: 'italic' }}>
          Assistant is thinking...
        </div>
      )}
    </div>
  );
}
