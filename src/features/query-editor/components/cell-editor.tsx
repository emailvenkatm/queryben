import { useEffect, useRef, useState } from 'react';
import { NULL_SENTINEL, type EditingValue } from '../hooks/use-browse-mutations';

function resolveInputType(sqlType: string): 'date' | 'datetime-local' | 'time' | null {
  const t = sqlType.toLowerCase().trim();
  if (t === 'date') return 'date';
  if (t === 'time') return 'time';
  if (
    t === 'datetime' || t === 'datetime2' || t === 'smalldatetime' ||
    t === 'datetimeoffset' || t.startsWith('datetime2(') || t.startsWith('datetimeoffset(')
  ) return 'datetime-local';
  return null;
}

function coerceToInputFormat(raw: unknown, inputType: 'date' | 'datetime-local' | 'time'): string {
  if (raw === null || raw === undefined) return '';
  const s = String(raw);
  if (inputType === 'date') return s.match(/^(\d{4}-\d{2}-\d{2})/)?.[1] ?? s;
  if (inputType === 'datetime-local') return s.match(/^(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2})/)?.[1] ?? s.slice(0, 16);
  if (inputType === 'time') return s.match(/^(\d{2}:\d{2}(:\d{2})?)/)?.[1] ?? s;
  return s;
}

interface CellEditorProps {
  initial: unknown;
  sqlType?: string;
  isNullable?: boolean;
  onCommit: (value: EditingValue) => void;
  onCancel: () => void;
}

export function CellEditor({
  initial,
  sqlType = '',
  isNullable = false,
  onCommit,
  onCancel,
}: CellEditorProps): React.ReactElement {
  const inputType = resolveInputType(sqlType);
  const [isNull, setIsNull] = useState(initial === null);
  const [text, setText] = useState(() => {
    if (initial === null || initial === undefined) return '';
    if (inputType) return coerceToInputFormat(initial, inputType);
    return String(initial);
  });
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const raf = requestAnimationFrame(() => {
      inputRef.current?.focus();
      if (!inputType) inputRef.current?.select();
    });
    return () => cancelAnimationFrame(raf);
  }, [inputType]);

  const commit = (): void => {
    onCommit(isNull ? NULL_SENTINEL : text);
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>): void => {
    if (e.key.toLowerCase() === 'n' && (e.metaKey || e.ctrlKey) && e.shiftKey) {
      e.preventDefault();
      setIsNull(true);
      setText('');
      return;
    }
    if (e.key === 'Enter') { e.preventDefault(); commit(); }
    if (e.key === 'Escape') { e.preventDefault(); onCancel(); }
  };

  return (
    <div style={{ display: 'flex', alignItems: 'center', outline: '2px solid var(--color-accent)', outlineOffset: -1, background: 'var(--color-bg-elevated)' }}>
      <input
        ref={inputRef}
        type={inputType ?? 'text'}
        value={isNull ? '' : text}
        onChange={(e) => { setText(e.target.value); setIsNull(false); }}
        onBlur={commit}
        onKeyDown={handleKeyDown}
        placeholder={isNull ? 'NULL' : ''}
        style={{ flex: 1, padding: '5px 12px', border: 'none', outline: 'none', background: '#fff', fontFamily: 'Geist Mono, monospace', fontSize: 12, color: 'var(--color-text)', minWidth: 0 }}
      />
      {isNullable && (
        <button
          type="button"
          onMouseDown={(e) => { e.preventDefault(); setIsNull((v) => !v); if (!isNull) setText(''); }}
          title="Toggle NULL"
          style={{ flexShrink: 0, padding: '3px 7px', fontSize: 10, fontWeight: 700, fontFamily: 'Geist Mono, monospace', letterSpacing: '0.04em', color: isNull ? '#fff' : 'rgba(26,46,42,0.45)', background: isNull ? 'rgba(26,46,42,0.55)' : 'rgba(26,46,42,0.06)', border: 'none', borderLeft: '1px solid rgba(26,46,42,0.12)', cursor: 'pointer', alignSelf: 'stretch' }}
        >
          NULL
        </button>
      )}
    </div>
  );
}

interface InsertCellInputProps {
  value: unknown;
  sqlType: string;
  isNullable: boolean;
  columnName: string;
  onCommit: (value: EditingValue) => void;
}

export function InsertCellInput({ value, sqlType, isNullable, columnName, onCommit }: InsertCellInputProps): React.ReactElement {
  const inputType = resolveInputType(sqlType);
  const [text, setText] = useState(() => {
    if (value === null || value === undefined) return '';
    if (inputType) return coerceToInputFormat(value, inputType);
    return String(value);
  });
  const [isNull, setIsNull] = useState(value === null);
  const [focused, setFocused] = useState(false);

  const commit = (raw: string, nullFlag: boolean): void => {
    if (nullFlag) onCommit(NULL_SENTINEL);
    else if (raw !== '') onCommit(raw);
  };

  return (
    <div style={{ display: 'flex', alignItems: 'center', width: '100%', borderBottom: `2px solid ${focused ? 'var(--color-accent)' : 'transparent'}`, transition: 'border-color 120ms' }}>
      <input
        type={inputType ?? 'text'}
        value={isNull ? '' : text}
        placeholder={isNull ? 'NULL' : columnName}
        onFocus={() => setFocused(true)}
        onChange={(e) => { setText(e.target.value); if (isNull) setIsNull(false); if (inputType) commit(e.target.value, false); }}
        onBlur={(e) => { setFocused(false); if (!inputType) commit(e.target.value, isNull); }}
        onKeyDown={(e) => {
          if (e.key === 'Enter') { e.preventDefault(); commit((e.target as HTMLInputElement).value, isNull); }
          if (e.key.toLowerCase() === 'n' && (e.metaKey || e.ctrlKey) && e.shiftKey) { e.preventDefault(); setIsNull(true); setText(''); commit('', true); }
        }}
        style={{ flex: 1, minWidth: 0, padding: '5px 12px', border: 'none', outline: 'none', background: 'transparent', fontFamily: 'Geist Mono, monospace', fontSize: 12, color: 'var(--color-text)' }}
      />
      {isNullable && (
        <button
          type="button"
          onMouseDown={(e) => { e.preventDefault(); if (isNull) { setIsNull(false); } else { setIsNull(true); setText(''); commit('', true); } }}
          title="Toggle NULL"
          style={{ flexShrink: 0, padding: '2px 7px', fontSize: 9, fontWeight: 700, fontFamily: 'Geist Mono, monospace', letterSpacing: '0.04em', color: isNull ? '#fff' : 'rgba(26,46,42,0.45)', background: isNull ? 'rgba(26,46,42,0.55)' : 'rgba(26,46,42,0.06)', border: 'none', borderLeft: '1px solid rgba(26,46,42,0.10)', cursor: 'pointer', alignSelf: 'stretch' }}
        >
          NULL
        </button>
      )}
    </div>
  );
}
