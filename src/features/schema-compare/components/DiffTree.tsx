import { useMemo, useState } from 'react';
import type { ObjectChange, ObjectKind, SchemaDiff } from '../types';

type Category = 'added' | 'dropped' | 'changed';

interface Props {
  diff: SchemaDiff;
  selectedKey: string | null;
  onSelect: (change: ObjectChange, cat: Category) => void;
}

const META: Record<Category, { label: string; color: string; bg: string }> = {
  added: { label: 'Added', color: 'var(--color-primary-hover, #1a2e2a)', bg: 'rgba(42,87,81,0.10)' },
  dropped: { label: 'Dropped', color: 'var(--color-error, #c0392b)', bg: 'rgba(192,57,43,0.08)' },
  changed: { label: 'Changed', color: 'var(--color-accent, #D58A4A)', bg: 'rgba(213,138,74,0.10)' },
};

const KIND_LABEL: Record<ObjectKind, string> = {
  table: 'Tables', view: 'Views', procedure: 'Procedures', function: 'Functions', index: 'Indexes',
};

function groupByKind(changes: ObjectChange[]): Map<ObjectKind, ObjectChange[]> {
  const out = new Map<ObjectKind, ObjectChange[]>();
  for (const c of changes) {
    const bucket = out.get(c.kind);
    if (bucket) bucket.push(c);
    else out.set(c.kind, [c]);
  }
  return out;
}

export function DiffTree({ diff, selectedKey, onSelect }: Props) {
  const grouped = useMemo(() => ({
    added: groupByKind(diff.added),
    dropped: groupByKind(diff.dropped),
    changed: groupByKind(diff.changed),
  }), [diff]);

  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());

  function toggle(key: string) {
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  }

  const total = diff.added.length + diff.dropped.length + diff.changed.length;
  if (total === 0) {
    return (
      <div style={{ padding: 32, color: 'var(--color-text-muted)', fontSize: 13, textAlign: 'center', fontFamily: 'Geist, sans-serif' }}>
        Schemas are identical
        {diff.unchangedCount > 0 ? ` (${diff.unchangedCount} object${diff.unchangedCount === 1 ? '' : 's'})` : ''}.
      </div>
    );
  }

  const cats: Category[] = ['added', 'changed', 'dropped'];

  return (
    <div role="tree" aria-label="Schema differences" style={{ fontFamily: 'Geist, sans-serif', fontSize: 13, padding: '8px 0', overflowY: 'auto' }}>
      {cats.map((cat) => {
        const byKind = grouped[cat];
        if (byKind.size === 0) return null;
        const meta = META[cat];
        const count = Array.from(byKind.values()).reduce((s, a) => s + a.length, 0);

        return (
          <div key={cat} style={{ marginBottom: 6 }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '4px 12px', fontSize: 11, fontFamily: 'Geist Mono, monospace', textTransform: 'uppercase', letterSpacing: '0.06em', color: 'var(--color-text-muted)' }}>
              <span style={{ color: meta.color }}>{meta.label}</span>
              <span>({count})</span>
            </div>
            {Array.from(byKind.entries()).map(([kind, items]) => {
              const gk = `${cat}:${kind}`;
              const isCollapsed = collapsed.has(gk);
              return (
                <div key={gk}>
                  <button
                    type="button"
                    onClick={() => toggle(gk)}
                    aria-expanded={!isCollapsed}
                    style={{ display: 'flex', alignItems: 'center', gap: 6, width: '100%', padding: '3px 12px 3px 20px', background: 'none', border: 'none', cursor: 'pointer', color: 'var(--color-text)', fontSize: 12, fontFamily: 'Geist, sans-serif', textAlign: 'left' }}
                  >
                    <span style={{ opacity: 0.5, fontSize: 9 }}>{isCollapsed ? '>' : 'v'}</span>
                    <span style={{ fontWeight: 500 }}>{KIND_LABEL[kind]}</span>
                    <span style={{ color: 'var(--color-text-muted)', fontSize: 11 }}>({items.length})</span>
                  </button>
                  {!isCollapsed && (
                    <div role="group">
                      {items.map((change) => {
                        const key = `${cat}:${change.qualifiedName}`;
                        const sel = selectedKey === key;
                        return (
                          <button
                            type="button"
                            key={key}
                            role="treeitem"
                            aria-selected={sel}
                            onClick={() => onSelect(change, cat)}
                            style={{
                              display: 'flex', alignItems: 'center', gap: 8, width: '100%',
                              padding: '4px 12px 4px 34px',
                              background: sel ? 'var(--color-bg-elevated, rgba(0,0,0,0.04))' : 'transparent',
                              border: 'none', borderLeft: `2px solid ${sel ? meta.color : 'transparent'}`,
                              cursor: 'pointer', color: 'var(--color-text)', fontSize: 12,
                              fontFamily: 'Geist Mono, monospace', textAlign: 'left',
                            }}
                          >
                            <span style={{ fontSize: 10, padding: '1px 6px', borderRadius: 3, color: meta.color, background: meta.bg, textTransform: 'uppercase', letterSpacing: '0.03em' }}>
                              {meta.label.slice(0, 3)}
                            </span>
                            <span style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                              {change.qualifiedName}
                            </span>
                          </button>
                        );
                      })}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        );
      })}
      {diff.unchangedCount > 0 && (
        <div style={{ padding: '8px 12px', fontSize: 11, color: 'var(--color-text-muted)', fontFamily: 'Geist Mono, monospace', borderTop: '1px solid var(--color-border, rgba(0,0,0,0.08))', marginTop: 8 }}>
          {diff.unchangedCount} identical object{diff.unchangedCount === 1 ? '' : 's'}
        </div>
      )}
    </div>
  );
}
