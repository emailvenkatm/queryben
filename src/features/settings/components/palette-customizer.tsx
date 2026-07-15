// TODO: depends on shared/stores/theme.ts + shared/theme/presets.ts (shell agent territory).
// Wire up once those are in place.

// eslint-disable-next-line @typescript-eslint/no-unused-vars
export function PaletteCustomizer(_props: Record<string, unknown>): React.ReactElement {
  return (
    <section style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
      <h3 style={{ fontSize: 12, fontFamily: 'Geist Mono, monospace', textTransform: 'uppercase', letterSpacing: '0.08em', color: 'var(--color-text-muted)', margin: 0 }}>Color palette</h3>
      <div style={{ padding: 16, borderRadius: 8, border: '1px dashed rgba(26,46,42,0.2)', color: 'var(--color-text-muted)', fontSize: 12, fontFamily: 'Geist, sans-serif', lineHeight: 1.5 }}>
        Palette customizer is wired to <code style={{ fontFamily: 'Geist Mono, monospace' }}>shared/stores/theme.ts</code>.<br />
        Available after the shell layer ships that store.
      </div>
    </section>
  );
}
