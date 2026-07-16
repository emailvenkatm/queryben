import { useEffect, useMemo, useState } from 'react';
import { save } from '@tauri-apps/plugin-dialog';
import type { CellValue, ResultColumn } from '@/shared/types';
import type { ExportFormat } from '../types';
import { FORMAT_OPTIONS, optionFor } from '../api';
import { useExport } from '../hooks/use-export';

export interface ExportDialogProps {
  open: boolean;
  onClose: () => void;
  columns: ResultColumn[];
  rows: CellValue[][];
  defaultFilename?: string;
}

type Phase = 'idle' | 'picking' | 'writing' | 'done' | 'error';

const JADE = '#2A5751';
const CREAM = '#F7F1E1';
const AMBER = '#D58A4A';

function mix(a: number): string {
  return `rgba(42, 87, 81, ${Math.min(100, Math.max(0, a)) / 100})`;
}

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / (1024 * 1024)).toFixed(2)} MB`;
}

function errStr(err: unknown): string {
  if (err === null || err === undefined) return 'Something went wrong.';
  if (typeof err === 'string') return err;
  if (typeof err === 'object') {
    const e = err as { message?: unknown; kind?: unknown };
    if (typeof e.message === 'string') return e.message;
    if (typeof e.kind === 'string') return e.kind;
    try { return JSON.stringify(err); } catch { return String(err); }
  }
  return String(err);
}

export function ExportDialog({ open, onClose, columns, rows, defaultFilename = 'results' }: ExportDialogProps) {
  const [format, setFormat] = useState<ExportFormat>('csv');
  const [phase, setPhase] = useState<Phase>('idle');
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const [doneMsg, setDoneMsg] = useState<string | null>(null);
  const exportMut = useExport();

  useEffect(() => {
    if (open) { setPhase('idle'); setErrorMsg(null); setDoneMsg(null); }
  }, [open]);

  const ROW_CAP = 10_000;
  const willTruncate = rows.length > ROW_CAP;

  const safeStem = useMemo(
    () =>
      defaultFilename.replace(/[\/\\:*?"<>|]/g, '-').replace(/\s+/g, '-').replace(/-+/g, '-').replace(/^-|-$/g, '').slice(0, 80) || 'results',
    [defaultFilename],
  );

  const handleExport = async (): Promise<void> => {
    setPhase('picking');
    setErrorMsg(null);
    const opt = optionFor(format);
    try {
      const chosen = await save({
        title: 'Export results',
        defaultPath: `${safeStem}.${opt.extension}`,
        filters: [{ name: opt.label, extensions: [opt.extension] }],
      });
      if (!chosen) { setPhase('idle'); return; }
      setPhase('writing');
      const result = await exportMut.mutateAsync({ format, path: chosen, columns, rows: rows.slice(0, ROW_CAP) });
      setDoneMsg(`Wrote ${result.rowsWritten.toLocaleString()} rows (${formatBytes(result.bytesWritten)}) to ${result.path}`);
      setPhase('done');
    } catch (err) {
      setErrorMsg(errStr(err));
      setPhase('error');
    }
  };

  if (!open) return null;

  const busy = phase === 'picking' || phase === 'writing';

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="export-dialog-title"
      style={{ position: 'fixed', inset: 0, zIndex: 1000, display: 'flex', alignItems: 'center', justifyContent: 'center', background: 'rgba(26,46,42,0.45)', padding: 24, fontFamily: 'Geist, sans-serif' }}
      onClick={(e) => { if (e.target === e.currentTarget && !busy) onClose(); }}
    >
      <div style={{ width: 520, maxWidth: '100%', background: CREAM, border: `1px solid ${mix(13)}`, borderRadius: 12, boxShadow: '0 20px 60px rgba(0,0,0,0.35)', overflow: 'hidden' }}>
        <div style={{ padding: '20px 24px 16px', borderBottom: `1px solid ${mix(7)}` }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
            <div aria-hidden="true" style={{ width: 28, height: 28, borderRadius: 8, background: 'rgba(213,138,74,0.13)', color: AMBER, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
              <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
                <path d="M7 1v8M4 6l3 3 3-3M2 12h10" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
            </div>
            <h2 id="export-dialog-title" style={{ margin: 0, fontSize: 15, fontWeight: 600, color: JADE, letterSpacing: '-0.01em' }}>
              Export results
            </h2>
          </div>
        </div>

        <div style={{ padding: '18px 24px 20px' }}>
          <div style={{ fontSize: 12, color: JADE, marginBottom: 10 }}>
            {rows.length.toLocaleString()} {rows.length === 1 ? 'row' : 'rows'} · {columns.length} {columns.length === 1 ? 'column' : 'columns'}
            {willTruncate && <span style={{ color: 'var(--color-warning)', marginLeft: 6 }}>(only the first {ROW_CAP.toLocaleString()} will be written)</span>}
          </div>

          <fieldset style={{ border: `1px solid ${mix(10)}`, borderRadius: 8, padding: 4, marginBottom: 14 }}>
            <legend style={{ padding: '0 6px', fontSize: 11, color: JADE, fontWeight: 600 }}>Format</legend>
            {FORMAT_OPTIONS.map((opt) => {
              const isActive = format === opt.id;
              return (
                <label key={opt.id} style={{ display: 'flex', alignItems: 'flex-start', gap: 10, padding: '9px 10px', cursor: 'pointer', borderRadius: 6, background: isActive ? mix(6) : 'transparent' }}>
                  <input type="radio" name="export-format" value={opt.id} checked={isActive} onChange={() => setFormat(opt.id)} disabled={busy} style={{ marginTop: 3, accentColor: AMBER }} />
                  <div style={{ flex: 1 }}>
                    <div style={{ fontSize: 13, fontWeight: 500, color: JADE }}>
                      {opt.label}
                      <span style={{ marginLeft: 6, fontFamily: 'Geist Mono, monospace', fontSize: 10, color: 'var(--color-text-muted)' }}>.{opt.extension}</span>
                    </div>
                    <div style={{ fontSize: 11, color: 'var(--color-text-muted)', marginTop: 2 }}>{opt.description}</div>
                  </div>
                </label>
              );
            })}
          </fieldset>

          {phase === 'picking' && <div style={{ fontSize: 12, color: 'var(--color-text-muted)' }}>Choose a destination in the save dialog…</div>}
          {phase === 'writing' && <div style={{ fontSize: 12, color: 'var(--color-text-muted)' }}>Writing {Math.min(rows.length, ROW_CAP).toLocaleString()} rows…</div>}
          {phase === 'done' && doneMsg && <div role="status" style={{ fontSize: 12, color: 'var(--color-success)', background: 'rgba(46,125,50,0.08)', borderRadius: 6, padding: '8px 10px', wordBreak: 'break-all' }}>{doneMsg}</div>}
          {phase === 'error' && errorMsg && <div role="alert" style={{ fontSize: 12, color: 'var(--color-error)', background: 'rgba(220,38,38,0.08)', borderRadius: 6, padding: '8px 10px', wordBreak: 'break-word' }}>Export failed: {errorMsg}</div>}
        </div>

        <div style={{ padding: '14px 24px 18px', borderTop: `1px solid ${mix(7)}`, display: 'flex', justifyContent: 'flex-end', gap: 10 }}>
          <button type="button" onClick={onClose} disabled={busy} style={{ background: 'transparent', border: `1px solid ${mix(15)}`, color: JADE, borderRadius: 8, padding: '7px 14px', fontSize: 13, cursor: busy ? 'not-allowed' : 'pointer', opacity: busy ? 0.6 : 1, fontFamily: 'Geist, sans-serif' }}>
            {phase === 'done' ? 'Close' : 'Cancel'}
          </button>
          {phase !== 'done' && (
            <button type="button" onClick={handleExport} disabled={busy || rows.length === 0} style={{ background: AMBER, border: 'none', color: '#fff', borderRadius: 8, padding: '7px 14px', fontSize: 13, fontWeight: 500, cursor: busy || rows.length === 0 ? 'not-allowed' : 'pointer', opacity: busy || rows.length === 0 ? 0.6 : 1, fontFamily: 'Geist, sans-serif' }}>
              {phase === 'picking' ? 'Waiting for save dialog…' : phase === 'writing' ? 'Exporting…' : 'Export'}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
