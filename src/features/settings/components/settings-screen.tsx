import { useUiPrefsStore } from '@/shared/stores/ui-prefs';
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
  const prefs = useUiPrefsStore((s) => s.prefs);
  const update = useUiPrefsStore((s) => s.updatePrefs);

  return (
    <div style={{ height: '100%', overflowY: 'auto', background: 'var(--color-bg)' }}>
      <div style={{ padding: '32px 48px', maxWidth: 760 }}>
        <Section id="theme" title="Theme" desc="Pick a palette. Switching is instant — every color in the app resolves through it.">
          <PaletteCustomizer />
        </Section>

        <Section id="general" title="General">
          <GeneralPrefs prefs={prefs} update={update} />
        </Section>
      </div>
    </div>
  );
}
