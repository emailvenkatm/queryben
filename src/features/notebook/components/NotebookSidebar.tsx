import { useNotebookList } from '../hooks/use-notebook';

interface Props {
  selectedId: string | null;
  onSelect: (id: string) => void;
  onCreate: () => void;
}

function formatWhen(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return '';
  return d.toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' });
}

export function NotebookSidebar({ selectedId, onSelect, onCreate }: Props) {
  const { data, isLoading } = useNotebookList();
  const notebooks = data ?? [];

  return (
    <aside
      aria-label="Notebook list"
      style={{
        width: 240,
        borderRight: '1px solid rgba(26,46,42,0.10)',
        background: 'var(--color-bg-elevated)',
        display: 'flex',
        flexDirection: 'column',
      }}
    >
      <div
        style={{
          padding: '10px 12px',
          borderBottom: '1px solid rgba(26,46,42,0.10)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
        }}
      >
        <span
          style={{
            fontSize: 12,
            fontWeight: 600,
            color: 'var(--color-text-muted)',
            letterSpacing: '0.05em',
            textTransform: 'uppercase',
            fontFamily: 'Geist, sans-serif',
          }}
        >
          Notebooks
        </span>
        <button
          type="button"
          onClick={onCreate}
          aria-label="New notebook"
          style={{
            border: 'none',
            background: 'var(--color-accent)',
            color: '#fff',
            width: 22,
            height: 22,
            borderRadius: 4,
            cursor: 'pointer',
            fontSize: 14,
            lineHeight: '20px',
            padding: 0,
          }}
        >
          +
        </button>
      </div>

      <div style={{ flex: 1, overflow: 'auto' }}>
        {isLoading && (
          <div style={{ padding: 12, fontSize: 12, color: 'var(--color-text-muted)' }}>
            Loading…
          </div>
        )}
        {!isLoading && notebooks.length === 0 && (
          <div
            style={{
              padding: 16,
              fontSize: 12,
              color: 'var(--color-text-muted)',
              lineHeight: 1.5,
              fontFamily: 'Geist, sans-serif',
            }}
          >
            No notebooks yet. Use <strong>+</strong> to make one.
          </div>
        )}
        {notebooks.map((nb) => (
          <button
            type="button"
            key={nb.id}
            onClick={() => onSelect(nb.id)}
            style={{
              display: 'block',
              width: '100%',
              textAlign: 'left',
              padding: '8px 12px',
              border: 'none',
              background: selectedId === nb.id ? 'rgba(42,87,81,0.10)' : 'transparent',
              cursor: 'pointer',
              borderBottom: '1px solid rgba(26,46,42,0.04)',
              fontFamily: 'Geist, sans-serif',
            }}
          >
            <div
              style={{
                fontSize: 13,
                fontWeight: 500,
                color: 'var(--color-text)',
                overflow: 'hidden',
                textOverflow: 'ellipsis',
                whiteSpace: 'nowrap',
              }}
            >
              {nb.name}
            </div>
            {nb.modifiedAt && (
              <div
                style={{
                  fontSize: 10,
                  color: 'var(--color-text-muted)',
                  fontFamily: 'Geist Mono, monospace',
                  marginTop: 2,
                }}
              >
                {formatWhen(nb.modifiedAt)}
              </div>
            )}
          </button>
        ))}
      </div>
    </aside>
  );
}
