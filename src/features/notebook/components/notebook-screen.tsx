import { useActiveConnectionStore } from '@/shared/stores/active-connection';
import { useConnections } from '@/features/connections';
import { useNotebookScreen } from '../hooks/use-notebook-screen';
import { NotebookCell } from './NotebookCell';
import { NotebookSidebar } from './NotebookSidebar';
import { NotebookToolbar } from './NotebookToolbar';

export function NotebookScreen() {
  const nb = useNotebookScreen();
  const activeConnectionId = useActiveConnectionStore((s) => s.activeConnectionId);
  const connectionsQuery = useConnections();
  const connections = (connectionsQuery.data ?? []).map((c) => ({ id: c.id, name: c.name }));
  const connectionId = nb.draft?.metadata.connectionId ?? activeConnectionId;

  return (
    <div style={{ display: 'flex', flex: 1, minHeight: 0, background: 'var(--color-bg)' }}>
      <NotebookSidebar
        selectedId={nb.selectedId}
        onSelect={nb.select}
        onCreate={() => nb.create(activeConnectionId)}
      />

      <div style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column' }}>
        {!nb.draft && (
          <div
            style={{
              display: 'flex',
              flex: 1,
              alignItems: 'center',
              justifyContent: 'center',
              color: 'var(--color-text-muted)',
              fontSize: 13,
              fontFamily: 'Geist, sans-serif',
            }}
          >
            Pick a notebook on the left, or start a new one with
            <strong style={{ margin: '0 4px' }}>+</strong>.
          </div>
        )}

        {nb.draft && (
          <>
            <NotebookToolbar
              title={nb.draft.metadata.title ?? nb.selectedId ?? 'Untitled'}
              onRename={(t) => void nb.rename(t)}
              renameBusy={nb.isRenaming}
              connections={connections}
              connectionId={connectionId}
              onConnectionChange={nb.setConnection}
              onSave={() => void nb.save()}
              isSaving={nb.isSaving}
              dirty={nb.dirty}
              saveToast={nb.saveToast}
            />
            <div
              style={{
                flex: 1,
                overflow: 'auto',
                padding: 20,
                display: 'flex',
                flexDirection: 'column',
                gap: 14,
              }}
            >
              {nb.draft.cells.map((cell, idx) => (
                <NotebookCell
                  key={cell.id}
                  cell={cell}
                  connectionId={connectionId}
                  index={idx}
                  onChange={(src) => nb.updateSource(idx, src)}
                  onDelete={() => nb.deleteCell(idx)}
                  onInsertBelow={(kind) => nb.insertBelow(idx, kind)}
                />
              ))}
            </div>
          </>
        )}
      </div>
    </div>
  );
}
