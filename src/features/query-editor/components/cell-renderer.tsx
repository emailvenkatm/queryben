import type { CellValue, ColumnType } from '@/shared/types';

const TYPE_STYLES: Record<ColumnType, { bg: string; color: string; label: string }> = {
  number:   { bg: 'rgba(46,125,50,0.10)',   color: 'var(--color-success)', label: 'int' },
  string:   { bg: 'rgba(21,101,192,0.10)',  color: '#1565c0', label: 'nvc' },
  datetime: { bg: 'rgba(136,14,79,0.10)',   color: '#880e4f', label: 'dt' },
  boolean:  { bg: 'rgba(106,27,154,0.10)',  color: '#6a1b9a', label: 'bit' },
  null:     { bg: 'rgba(38,50,56,0.08)',    color: 'var(--color-text-muted)', label: '—' },
  unknown:  { bg: 'rgba(38,50,56,0.08)',    color: 'var(--color-text-muted)', label: '?' },
};

export function TypeBadge({ type }: { type: ColumnType }) {
  const s = TYPE_STYLES[type] ?? TYPE_STYLES.unknown;
  return (
    <span
      style={{ fontSize: 9, fontWeight: 600, padding: '1px 5px', borderRadius: 3, letterSpacing: '0.04em', marginLeft: 5, flexShrink: 0, background: s.bg, color: s.color }}
      aria-hidden="true"
    >
      {s.label}
    </span>
  );
}

export function CellDisplay({ value, type }: { value: CellValue; type?: ColumnType }) {
  if (value === null) {
    return <span style={{ color: 'var(--color-text-muted)', fontStyle: 'italic', fontWeight: 400 }} className="select-text">NULL</span>;
  }
  if (type === 'boolean') {
    const on = Boolean(value);
    return <span style={{ color: on ? 'var(--color-success)' : 'var(--color-text-muted)', fontWeight: on ? 500 : 400 }} className="select-text">{on ? '1' : '0'}</span>;
  }
  return <span className="select-text" style={{ fontFamily: 'Geist Mono, monospace' }}>{String(value)}</span>;
}
