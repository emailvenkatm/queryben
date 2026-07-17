import { useEffect, useMemo, useRef, useState } from 'react';
import { useSavedList, useSaveQuery } from '../hooks/use-saved-queries';

interface SaveQueryDialogProps {
  open: boolean;
  sql: string;
  connectionId: string | null;
  onClose: () => void;
  onSaved?: () => void;
}

const labelStyle: React.CSSProperties = { display: 'flex', flexDirection: 'column', gap: 5, fontSize: 11, color: 'var(--color-text-muted)', fontFamily: 'Geist, sans-serif', marginBottom: 12, fontWeight: 500 };
const inputStyle: React.CSSProperties = { padding: '7px 10px', border: '1px solid rgba(26,46,42,0.15)', borderRadius: 5, fontSize: 13, background: 'var(--color-bg)', color: 'var(--color-text)', fontFamily: 'Geist, sans-serif', outline: 'none' };

export function SaveQueryDialog({ open, sql, connectionId, onClose, onSaved }: SaveQueryDialogProps) {
  const [name, setName] = useState('');
  const [folder, setFolder] = useState('');
  const [error, setError] = useState<string | null>(null);
  const nameInputRef = useRef<HTMLInputElement>(null);
  const { data: existing } = useSavedList({});
  const saveMutation = useSaveQuery();

  const folders = useMemo(() => {
    const set = new Set<string>(['General']);
    for (const q of existing ?? []) set.add(q.folder);
    return Array.from(set).sort((a, b) => a.localeCompare(b));
  }, [existing]);

  useEffect(() => {
    if (!open) return;
    setName('');
    setFolder('');
    setError(null);
    requestAnimationFrame(() => nameInputRef.current?.focus());
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent): void => { if (e.key === 'Escape') onClose(); };
    document.addEventListener('keydown', handler);
    return () => document.removeEventListener('keydown', handler);
  }, [open, onClose]);

  if (!open) return null;

  const submit = async (): Promise<void> => {
    const trimmed = name.trim();
    if (!trimmed) { setError('Give it a name so you can find it later.'); nameInputRef.current?.focus(); return; }
    if (!sql.trim()) { setError("There's nothing in the editor to save yet."); return; }
    try {
      await saveMutation.mutateAsync({ name: trimmed, folder: folder.trim() || undefined, sql, connectionId });
      onSaved?.();
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : typeof err === 'string' ? err : 'Save failed.');
    }
  };

  return (
    <div role="dialog" aria-modal="true" aria-label="Save query" style={{ position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.35)', display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 1000 }} onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}>
      <div style={{ width: 420, background: 'var(--color-bg-elevated)', borderRadius: 10, padding: 20, boxShadow: '0 10px 40px rgba(0,0,0,0.35)', border: '1px solid rgba(26,46,42,0.15)' }} onClick={(e) => e.stopPropagation()}>
        <h2 style={{ fontSize: 15, fontWeight: 600, margin: 0, marginBottom: 12, color: 'var(--color-text)', fontFamily: 'Geist, sans-serif' }}>Save query</h2>

        <label style={labelStyle}>
          Name
          <input ref={nameInputRef} type="text" value={name} onChange={(e) => setName(e.target.value)} onKeyDown={(e) => { if (e.key === 'Enter') { e.preventDefault(); void submit(); } }} placeholder="e.g. Top 100 users by revenue" style={inputStyle} />
        </label>

        <label style={labelStyle}>
          Folder <span style={{ color: 'var(--color-text-muted)', fontSize: 10, fontWeight: 400, marginLeft: 4 }}>(optional)</span>
          <input list="save-query-folders" type="text" value={folder} onChange={(e) => setFolder(e.target.value)} placeholder="General" style={inputStyle} />
          <datalist id="save-query-folders">
            {folders.map((f) => <option key={f} value={f} />)}
          </datalist>
        </label>

        {error && (
          <div role="alert" style={{ marginTop: 6, padding: '6px 10px', background: 'rgba(220,38,38,0.08)', border: '1px solid rgba(220,38,38,0.25)', color: 'var(--color-error)', borderRadius: 5, fontSize: 12, fontFamily: 'Geist, sans-serif' }}>
            {error}
          </div>
        )}

        <div style={{ display: 'flex', justifyContent: 'flex-end', gap: 8, marginTop: 16 }}>
          <button type="button" onClick={onClose} style={{ padding: '6px 14px', background: 'transparent', border: '1px solid rgba(26,46,42,0.15)', borderRadius: 6, fontSize: 12, fontWeight: 500, color: 'var(--color-text)', cursor: 'pointer', fontFamily: 'Geist, sans-serif' }}>Cancel</button>
          <button type="button" onClick={() => void submit()} disabled={saveMutation.isPending} style={{ padding: '6px 14px', background: saveMutation.isPending ? 'rgba(42,87,81,0.45)' : 'var(--color-primary)', color: '#fff', border: 'none', borderRadius: 6, fontSize: 12, fontWeight: 600, cursor: saveMutation.isPending ? 'default' : 'pointer', fontFamily: 'Geist, sans-serif' }}>
            {saveMutation.isPending ? 'Saving…' : 'Save'}
          </button>
        </div>
      </div>
    </div>
  );
}
