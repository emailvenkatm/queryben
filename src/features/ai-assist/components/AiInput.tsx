import { useState } from 'react';

interface Props {
  onSubmit: (prompt: string) => void;
  disabled?: boolean;
  isPending?: boolean;
}

export function AiInput({ onSubmit, disabled, isPending }: Props) {
  const [value, setValue] = useState('');

  function submit() {
    const trimmed = value.trim();
    if (!trimmed) return;
    onSubmit(trimmed);
    setValue('');
  }

  return (
    <div style={{ borderTop: '1px solid rgba(26,46,42,0.10)', padding: 10, background: 'var(--color-bg)' }}>
      <textarea
        value={value}
        disabled={disabled}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault();
            submit();
          }
        }}
        placeholder={
          disabled
            ? 'Open a connection to ask the assistant...'
            : 'Ask about your data - e.g. "customers who signed up last week"'
        }
        rows={3}
        style={{
          width: '100%',
          resize: 'vertical',
          minHeight: 56,
          maxHeight: 200,
          padding: '8px 10px',
          fontSize: 13,
          fontFamily: 'Geist, sans-serif',
          color: 'var(--color-text)',
          background: 'rgba(26,46,42,0.03)',
          border: '1px solid rgba(26,46,42,0.10)',
          borderRadius: 6,
          outline: 'none',
          boxSizing: 'border-box',
        }}
      />
      <div style={{ display: 'flex', justifyContent: 'flex-end', marginTop: 6, gap: 6, alignItems: 'center' }}>
        <span style={{ fontSize: 10, color: 'var(--color-text-muted)', marginRight: 'auto' }}>
          {isPending ? 'Thinking...' : 'Enter to send, Shift+Enter for newline'}
        </span>
        <button
          type="button"
          onClick={submit}
          disabled={disabled || isPending || !value.trim()}
          style={{
            background: disabled || isPending || !value.trim() ? 'rgba(213,138,74,0.4)' : 'var(--color-accent)',
            color: '#fff',
            padding: '5px 12px',
            fontSize: 12,
            fontWeight: 600,
            border: 'none',
            borderRadius: 5,
            cursor: disabled || isPending || !value.trim() ? 'default' : 'pointer',
            fontFamily: 'Geist, sans-serif',
          }}
        >
          Send
        </button>
      </div>
    </div>
  );
}
