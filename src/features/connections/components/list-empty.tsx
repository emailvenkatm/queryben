interface ListEmptyProps {
  onAdd: () => void;
}

export function ListEmpty({ onAdd }: ListEmptyProps) {
  return (
    <div style={{ display: 'flex', height: '100%', alignItems: 'center', justifyContent: 'center', flexDirection: 'column', gap: 16 }}>
      <div style={{ textAlign: 'center' }}>
        <p style={{ fontSize: 15, fontWeight: 600, color: 'var(--color-text)', margin: '0 0 6px' }}>No connections yet</p>
        <p style={{ fontSize: 13, color: 'var(--color-text-muted)', margin: 0, maxWidth: 280 }}>
          Add a SQL Server or Azure SQL connection to get started.
        </p>
      </div>
      <button
        type="button"
        onClick={onAdd}
        style={{ background: 'var(--color-accent)', color: '#fff', fontSize: 13, fontWeight: 500, padding: '8px 20px', borderRadius: 8, border: 'none', cursor: 'pointer', fontFamily: 'Geist, sans-serif' }}
      >
        Add connection
      </button>
    </div>
  );
}
