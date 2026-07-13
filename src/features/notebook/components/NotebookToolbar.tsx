import { useEffect, useRef, useState } from 'react';

interface Connection {
  id: string;
  name: string;
}

interface Props {
  title: string;
  onRename: (t: string) => void | Promise<void>;
  renameBusy: boolean;
  connections: Connection[];
  connectionId: string | null;
  onConnectionChange: (id: string | null) => void;
  onSave: () => void;
  isSaving: boolean;
  dirty: boolean;
  saveToast: string | null;
}

export function NotebookToolbar({
  title,
  onRename,
  renameBusy,
  connections,
  connectionId,
  onConnectionChange,
  onSave,
  isSaving,
  dirty,
  saveToast,
}: Props) {
  return (
    <div
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: 12,
        padding: '10px 20px',
        borderBottom: '1px solid rgba(26,46,42,0.10)',
        background: 'var(--color-bg-elevated)',
        flexShrink: 0,
      }}
    >
      <InlineTitle value={title} onCommit={onRename} busy={renameBusy} />

      <label
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: 6,
          fontSize: 11,
          color: 'var(--color-text-muted)',
          fontFamily: 'Geist, sans-serif',
        }}
      >
        Connection:
        <select
          value={connectionId ?? ''}
          onChange={(e) => onConnectionChange(e.target.value || null)}
          style={{
            fontSize: 12,
            padding: '3px 8px',
            border: '1px solid rgba(26,46,42,0.15)',
            borderRadius: 4,
            background: 'var(--color-bg)',
            color: 'var(--color-text)',
            fontFamily: 'Geist, sans-serif',
          }}
        >
          <option value="">--- none ---</option>
          {connections.map((c) => (
            <option key={c.id} value={c.id}>
              {c.name}
            </option>
          ))}
        </select>
      </label>

      <SaveButton onSave={onSave} isSaving={isSaving} dirty={dirty} saveToast={saveToast} />
    </div>
  );
}

function InlineTitle({
  value,
  onCommit,
  busy,
}: {
  value: string;
  onCommit: (v: string) => void | Promise<void>;
  busy: boolean;
}) {
  const [editing, setEditing] = useState(false);
  const [buffer, setBuffer] = useState(value);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!editing) setBuffer(value);
  }, [value, editing]);

  useEffect(() => {
    if (editing) inputRef.current?.focus();
  }, [editing]);

  function commit() {
    const next = buffer.trim();
    setEditing(false);
    if (next && next !== value) void onCommit(next);
    else setBuffer(value);
  }

  function cancel() {
    setBuffer(value);
    setEditing(false);
  }

  if (!editing) {
    return (
      <button
        type="button"
        onClick={() => setEditing(true)}
        title="Click to rename"
        disabled={busy}
        style={{
          fontSize: 15,
          fontWeight: 600,
          border: 'none',
          background: 'transparent',
          color: 'var(--color-text)',
          fontFamily: 'Geist, sans-serif',
          textAlign: 'left',
          padding: '2px 6px',
          borderRadius: 4,
          cursor: busy ? 'default' : 'text',
          minWidth: 200,
          flex: 1,
          opacity: busy ? 0.7 : 1,
        }}
      >
        {value}
      </button>
    );
  }

  return (
    <input
      ref={inputRef}
      type="text"
      value={buffer}
      onChange={(e) => setBuffer(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === 'Enter') { e.preventDefault(); commit(); }
        else if (e.key === 'Escape') { e.preventDefault(); cancel(); }
      }}
      aria-label="Notebook title"
      style={{
        fontSize: 15,
        fontWeight: 600,
        border: '1px solid var(--color-primary)',
        background: 'var(--color-bg)',
        color: 'var(--color-text)',
        fontFamily: 'Geist, sans-serif',
        outline: 'none',
        minWidth: 200,
        flex: 1,
        padding: '1px 6px',
        borderRadius: 4,
      }}
    />
  );
}

function SaveButton({
  onSave,
  isSaving,
  dirty,
  saveToast,
}: {
  onSave: () => void;
  isSaving: boolean;
  dirty: boolean;
  saveToast: string | null;
}) {
  const showToast = Boolean(saveToast) && !dirty && !isSaving;
  const label = isSaving ? 'Saving...' : showToast ? saveToast! : dirty ? 'Save * ' : 'Save';
  const inactive = !dirty && !showToast;

  return (
    <button
      type="button"
      onClick={onSave}
      disabled={isSaving || !dirty}
      aria-label="Save notebook"
      aria-live={showToast ? 'polite' : undefined}
      style={{
        border: 'none',
        background: showToast
          ? 'var(--color-success, #2E7D32)'
          : inactive || isSaving
            ? 'rgba(42,87,81,0.35)'
            : 'var(--color-primary)',
        color: '#fff',
        padding: '5px 14px',
        borderRadius: 6,
        fontSize: 12,
        fontWeight: 600,
        cursor: inactive || isSaving ? 'default' : 'pointer',
        fontFamily: 'Geist, sans-serif',
        transition: 'background 200ms',
        minWidth: 76,
      }}
    >
      {label}
    </button>
  );
}
