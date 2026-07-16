import { useEffect, useState } from 'react';
import { Sheet, SheetContent } from '@/shared/ui/sheet';
import { NicknameColorFields } from '@/shared/ui/color-tag';
import { useUpdateConnection } from '../api';
import { formatAppErrorForDisplay } from '@/shared/api/tauri-bindings';
import type { Connection, ConnectionColor } from '@/shared/types';

interface EditSheetProps {
  connection: Connection | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function EditSheet({ connection, open, onOpenChange }: EditSheetProps) {
  const [nickname, setNickname] = useState('');
  const [color, setColor] = useState<ConnectionColor | null>(null);
  const [error, setError] = useState<string | null>(null);
  const updateConn = useUpdateConnection();

  useEffect(() => {
    if (!open || !connection) return;
    setNickname(connection.nickname ?? '');
    setColor(connection.color ?? null);
    setError(null);
  }, [open, connection]);

  const handleSave = async (): Promise<void> => {
    if (!connection) return;
    setError(null);
    const trimmed = nickname.trim();
    try {
      await updateConn.mutateAsync({
        id: connection.id,
        nickname: trimmed.length > 0 ? trimmed : null,
        color: color ?? null,
        clearNickname: trimmed.length === 0,
        clearColor: color === null,
      });
      onOpenChange(false);
    } catch (err) {
      setError(formatAppErrorForDisplay(err));
    }
  };

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent
        className="p-0 flex flex-col gap-0"
        style={{ width: 420, maxWidth: 420, background: 'var(--color-bg-elevated)', borderLeft: '1px solid rgba(26,46,42,0.10)', boxShadow: '-8px 0 32px rgba(26,46,42,0.08)' }}
        aria-label="Edit connection labels"
      >
        <div style={{ padding: '24px 28px 20px', borderBottom: '1px solid rgba(26,46,42,0.08)' }}>
          <h2 style={{ fontSize: 16, fontWeight: 600, color: 'var(--color-text)', letterSpacing: '-0.02em', margin: 0 }}>
            Edit connection
          </h2>
          {connection && (
            <p style={{ fontSize: 12, color: 'var(--color-text-muted)', margin: '3px 0 0', fontFamily: 'Geist Mono, monospace' }}>
              {connection.server}
            </p>
          )}
        </div>

        <div style={{ flex: 1, overflowY: 'auto', padding: '24px 28px' }}>
          <NicknameColorFields
            nickname={nickname}
            color={color}
            onNicknameChange={setNickname}
            onColorChange={setColor}
          />
          {error && (
            <div style={{ marginTop: 12, fontSize: 12, color: 'var(--color-error)' }} role="alert">
              {error}
            </div>
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
          <button
            type="button"
            onClick={() => { void handleSave(); }}
            disabled={updateConn.isPending}
            style={{ background: 'var(--color-accent)', border: 'none', borderRadius: 8, padding: '9px 20px', fontSize: 13, fontWeight: 500, color: '#fff', cursor: 'pointer', fontFamily: 'Geist, sans-serif', opacity: updateConn.isPending ? 0.7 : 1 }}
          >
            {updateConn.isPending ? 'Saving…' : 'Save'}
          </button>
        </div>
      </SheetContent>
    </Sheet>
  );
}
