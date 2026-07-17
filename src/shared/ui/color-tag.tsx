import {
  CONNECTION_COLORS,
  CONNECTION_COLOR_HEX,
  NICKNAME_MAX_LEN,
  type ConnectionColor,
} from '@/shared/types';

interface NicknameColorFieldsProps {
  nickname: string;
  color: ConnectionColor | null;
  onNicknameChange: (value: string) => void;
  onColorChange: (value: ConnectionColor | null) => void;
  compact?: boolean;
}

const fieldLabel: React.CSSProperties = {
  fontSize: 12,
  fontWeight: 500,
  color: 'var(--color-text)',
  marginBottom: 6,
  display: 'block',
  letterSpacing: '0.005em',
};

const inputStyle: React.CSSProperties = {
  width: '100%',
  padding: '9px 12px',
  fontSize: 13,
  fontFamily: 'Geist, sans-serif',
  background: 'var(--color-bg)',
  border: '1px solid rgba(26,46,42,0.15)',
  borderRadius: 8,
  color: 'var(--color-text)',
  outline: 'none',
  boxSizing: 'border-box',
};

export function NicknameColorFields({
  nickname,
  color,
  onNicknameChange,
  onColorChange,
  compact,
}: NicknameColorFieldsProps) {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: compact ? 10 : 14 }}>
      <div>
        <label htmlFor="conn-nickname" style={fieldLabel}>
          Nickname (optional)
        </label>
        <input
          id="conn-nickname"
          type="text"
          value={nickname}
          onChange={(e) => onNicknameChange(e.target.value.slice(0, NICKNAME_MAX_LEN))}
          placeholder="e.g. Prod · East US"
          maxLength={NICKNAME_MAX_LEN}
          autoComplete="off"
          style={inputStyle}
        />
      </div>
      <div>
        <label style={fieldLabel}>Color tag</label>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          {CONNECTION_COLORS.map((c) => {
            const selected = color === c;
            return (
              <button
                key={c}
                type="button"
                onClick={() => onColorChange(selected ? null : c)}
                aria-label={`${c}${selected ? ' (selected)' : ''}`}
                aria-pressed={selected}
                title={c.charAt(0).toUpperCase() + c.slice(1)}
                style={{
                  width: 22,
                  height: 22,
                  padding: 0,
                  borderRadius: '50%',
                  background: CONNECTION_COLOR_HEX[c],
                  border: selected ? '2px solid var(--color-text)' : '1px solid rgba(26,46,42,0.20)',
                  boxShadow: selected ? '0 0 0 2px var(--color-bg)' : 'none',
                  cursor: 'pointer',
                  outline: 'none',
                  transition: 'transform 100ms',
                  transform: selected ? 'scale(1.05)' : 'scale(1)',
                }}
              />
            );
          })}
          {color && (
            <button
              type="button"
              onClick={() => onColorChange(null)}
              style={{
                background: 'transparent',
                border: 'none',
                color: 'var(--color-text-muted)',
                fontSize: 11,
                cursor: 'pointer',
                padding: '2px 6px',
                marginLeft: 4,
                fontFamily: 'Geist, sans-serif',
              }}
            >
              Clear
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

interface ConnectionDotProps {
  color: ConnectionColor | null | undefined;
  size?: number;
}

export function ConnectionDot({ color, size = 8 }: ConnectionDotProps) {
  if (!color) return null;
  return (
    <span
      aria-hidden="true"
      style={{
        display: 'inline-block',
        width: size,
        height: size,
        borderRadius: '50%',
        background: CONNECTION_COLOR_HEX[color],
        border: '1px solid rgba(26,46,42,0.15)',
        flexShrink: 0,
      }}
    />
  );
}
