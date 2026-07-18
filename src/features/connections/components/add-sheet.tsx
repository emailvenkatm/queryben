import { useState } from 'react';
import { useAddConnectionForm } from '../hooks/use-add-connection-form';
import { AzureWizard } from './azure-wizard';

interface AddSheetProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

type Mode = 'azure' | 'manual';

// Local overlay — kept here until a second caller shows up and we lift it
// into shared/ui with a proper API.
interface OverlaySheetProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  label: string;
  width?: number;
  children: React.ReactNode;
}

function OverlaySheet({ open, onOpenChange, label, width = 480, children }: OverlaySheetProps) {
  if (!open) return null;
  return (
    <>
      <div
        style={{ position: 'fixed', inset: 0, background: 'rgba(26,46,42,0.25)', zIndex: 40 }}
        onClick={() => onOpenChange(false)}
        aria-hidden="true"
      />
      <div
        role="dialog"
        aria-modal="true"
        aria-label={label}
        style={{ position: 'fixed', right: 0, top: 0, bottom: 0, width, maxWidth: width, background: 'var(--color-bg-elevated)', borderLeft: '1px solid rgba(26,46,42,0.10)', boxShadow: '-8px 0 32px rgba(26,46,42,0.08)', zIndex: 41, display: 'flex', flexDirection: 'column', gap: 0 }}
      >
        {children}
      </div>
    </>
  );
}

const inputStyle: React.CSSProperties = {
  width: '100%',
  padding: '9px 12px',
  fontSize: 13,
  fontFamily: 'Geist Mono, monospace',
  background: 'var(--color-bg)',
  border: '1px solid rgba(26,46,42,0.15)',
  borderRadius: 8,
  color: 'var(--color-text)',
  outline: 'none',
  boxSizing: 'border-box',
};

const labelStyle: React.CSSProperties = {
  fontSize: 12,
  fontWeight: 500,
  color: 'var(--color-text)',
  marginBottom: 6,
  display: 'block',
};

export function AddSheet({ open, onOpenChange }: AddSheetProps) {
  const [mode, setMode] = useState<Mode>('azure');
  const { form, needsCredentials, createConn, testConn, handleSubmit, handleTest } =
    useAddConnectionForm(open, () => onOpenChange(false));

  const { register, formState: { errors } } = form;

  return (
    <OverlaySheet open={open} onOpenChange={onOpenChange} label="New connection">
      <div style={{ padding: '24px 28px 20px', borderBottom: '1px solid rgba(26,46,42,0.08)' }}>
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
          <div>
            <h2 style={{ fontSize: 16, fontWeight: 600, color: 'var(--color-text)', letterSpacing: '-0.02em', margin: 0 }}>
              New connection
            </h2>
            <p style={{ fontSize: 12, color: 'var(--color-text-muted)', margin: '3px 0 0' }}>
              SQL Server · Azure SQL · Synapse
            </p>
          </div>
          <button
            type="button"
            onClick={() => onOpenChange(false)}
            aria-label="Close"
            style={{ background: 'rgba(26,46,42,0.06)', border: 'none', borderRadius: 7, width: 28, height: 28, display: 'flex', alignItems: 'center', justifyContent: 'center', cursor: 'pointer', color: 'var(--color-text-muted)' }}
          >
            <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true">
              <path d="M1 1l10 10M11 1L1 11" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
            </svg>
          </button>
        </div>
      </div>

      <div style={{ padding: '16px 28px 0' }}>
        <div role="tablist" aria-label="Connection mode" style={{ display: 'flex', background: 'rgba(26,46,42,0.05)', borderRadius: 8, padding: 3, gap: 2 }}>
          {(['azure', 'manual'] as Mode[]).map((m) => (
            <button
              key={m}
              role="tab"
              type="button"
              aria-selected={mode === m}
              onClick={() => setMode(m)}
              style={{ flex: 1, padding: '6px 10px', fontSize: 12, fontWeight: 500, borderRadius: 6, cursor: 'pointer', textAlign: 'center', border: 'none', fontFamily: 'Geist, sans-serif', transition: 'all 120ms', background: mode === m ? 'var(--color-bg-elevated)' : 'transparent', color: mode === m ? 'var(--color-text)' : 'var(--color-text-muted)', boxShadow: mode === m ? '0 1px 3px rgba(26,46,42,0.10)' : 'none' }}
            >
              {m === 'azure' ? 'Azure SSO' : 'Manual (SQL Auth)'}
            </button>
          ))}
        </div>
      </div>

      <div style={{ flex: 1, overflowY: 'auto', padding: '24px 28px' }}>
        {mode === 'azure' ? (
          <AzureWizard onSuccess={() => onOpenChange(false)} />
        ) : (
          <form onSubmit={handleSubmit} noValidate style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
            <div>
              <label style={labelStyle}>Server</label>
              <input {...register('server')} placeholder="myserver.database.windows.net" autoComplete="off" style={inputStyle} />
              {errors.server && <p style={{ fontSize: 11, color: 'var(--color-error)', marginTop: 4 }}>{errors.server.message}</p>}
            </div>
            <div>
              <label style={labelStyle}>Database</label>
              <input {...register('database')} placeholder="my_database" autoComplete="off" style={inputStyle} />
              {errors.database && <p style={{ fontSize: 11, color: 'var(--color-error)', marginTop: 4 }}>{errors.database.message}</p>}
            </div>
            {needsCredentials && (
              <>
                <div>
                  <label style={labelStyle}>Username</label>
                  <input {...register('username')} autoComplete="username" style={inputStyle} />
                </div>
                <div>
                  <label style={labelStyle}>Password</label>
                  <input {...register('password')} type="password" autoComplete="current-password" style={inputStyle} />
                </div>
              </>
            )}
            {(testConn.isSuccess || testConn.isError) && (
              <div
                role="status"
                aria-live="polite"
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: 8,
                  padding: '8px 12px',
                  borderRadius: 6,
                  fontSize: 12,
                  background: testConn.data?.ok ? 'rgba(74,168,97,0.1)' : 'rgba(220,38,38,0.08)',
                  color: testConn.data?.ok ? 'var(--color-success)' : 'var(--color-error)',
                }}
              >
                {testConn.data?.ok ? 'Connection successful' : testConn.data?.message ?? 'Connection failed'}
              </div>
            )}
          </form>
        )}
      </div>

      <div style={{ padding: '16px 28px', borderTop: '1px solid rgba(26,46,42,0.08)', display: 'flex', gap: 10, justifyContent: 'flex-end' }}>
        <button
          type="button"
          onClick={() => onOpenChange(false)}
          style={{ background: 'rgba(26,46,42,0.06)', border: 'none', borderRadius: 8, padding: '9px 18px', fontSize: 13, fontWeight: 500, color: 'var(--color-text-muted)', cursor: 'pointer', fontFamily: 'Geist, sans-serif' }}
        >
          Cancel
        </button>
        {mode === 'manual' && (
          <>
            <button
              type="button"
              onClick={() => { void handleTest(); }}
              disabled={testConn.isPending}
              style={{ background: 'rgba(26,46,42,0.06)', border: 'none', borderRadius: 8, padding: '9px 18px', fontSize: 13, fontWeight: 500, color: 'var(--color-text)', cursor: 'pointer', fontFamily: 'Geist, sans-serif', opacity: testConn.isPending ? 0.6 : 1 }}
            >
              {testConn.isPending ? 'Testing…' : 'Test connection'}
            </button>
            <button
              type="button"
              onClick={() => { void handleSubmit(); }}
              disabled={createConn.isPending}
              style={{ background: 'var(--color-accent)', border: 'none', borderRadius: 8, padding: '9px 20px', fontSize: 13, fontWeight: 500, color: '#fff', cursor: 'pointer', fontFamily: 'Geist, sans-serif', opacity: createConn.isPending ? 0.7 : 1 }}
            >
              {createConn.isPending ? 'Saving…' : 'Save & connect'}
            </button>
          </>
        )}
      </div>
    </OverlaySheet>
  );
}
