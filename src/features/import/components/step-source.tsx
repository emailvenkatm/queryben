import type { ImportFormat } from '../types';

interface Props {
  path: string | null;
  format: ImportFormat;
  isLoading: boolean;
  onPick: () => void;
}

export function StepSource({ path, format, isLoading, onPick }: Props) {
  return (
    <div>
      <p style={{ fontSize: 12, color: 'var(--color-primary)', margin: '0 0 12px' }}>
        Pick a CSV or JSON file. Column types are inferred from the first rows; adjust them on the mapping step.
      </p>
      <button
        type="button"
        onClick={onPick}
        disabled={isLoading}
        style={{
          width: '100%', padding: '32px 16px',
          border: '1.5px dashed rgba(42,87,81,0.25)',
          background: 'transparent', borderRadius: 10,
          cursor: isLoading ? 'not-allowed' : 'pointer',
          color: 'var(--color-primary)', fontFamily: 'Geist, sans-serif', fontSize: 13,
        }}
      >
        {isLoading ? 'Reading preview…' : (path ?? 'Click to choose a file (.csv or .json)')}
      </button>
      {path && !isLoading && (
        <div style={{ marginTop: 10, fontSize: 11, color: 'rgba(42,87,81,0.55)', fontFamily: 'Geist Mono, monospace' }}>
          Format: {format.toUpperCase()}
        </div>
      )}
    </div>
  );
}
