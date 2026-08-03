import type { UiPrefs } from '@/shared/types';

interface Props {
  prefs: UiPrefs;
  update: (patch: Partial<UiPrefs>) => void;
}

function Toggle({ checked, onChange }: { checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      onClick={() => onChange(!checked)}
      style={{
        width: 38, height: 22, borderRadius: 11,
        background: checked ? 'var(--color-primary)' : 'rgba(26,46,42,0.15)',
        display: 'flex', alignItems: 'center', padding: 2, cursor: 'pointer',
        border: 'none', transition: 'background 150ms', flexShrink: 0,
      }}
    >
      <span style={{
        width: 18, height: 18, borderRadius: '50%',
        background: 'var(--color-bg-elevated)',
        boxShadow: '0 1px 3px rgba(0,0,0,0.15)',
        transition: 'transform 150ms',
        transform: checked ? 'translateX(16px)' : 'translateX(0)',
        display: 'block',
      }} />
    </button>
  );
}

function Row({ label, hint, control, last }: { label: string; hint?: string; control: React.ReactNode; last?: boolean }) {
  return (
    <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '12px 0', borderBottom: last ? 'none' : '1px solid rgba(26,46,42,0.06)', gap: 24 }}>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ fontSize: 13, fontWeight: 500, color: 'var(--color-text)' }}>{label}</div>
        {hint && <div style={{ fontSize: 11, color: 'var(--color-text-muted)', marginTop: 2, lineHeight: 1.4 }}>{hint}</div>}
      </div>
      <div style={{ flexShrink: 0 }}>{control}</div>
    </div>
  );
}

export function GeneralPrefs({ prefs, update }: Props) {
  return (
    <div>
      <Row
        label="Editor font size"
        control={
          <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
            <span style={{ fontSize: 11, color: 'var(--color-text-muted)' }}>11</span>
            <input
              type="range" min={11} max={20} value={prefs.editorFontSize}
              onChange={(e) => update({ editorFontSize: Number(e.target.value) })}
              style={{ appearance: 'none', width: 120, height: 4, borderRadius: 2, background: 'rgba(26,46,42,0.12)', outline: 'none', cursor: 'pointer' }}
            />
            <span style={{ fontSize: 11, color: 'var(--color-text-muted)' }}>20</span>
            <span style={{ fontSize: 12, fontFamily: 'Geist Mono, monospace', color: 'var(--color-text-muted)', minWidth: 24 }}>
              {prefs.editorFontSize}
            </span>
          </div>
        }
      />
      <Row
        label="Word wrap"
        hint="Wrap long lines instead of scrolling sideways"
        last
        control={<Toggle checked={prefs.editorWordWrap} onChange={(v) => update({ editorWordWrap: v })} />}
      />
    </div>
  );
}
