import { useThemeStore } from '@/shared/stores/theme';
import { PRESETS } from '@/shared/theme/presets';

export function PaletteCustomizer() {
  const { paletteId, setPreset } = useThemeStore((s) => ({
    paletteId: s.paletteId,
    setPreset: s.setPreset,
  }));

  return (
    <section style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
      <h3
        style={{
          fontSize: 12,
          fontFamily: 'Geist Mono, monospace',
          textTransform: 'uppercase',
          letterSpacing: '0.08em',
          color: 'var(--color-text-muted)',
          margin: 0,
        }}
      >
        Color palette
      </h3>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
        {PRESETS.map((preset) => {
          const active = preset.id === paletteId;
          return (
            <button
              key={preset.id}
              type="button"
              onClick={() => setPreset(preset.id)}
              aria-pressed={active}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 10,
                padding: '8px 12px',
                borderRadius: 8,
                border: active
                  ? '1.5px solid var(--color-accent)'
                  : '1px solid rgba(26,46,42,0.12)',
                background: active ? 'rgba(213,138,74,0.06)' : 'var(--color-bg-elevated)',
                cursor: 'pointer',
                textAlign: 'left',
                fontFamily: 'Geist, sans-serif',
              }}
            >
              <SwatchRow colors={[preset.colors.bg, preset.colors.bgSidebar, preset.colors.accent, preset.colors.primary]} />
              <div style={{ flex: 1, minWidth: 0 }}>
                <div style={{ fontSize: 13, fontWeight: 500, color: 'var(--color-text)', lineHeight: 1.2 }}>
                  {preset.name}
                </div>
              </div>
              {active && (
                <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true">
                  <path d="M2.5 7.5l3 3 6-6" stroke="var(--color-accent)" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
                </svg>
              )}
            </button>
          );
        })}
      </div>
    </section>
  );
}

function SwatchRow({ colors }: { colors: string[] }) {
  return (
    <div style={{ display: 'flex', gap: 3, flexShrink: 0 }}>
      {colors.map((c, i) => (
        <div
          key={i}
          style={{ width: 14, height: 14, borderRadius: 3, background: c, border: '1px solid rgba(0,0,0,0.08)' }}
          aria-hidden="true"
        />
      ))}
    </div>
  );
}
