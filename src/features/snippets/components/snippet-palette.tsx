import { useEffect, useMemo, useRef, useState } from 'react';
import { useSnippets } from '../hooks/use-snippets';
import type { Snippet } from '../types';

interface SnippetPaletteProps {
  open: boolean;
  onClose: () => void;
  onInsert: (body: string) => void;
}

function Kbd({ children }: { children: React.ReactNode }): React.ReactElement {
  return (
    <span style={{ background: 'rgba(26,46,42,0.07)', border: '1px solid rgba(26,46,42,0.10)', borderRadius: 4, padding: '2px 6px', fontSize: 10, fontFamily: 'Geist Mono, monospace', color: 'var(--color-text-muted)' }} aria-hidden="true">
      {children}
    </span>
  );
}

function scoreMatch(snip: Snippet, q: string): number {
  if (!q) return 1;
  const query = q.toLowerCase();
  const name = snip.name.toLowerCase();
  if (name.startsWith(query)) return 100;
  if (name.includes(query)) return 50;
  if (snip.tags.join(' ').toLowerCase().includes(query)) return 30;
  if (snip.description.toLowerCase().includes(query)) return 10;
  return 0;
}

export function SnippetPalette({ open, onClose, onInsert }: SnippetPaletteProps): React.ReactElement | null {
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const [query, setQuery] = useState('');
  const [focusIdx, setFocusIdx] = useState(0);
  const { data: snippets, isLoading } = useSnippets('mssql');

  useEffect(() => {
    if (!open) return;
    setQuery('');
    setFocusIdx(0);
    setTimeout(() => inputRef.current?.focus(), 30);
  }, [open]);

  const filtered = useMemo(() => {
    const all = snippets ?? [];
    if (!query.trim()) return [...all].sort((a, b) => a.name.localeCompare(b.name));
    return all.map((s) => ({ s, score: scoreMatch(s, query.trim()) })).filter((r) => r.score > 0).sort((a, b) => b.score - a.score || a.s.name.localeCompare(b.s.name)).map((r) => r.s);
  }, [snippets, query]);

  useEffect(() => { setFocusIdx(0); }, [query]);

  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent): void => {
      if (e.key === 'Escape') { e.preventDefault(); onClose(); }
      else if (e.key === 'ArrowDown') { e.preventDefault(); setFocusIdx((i) => Math.min(i + 1, filtered.length - 1)); }
      else if (e.key === 'ArrowUp') { e.preventDefault(); setFocusIdx((i) => Math.max(i - 1, 0)); }
      else if (e.key === 'Enter') {
        e.preventDefault();
        const pick = filtered[focusIdx];
        if (pick) { onInsert(pick.body); onClose(); }
      }
    };
    document.addEventListener('keydown', handler);
    return () => document.removeEventListener('keydown', handler);
  }, [open, filtered, focusIdx, onInsert, onClose]);

  useEffect(() => {
    if (!listRef.current) return;
    listRef.current.querySelector<HTMLDivElement>(`[data-idx="${focusIdx}"]`)?.scrollIntoView({ block: 'nearest' });
  }, [focusIdx]);

  if (!open) return null;

  const preview = filtered[focusIdx];

  return (
    <>
      <div style={{ position: 'fixed', inset: 0, background: 'rgba(26,46,42,0.3)', zIndex: 60 }} onClick={onClose} aria-hidden="true" />
      <div role="dialog" aria-label="Insert snippet" aria-modal="true" style={{ position: 'fixed', top: 80, left: '50%', transform: 'translateX(-50%)', width: 720, maxWidth: 'calc(100vw - 32px)', background: 'var(--color-bg-elevated)', borderRadius: 14, boxShadow: '0 32px 80px rgba(26,46,42,0.22), 0 8px 24px rgba(26,46,42,0.10), 0 0 0 1px rgba(26,46,42,0.08)', overflow: 'hidden', zIndex: 61 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '14px 18px', borderBottom: '1px solid rgba(26,46,42,0.08)' }}>
          <svg width="18" height="18" viewBox="0 0 18 18" fill="none" aria-hidden="true">
            <circle cx="8" cy="8" r="5.5" stroke="var(--color-text-muted)" strokeWidth="1.6" />
            <path d="M13 13l3.5 3.5" stroke="var(--color-text-muted)" strokeWidth="1.6" strokeLinecap="round" />
          </svg>
          <input ref={inputRef} type="text" value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Search snippets — name, tag, or description…" aria-label="Search snippets" style={{ flex: 1, background: 'transparent', border: 'none', outline: 'none', fontSize: 15, fontFamily: 'Geist, sans-serif', color: 'var(--color-text)', letterSpacing: '-0.01em' }} />
          <Kbd>Esc</Kbd>
        </div>

        <div style={{ display: 'flex', minHeight: 320, maxHeight: 460 }}>
          <div ref={listRef} role="listbox" aria-label="Snippets" style={{ flex: '0 0 300px', borderRight: '1px solid rgba(26,46,42,0.08)', overflowY: 'auto', padding: '6px 0' }}>
            {isLoading && <div style={{ padding: '12px 18px', fontSize: 12, color: 'var(--color-text-muted)' }}>Loading…</div>}
            {!isLoading && filtered.length === 0 && <div style={{ padding: 18, fontSize: 12, color: 'var(--color-text-muted)' }}>No snippets match "{query}".</div>}
            {filtered.map((s, i) => {
              const focused = i === focusIdx;
              return (
                <div key={s.id} data-idx={i} role="option" aria-selected={focused} onClick={() => { onInsert(s.body); onClose(); }} onMouseEnter={() => setFocusIdx(i)} style={{ padding: '8px 14px', cursor: 'pointer', background: focused ? 'rgba(26,46,42,0.06)' : 'transparent', borderLeft: `3px solid ${focused ? 'var(--color-accent)' : 'transparent'}` }}>
                  <div style={{ fontSize: 13, fontFamily: 'Geist, sans-serif', color: 'var(--color-text)', fontWeight: 500 }}>{s.name}</div>
                  <div style={{ fontSize: 11, color: 'var(--color-text-muted)', marginTop: 2, display: 'flex', gap: 6, flexWrap: 'wrap' }}>
                    {s.scope && <span>{s.scope}</span>}
                    {s.scope && s.tags.length > 0 && <span>·</span>}
                    <span>{s.tags.slice(0, 3).join(' · ')}</span>
                  </div>
                </div>
              );
            })}
          </div>

          <div style={{ flex: 1, display: 'flex', flexDirection: 'column', minWidth: 0 }}>
            {preview ? (
              <>
                <div style={{ padding: '12px 16px', borderBottom: '1px solid rgba(26,46,42,0.06)' }}>
                  <div style={{ fontSize: 13, fontWeight: 600, color: 'var(--color-text)', fontFamily: 'Geist, sans-serif' }}>{preview.name}</div>
                  <div style={{ fontSize: 11, color: 'var(--color-text-muted)', marginTop: 3 }}>{preview.description}</div>
                </div>
                <pre style={{ flex: 1, margin: 0, padding: '12px 16px', fontSize: 12, fontFamily: 'Geist Mono, monospace', color: 'var(--color-text)', background: 'rgba(26,46,42,0.03)', overflow: 'auto', whiteSpace: 'pre' }}>{preview.body}</pre>
              </>
            ) : (
              <div style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: 12, color: 'var(--color-text-muted)' }}>Select a snippet to preview.</div>
            )}
          </div>
        </div>

        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '8px 14px', borderTop: '1px solid rgba(26,46,42,0.08)', background: 'rgba(26,46,42,0.02)' }}>
          <div style={{ display: 'flex', gap: 12, fontSize: 11, color: 'var(--color-text-muted)', fontFamily: 'Geist, sans-serif' }}>
            <span style={{ display: 'flex', alignItems: 'center', gap: 5 }}><Kbd>↑↓</Kbd> Navigate</span>
            <span style={{ display: 'flex', alignItems: 'center', gap: 5 }}><Kbd>↵</Kbd> Insert</span>
            <span style={{ display: 'flex', alignItems: 'center', gap: 5 }}><Kbd>Esc</Kbd> Close</span>
          </div>
          <span style={{ fontSize: 11, color: 'var(--color-text-muted)', fontFamily: 'Geist Mono, monospace' }}>{filtered.length} / {snippets?.length ?? 0}</span>
        </div>
      </div>
    </>
  );
}
