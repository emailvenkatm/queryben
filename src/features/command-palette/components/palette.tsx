import { useEffect, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { PaletteItem, SectionHeader, HighlightMatch } from './palette-item';
import { usePaletteCommands } from '../hooks/use-palette-commands';
import { ConnectionDot } from '@/shared/ui/color-tag';

interface PaletteProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

function EnvBadge({ env }: { env: string | undefined }) {
  if (!env) return null;
  const styles: Record<string, { bg: string; color: string }> = {
    production:  { bg: 'var(--color-code-bg)', color: 'var(--color-error)' },
    staging:     { bg: 'var(--color-code-bg)', color: 'var(--color-warning)' },
    development: { bg: 'var(--color-code-bg)', color: 'var(--color-success)' },
    local:       { bg: 'var(--color-code-bg)', color: 'var(--color-success)' },
  };
  const s = styles[env] ?? { bg: 'var(--color-code-bg)', color: 'var(--color-success)' };
  const label = env === 'production' ? 'PROD' : env === 'staging' ? 'STAGING' : 'DEV';
  return (
    <span style={{ background: s.bg, color: s.color, fontSize: 9, fontWeight: 700, padding: '1px 5px', borderRadius: 3, flexShrink: 0, fontFamily: 'Geist Mono, monospace' }}>
      {label}
    </span>
  );
}

function QueryIcon() {
  return (
    <div style={{ width: 30, height: 30, borderRadius: 8, background: 'rgba(26,46,42,0.07)', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
      <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true">
        <path d="M1 1h12v12H1z" stroke="rgba(26,46,42,0.5)" strokeWidth="1.1" />
        <path d="M3 4h8M3 7h5M3 10h6" stroke="rgba(26,46,42,0.5)" strokeWidth="1" strokeLinecap="round" />
      </svg>
    </div>
  );
}

function ConnIcon() {
  return (
    <div style={{ width: 30, height: 30, borderRadius: 8, background: 'rgba(0,120,212,0.08)', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
      <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true">
        <ellipse cx="7" cy="4.5" rx="4.5" ry="1.8" stroke="#0078D4" strokeWidth="1.2" />
        <path d="M2.5 4.5v5c0 .994 2.015 1.8 4.5 1.8s4.5-.806 4.5-1.8v-5" stroke="#0078D4" strokeWidth="1.2" />
      </svg>
    </div>
  );
}

function CmdIcon() {
  return (
    <div style={{ width: 30, height: 30, borderRadius: 8, background: 'rgba(213,138,74,0.10)', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
      <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true">
        <path d="M2 9l3-3-3-3M7 9h5" stroke="var(--color-accent)" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" />
      </svg>
    </div>
  );
}

function PDivider() {
  return <div style={{ borderTop: '1px solid rgba(26,46,42,0.07)', margin: '4px 0' }} />;
}

// TODO: wire to query-editor + results-copy features when ported.
// For now, these stubs keep the palette functional without the missing features.
function useResultsStub() {
  return { hasResults: false, copy: (_format: string) => {} };
}

export function Palette({ open, onOpenChange }: PaletteProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState('');
  const [exportOpen, setExportOpen] = useState(false);
  const { hasResults, copy } = useResultsStub();

  const close = (): void => {
    onOpenChange(false);
    setQuery('');
  };

  useEffect(() => {
    if (open) {
      setTimeout(() => inputRef.current?.focus(), 50);
      setQuery('');
    }
  }, [open]);

  useEffect(() => {
    const handler = (e: KeyboardEvent): void => { if (e.key === 'Escape') close(); };
    document.addEventListener('keydown', handler);
    return () => document.removeEventListener('keydown', handler);
  });

  const { tabCommands, systemCommands, connectionCommands, filteredConnections } = usePaletteCommands({
    query,
    onClose: close,
    onExport: () => setExportOpen(true),
    onCopy: (format) => copy(format),
    hasResults,
  });

  if (!open) return null;

  return (
    <>
      <div
        style={{ position: 'fixed', inset: 0, background: 'rgba(26,46,42,0.3)', zIndex: 50 }}
        onClick={close}
        aria-hidden="true"
      />
      <div
        role="dialog"
        aria-label="Command palette"
        aria-modal="true"
        style={{ position: 'fixed', top: 80, left: '50%', transform: 'translateX(-50%)', width: 640, background: 'var(--color-bg-elevated)', borderRadius: 14, boxShadow: '0 32px 80px rgba(26,46,42,0.22), 0 8px 24px rgba(26,46,42,0.10), 0 0 0 1px rgba(26,46,42,0.08)', overflow: 'hidden', zIndex: 51 }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '14px 18px', borderBottom: '1px solid rgba(26,46,42,0.08)' }}>
          <svg width="18" height="18" viewBox="0 0 18 18" fill="none" aria-hidden="true">
            <circle cx="8" cy="8" r="5.5" stroke="var(--color-text-muted)" strokeWidth="1.6" />
            <path d="M13 13l3.5 3.5" stroke="var(--color-text-muted)" strokeWidth="1.6" strokeLinecap="round" />
          </svg>
          <input
            ref={inputRef}
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search queries, connections, commands…"
            aria-label="Search"
            style={{ flex: 1, background: 'transparent', border: 'none', outline: 'none', fontSize: 16, fontFamily: 'Geist, sans-serif', color: 'var(--color-text)', letterSpacing: '-0.01em' }}
          />
          <span style={{ background: 'rgba(26,46,42,0.07)', border: '1px solid rgba(26,46,42,0.10)', borderRadius: 4, padding: '2px 6px', fontSize: 10, fontFamily: 'Geist Mono, monospace', color: 'var(--color-text-muted)' }} aria-hidden="true">Esc</span>
        </div>

        <div role="listbox" style={{ maxHeight: 420, overflowY: 'auto' }}>
          {tabCommands.length > 0 && (
            <>
              <SectionHeader label="Open Tabs" />
              {tabCommands.map((cmd, i) => (
                <PaletteItem
                  key={cmd.id}
                  icon={<QueryIcon />}
                  label={<HighlightMatch text={cmd.label} query={query} />}
                  sub={cmd.sub}
                  isFocused={i === 0 && !query}
                  onClick={cmd.onSelect}
                />
              ))}
              <PDivider />
            </>
          )}

          {systemCommands.length > 0 && (
            <>
              <SectionHeader label="Commands" />
              {systemCommands.map((cmd) => (
                <PaletteItem
                  key={cmd.id}
                  icon={<CmdIcon />}
                  label={<HighlightMatch text={cmd.label} query={query} />}
                  sub={cmd.sub}
                  kbd={cmd.kbd}
                  onClick={cmd.onSelect}
                />
              ))}
              <PDivider />
            </>
          )}

          {connectionCommands.length > 0 && (
            <>
              <SectionHeader label="Switch connection" />
              {(connectionCommands as (ReturnType<typeof usePaletteCommands>['connectionCommands'][number] & { _conn?: import('@/shared/types').Connection })[]).map((cmd) => (
                <PaletteItem
                  key={cmd.id}
                  icon={<ConnIcon />}
                  label={
                    <span style={{ display: 'flex', alignItems: 'center', gap: 7 }}>
                      {cmd._conn && <ConnectionDot color={cmd._conn.color} />}
                      <HighlightMatch text={cmd.label} query={query} />
                      {cmd._conn && <EnvBadge env={cmd._conn.environment} />}
                    </span>
                  }
                  sub={cmd.sub}
                  onClick={cmd.onSelect}
                />
              ))}
            </>
          )}

          {connectionCommands.length === 0 && tabCommands.length === 0 && systemCommands.length === 0 && (
            <div style={{ padding: '32px 16px', textAlign: 'center', fontSize: 13, color: 'var(--color-text-muted)' }}>
              No results for "{query}"
            </div>
          )}
        </div>

        <div style={{ borderTop: '1px solid rgba(26,46,42,0.07)', padding: '9px 16px', display: 'flex', alignItems: 'center', gap: 16 }}>
          {[{ key: '↑↓', label: 'Navigate' }, { key: '↵', label: 'Open' }, { key: 'Tab', label: 'Preview' }].map(({ key, label }) => (
            <div key={key} style={{ display: 'flex', alignItems: 'center', gap: 5, fontSize: 11, color: 'var(--color-text-muted)' }}>
              <span style={{ background: 'rgba(26,46,42,0.06)', borderRadius: 3, padding: '1px 5px', fontSize: 10, fontFamily: 'Geist Mono, monospace' }} aria-hidden="true">{key}</span>
              {label}
            </div>
          ))}
          <div style={{ flex: 1 }} />
          <div style={{ display: 'flex', alignItems: 'center', gap: 5, fontSize: 11, color: 'var(--color-text-muted)' }}>
            <span style={{ background: 'rgba(26,46,42,0.06)', borderRadius: 3, padding: '1px 5px', fontSize: 10, fontFamily: 'Geist Mono, monospace' }} aria-hidden="true">⌘K</span>
            Close
          </div>
        </div>
      </div>
    </>
  );
}
