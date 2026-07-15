import { useImport } from '../hooks/use-import';
import type { ImportResult } from '../types';
import { StepSource } from './step-source';
import { StepPreview } from './step-preview';
import { StepMapping } from './step-mapping';
import { StepExecute } from './step-execute';

export interface ImportDialogProps {
  open: boolean;
  onClose: () => void;
  connectionId: string | null;
  defaultSchema?: string;
  defaultTable?: string;
  onImported?: (result: ImportResult) => void;
}

const STEPS = [
  { id: 'source', label: 'File' },
  { id: 'preview', label: 'Preview' },
  { id: 'mapping', label: 'Mapping' },
  { id: 'execute', label: 'Options' },
] as const;

function ghostBtn(disabled: boolean): React.CSSProperties {
  return { background: 'transparent', border: '1px solid rgba(42,87,81,0.15)', color: 'var(--color-primary)', borderRadius: 8, padding: '7px 14px', fontSize: 13, cursor: disabled ? 'not-allowed' : 'pointer', opacity: disabled ? 0.6 : 1, fontFamily: 'Geist, sans-serif' };
}

function accentBtn(disabled: boolean): React.CSSProperties {
  return { background: 'var(--color-accent)', border: 'none', color: '#fff', borderRadius: 8, padding: '7px 14px', fontSize: 13, fontWeight: 500, cursor: disabled ? 'not-allowed' : 'pointer', opacity: disabled ? 0.6 : 1, fontFamily: 'Geist, sans-serif' };
}

export function ImportDialog({ open: isOpen, onClose, connectionId, defaultSchema = 'dbo', defaultTable = '', onImported }: ImportDialogProps) {
  const imp = useImport({ isOpen, connectionId, defaultSchema, defaultTable, onImported });

  if (!isOpen) return null;

  const activeIdx = STEPS.findIndex((s) => s.id === imp.step);
  const isFirst = imp.step === 'source';
  const isLast = imp.step === 'execute';
  const done = imp.result !== null && imp.result.rowsFailed === 0;

  return (
    <div
      role="dialog"
      aria-modal
      aria-labelledby="import-dialog-title"
      style={{ position: 'fixed', inset: 0, zIndex: 1000, display: 'flex', alignItems: 'center', justifyContent: 'center', background: 'rgba(26,46,42,0.45)', padding: 24, fontFamily: 'Geist, sans-serif' }}
      onClick={(e) => { if (e.target === e.currentTarget && !imp.inFlight) onClose(); }}
    >
      <div style={{ width: 720, maxWidth: '100%', maxHeight: '90vh', display: 'flex', flexDirection: 'column', background: 'var(--color-bg)', border: '1px solid rgba(42,87,81,0.13)', borderRadius: 12, boxShadow: '0 20px 60px rgba(0,0,0,0.35)', overflow: 'hidden' }}>
        {/* Header */}
        <div style={{ padding: '20px 24px 14px', borderBottom: '1px solid rgba(42,87,81,0.07)' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: 10 }}>
            <div aria-hidden style={{ width: 28, height: 28, borderRadius: 8, background: 'rgba(213,138,74,0.13)', color: 'var(--color-accent)', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
              <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
                <path d="M7 13V5M4 8l3-3 3 3M2 2h10" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
            </div>
            <h2 id="import-dialog-title" style={{ margin: 0, fontSize: 15, fontWeight: 600, color: 'var(--color-primary)', letterSpacing: '-0.01em' }}>Import data</h2>
            <div style={{ flex: 1 }} />
            {!imp.inFlight && <button type="button" onClick={onClose} aria-label="Close" style={{ background: 'transparent', border: 'none', color: 'var(--color-primary)', cursor: 'pointer', padding: 4, fontSize: 14 }}>×</button>}
          </div>
          <div style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
            {STEPS.map((s, i) => {
              const active = i === activeIdx;
              const done_ = i < activeIdx;
              return (
                <div key={s.id} style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                  <span style={{ width: 18, height: 18, borderRadius: '50%', background: done_ ? 'var(--color-primary)' : active ? 'var(--color-accent)' : 'rgba(42,87,81,0.1)', color: done_ || active ? '#fff' : 'var(--color-primary)', fontSize: 10, fontWeight: 600, display: 'inline-flex', alignItems: 'center', justifyContent: 'center', fontFamily: 'Geist Mono, monospace' }}>
                    {i + 1}
                  </span>
                  <span style={{ fontSize: 11, color: active ? 'var(--color-primary)' : 'rgba(42,87,81,0.5)', fontWeight: active ? 600 : 500 }}>{s.label}</span>
                  {i < STEPS.length - 1 && <span style={{ width: 20, height: 1, background: 'rgba(42,87,81,0.15)', margin: '0 2px' }} />}
                </div>
              );
            })}
          </div>
        </div>

        {/* Body */}
        <div style={{ padding: '18px 24px', flex: 1, overflowY: 'auto', minHeight: 0 }}>
          {imp.step === 'source' && <StepSource path={imp.path} format={imp.format} isLoading={imp.inFlight} onPick={imp.pickFile} />}
          {imp.step === 'preview' && imp.preview && <StepPreview preview={imp.preview} />}
          {imp.step === 'mapping' && <StepMapping mapping={imp.mapping} onChange={imp.setMapping} />}
          {imp.step === 'execute' && <StepExecute options={imp.options} onChange={imp.setOptions} targetSchema={imp.targetSchema} targetTable={imp.targetTable} onSchemaChange={imp.setTargetSchema} onTableChange={imp.setTargetTable} connectionMissing={connectionId === null} result={imp.result} isRunning={imp.inFlight} />}
          {imp.errorMsg && <div role="alert" style={{ marginTop: 12, fontSize: 12, color: 'var(--color-error)', background: 'rgba(220,38,38,0.08)', borderRadius: 6, padding: '8px 10px', wordBreak: 'break-word' }}>{imp.errorMsg}</div>}
        </div>

        {/* Footer */}
        <div style={{ padding: '14px 24px 18px', borderTop: '1px solid rgba(42,87,81,0.07)', display: 'flex', justifyContent: 'flex-end', gap: 10 }}>
          {!isFirst && <button type="button" onClick={imp.goBack} disabled={imp.inFlight} style={ghostBtn(imp.inFlight)}>Back</button>}
          <div style={{ flex: 1 }} />
          <button type="button" onClick={onClose} disabled={imp.inFlight} style={ghostBtn(imp.inFlight)}>{done ? 'Close' : 'Cancel'}</button>
          {!isLast
            ? <button type="button" onClick={imp.goNext} disabled={!imp.canGoNext || imp.inFlight} style={accentBtn(!imp.canGoNext || imp.inFlight)}>Next</button>
            : (!done && <button type="button" onClick={imp.runImport} disabled={!imp.canImport || imp.inFlight} style={accentBtn(!imp.canImport || imp.inFlight)}>{imp.inFlight ? 'Importing…' : 'Start import'}</button>)
          }
        </div>
      </div>
    </div>
  );
}
