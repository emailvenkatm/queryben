interface FirewallScopePickerProps {
  ip: string;
  subnetStart: string;
  subnetEnd: string;
  useSubnet: boolean;
  disabled: boolean;
  onChange: (useSubnet: boolean) => void;
}

const JADE = 'var(--color-primary)';
const mix = (color: string, pct: number) => `color-mix(in srgb, ${color} ${pct}%, transparent)`;

export function FirewallScopePicker({
  ip,
  subnetStart,
  subnetEnd,
  useSubnet,
  disabled,
  onChange,
}: FirewallScopePickerProps) {
  const codeStyle = {
    fontFamily: 'Geist Mono, monospace',
    background: mix(JADE, 6),
    padding: '0 4px',
    borderRadius: 3,
    fontSize: 11,
  };

  return (
    <div
      role="radiogroup"
      aria-label="Firewall rule scope"
      style={{ marginTop: 12, display: 'flex', flexDirection: 'column', gap: 6, fontSize: 12, color: JADE, lineHeight: 1.4 }}
    >
      <label style={{ display: 'flex', alignItems: 'center', gap: 8, cursor: disabled ? 'not-allowed' : 'pointer', opacity: disabled ? 0.6 : 1 }}>
        <input
          type="radio"
          name="firewall-scope"
          checked={!useSubnet}
          disabled={disabled}
          onChange={() => onChange(false)}
          style={{ accentColor: JADE, margin: 0 }}
        />
        <span>
          Add my client IP (<code style={codeStyle}>{ip}</code>)
        </span>
      </label>
      <label style={{ display: 'flex', alignItems: 'center', gap: 8, cursor: disabled ? 'not-allowed' : 'pointer', opacity: disabled ? 0.6 : 1 }}>
        <input
          type="radio"
          name="firewall-scope"
          checked={useSubnet}
          disabled={disabled}
          onChange={() => onChange(true)}
          style={{ accentColor: JADE, margin: 0 }}
        />
        <span>
          Add my subnet (<code style={codeStyle}>{subnetStart} – {subnetEnd}</code>)
        </span>
      </label>
    </div>
  );
}
