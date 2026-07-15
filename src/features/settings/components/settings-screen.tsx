import { useUiPrefs } from '../hooks/use-ui-prefs';
import { GeneralPrefs } from './general-prefs';
import { PaletteCustomizer } from './palette-customizer';

function Section({ id, title, desc, children }: { id: string; title: string; desc?: string; children: React.ReactNode }) {
  return (
    <section aria-labelledby={id} style={{ marginBottom: 40 }}>
      <div id={id} style={{ fontSize: 14, fontWeight: 600, color: 'var(--color-text)', letterSpacing: '-0.01em', marginBottom: desc ? 4 : 16 }}>
        {title}
      </div>
      {desc && <div style={{ fontSize: 12, color: 'var(--color-text-muted)', marginBottom: 20 }}>{desc}</div>}
      {children}
    </section>
  );
}

export function SettingsScreen() {
  const { prefs, update } = useUiPrefs();

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', background: 'var(--color-bg)' }}>
      <div style={{ flex: 1, overflow: 'hidden', display: 'flex' }}>
        <nav
          aria-label="Settings navigation"
          style={{ width: 200, flexShrink: 0, borderRight: '1px solid rgba(26,46,42,0.08)', padding: '16px 0', overflowY: 'auto', background: 'var(--color-bg-elevated)' }}
        >
          <div style={{ fontSize: 10, fontWeight: 700, color: 'var(--color-text-muted)', textTransform: 'uppercase', letterSpacing: '0.07em', padding: '10px 16px 4px' }}>
            Preferences
          </div>
          <NavItem label="General" active />
          <NavItem label="Editor" />
          <NavItem label="Connections" />
          <div style={{ fontSize: 10, fontWeight: 700, color: 'var(--color-text-muted)', textTransform: 'uppercase', letterSpacing: '0.07em', padding: '18px 16px 4px' }}>
            Tools
          </div>
          <NavItem label="Keyboard shortcuts" />
          <NavItem label="Updates" />
          <NavItem label="About" />
        </nav>

        <div style={{ flex: 1, overflowY: 'auto', padding: '32px 48px', maxWidth: 760 }}>
          <Section id="theme" title="Theme" desc="Pick a palette. Switching is instant — every color in the app resolves through it.">
            <PaletteCustomizer presetIdForReset="default" />
          </Section>

          <Section id="general" title="General">
            <GeneralPrefs prefs={prefs} update={update} />
          </Section>
        </div>
      </div>
    </div>
  );
}

function NavItem({ label, active }: { label: string; active?: boolean }) {
  return (
    <button
      type="button"
      style={{
        display: 'flex', alignItems: 'center', gap: 9, padding: '7px 16px', fontSize: 13,
        color: active ? 'var(--color-text)' : 'var(--color-text-muted)',
        cursor: 'pointer', borderRadius: 0, position: 'relative',
        background: active ? 'rgba(26,46,42,0.07)' : 'transparent',
        fontWeight: active ? 500 : 400, border: 'none', width: '100%',
        textAlign: 'left', fontFamily: 'Geist, sans-serif',
      }}
    >
      {active && <span style={{ position: 'absolute', left: 0, top: 5, bottom: 5, width: 2, background: 'var(--color-accent)', borderRadius: 1 }} aria-hidden />}
      {label}
    </button>
  );
}
