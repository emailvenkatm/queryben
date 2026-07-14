import { useState } from 'react';
import type { DdlStatement } from '../types';

interface Props {
  statements: DdlStatement[];
  isGenerating: boolean;
  onGenerate: () => void;
}

const FALLBACK = { color: 'var(--color-accent, #D58A4A)', bg: 'rgba(213,138,74,0.12)' };
const KIND_COLOR: Record<string, { color: string; bg: string }> = {
  CREATE: { color: 'var(--color-primary-hover, #1a2e2a)', bg: 'rgba(42,87,81,0.10)' },
  ALTER: { color: 'var(--color-accent, #D58A4A)', bg: 'rgba(213,138,74,0.12)' },
  DROP: { color: 'var(--color-error, #c0392b)', bg: 'rgba(192,57,43,0.08)' },
};

export function MigrationSqlPanel({ statements, isGenerating, onGenerate }: Props) {
  const [copied, setCopied] = useState(false);
  const full = statements.map((s) => s.sql).join('\n\n');

  async function handleCopy() {
    try {
      await navigator.clipboard.writeText(full);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // clipboard denied - user can select manually
    }
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', borderTop: '1px solid rgba(26,46,42,0.08)', background: 'var(--color-bg-elevated)', minHeight: 0, maxHeight: '40vh' }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 12, padding: '10px 16px', borderBottom: '1px solid rgba(26,46,42,0.08)' }}>
        <span style={{ fontSize: 12, fontFamily: 'Geist Mono, monospace', textTransform: 'uppercase', letterSpacing: '0.05em', color: 'var(--color-text-muted)' }}>
          Migration SQL
        </span>
        <span style={{ fontSize: 11, color: 'var(--color-text-muted)' }}>
          {statements.length} statement{statements.length === 1 ? '' : 's'}
        </span>
        <div style={{ flex: 1 }} />
        <button
          type="button"
          onClick={onGenerate}
          disabled={isGenerating}
          style={{ fontSize: 12, padding: '5px 12px', borderRadius: 6, border: '1px solid rgba(26,46,42,0.12)', background: 'var(--color-bg)', cursor: isGenerating ? 'progress' : 'pointer', color: 'var(--color-text)', fontFamily: 'Geist, sans-serif' }}
        >
          {isGenerating ? 'Generating...' : 'Regenerate'}
        </button>
        <button
          type="button"
          onClick={() => void handleCopy()}
          disabled={statements.length === 0}
          style={{ fontSize: 12, fontWeight: 500, padding: '5px 12px', borderRadius: 6, border: 'none', background: 'var(--color-accent)', color: '#fff', cursor: statements.length === 0 ? 'not-allowed' : 'pointer', opacity: statements.length === 0 ? 0.5 : 1, fontFamily: 'Geist, sans-serif' }}
        >
          {copied ? 'Copied' : 'Copy all'}
        </button>
      </div>
      <div style={{ flex: 1, overflow: 'auto', padding: '6px 0' }}>
        {statements.length === 0 ? (
          <div style={{ padding: 20, color: 'var(--color-text-muted)', fontSize: 12, fontFamily: 'Geist Mono, monospace' }}>
            {isGenerating ? 'Generating migration...' : 'Run a compare to generate migration SQL.'}
          </div>
        ) : (
          statements.map((stmt, idx) => {
            const meta = KIND_COLOR[stmt.kind] ?? FALLBACK;
            return (
              <div key={`${stmt.objectName}-${idx}`} style={{ padding: '4px 14px', fontFamily: 'Geist Mono, monospace', fontSize: 12, color: 'var(--color-text)' }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 2 }}>
                  <span style={{ fontSize: 10, padding: '1px 6px', borderRadius: 3, color: meta.color, background: meta.bg, textTransform: 'uppercase', letterSpacing: '0.03em' }}>
                    {stmt.kind}
                  </span>
                  <span style={{ color: 'var(--color-text-muted)', fontSize: 11 }}>{stmt.objectName}</span>
                </div>
                <pre style={{ margin: 0, padding: '2px 0 8px 0', whiteSpace: 'pre-wrap', wordBreak: 'break-word' }}>
                  {stmt.sql}
                </pre>
              </div>
            );
          })
        )}
      </div>
    </div>
  );
}
