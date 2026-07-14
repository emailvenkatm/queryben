// Fake macOS traffic lights. Real ones are provided by the OS when
// decorations = true, but we render our own so the title bar can host
// content on both platforms.
export function TitleBar(): React.ReactElement {
  return (
    <div
      style={{
        background: 'var(--color-bg-sidebar)',
        height: 40,
        display: 'flex',
        alignItems: 'center',
        padding: '0 16px',
        gap: 8,
        flexShrink: 0,
      }}
      aria-hidden="true"
    >
      <span style={{ width: 12, height: 12, borderRadius: '50%', background: '#ff5f57', flexShrink: 0 }} />
      <span style={{ width: 12, height: 12, borderRadius: '50%', background: '#febc2e', flexShrink: 0 }} />
      <span style={{ width: 12, height: 12, borderRadius: '50%', background: '#28c840', flexShrink: 0 }} />
      <div style={{ flex: 1 }} />
      <span style={{ fontSize: 12, color: 'rgba(244,239,231,0.4)', fontFamily: 'Geist Mono, monospace' }}>
        QueryBen
      </span>
      <div style={{ flex: 1 }} />
    </div>
  );
}
