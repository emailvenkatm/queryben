import { useMemo } from 'react';
import type { ObjectChange, SchemaObject } from '../types';

interface Props {
  change: ObjectChange | null;
  sourceLabel: string;
  targetLabel: string;
}

type DiffOp = 'equal' | 'add' | 'remove';
interface DiffLine { op: DiffOp; sourceLine: string | null; targetLine: string | null; }

function lineDiff(source: string, target: string): DiffLine[] {
  const src = source.split(/\r?\n/);
  const tgt = target.split(/\r?\n/);
  const rows: DiffLine[] = [];
  const m = src.length;
  const n = tgt.length;

  if (m * n > 2_000_000) {
    const max = Math.max(m, n);
    for (let i = 0; i < max; i++) {
      rows.push({ op: 'equal', sourceLine: src[i] ?? null, targetLine: tgt[i] ?? null });
    }
    return rows;
  }

  const dp = new Array<number>((m + 1) * (n + 1)).fill(0);
  const at = (i: number, j: number) => dp[i * (n + 1) + j] ?? 0;
  const set = (i: number, j: number, v: number) => { dp[i * (n + 1) + j] = v; };

  for (let i = m - 1; i >= 0; i--) {
    for (let j = n - 1; j >= 0; j--) {
      if (src[i] === tgt[j]) set(i, j, at(i + 1, j + 1) + 1);
      else set(i, j, Math.max(at(i + 1, j), at(i, j + 1)));
    }
  }

  let i = 0; let j = 0;
  while (i < m && j < n) {
    const s = src[i] ?? ''; const t = tgt[j] ?? '';
    if (s === t) { rows.push({ op: 'equal', sourceLine: s, targetLine: t }); i++; j++; }
    else if (at(i + 1, j) >= at(i, j + 1)) { rows.push({ op: 'add', sourceLine: s, targetLine: null }); i++; }
    else { rows.push({ op: 'remove', sourceLine: null, targetLine: t }); j++; }
  }
  while (i < m) { rows.push({ op: 'add', sourceLine: src[i] ?? '', targetLine: null }); i++; }
  while (j < n) { rows.push({ op: 'remove', sourceLine: null, targetLine: tgt[j] ?? '' }); j++; }
  return rows;
}

function renderBody(obj: SchemaObject | null): string {
  if (!obj) return '';
  if (obj.body != null) return obj.body;
  const lines: string[] = [];
  for (const col of obj.columns) {
    const flags: string[] = [];
    if (col.isIdentity) flags.push('IDENTITY');
    if (col.isComputed) flags.push('COMPUTED');
    flags.push(col.isNullable ? 'NULL' : 'NOT NULL');
    const def = col.defaultExpression ? ` DEFAULT ${col.defaultExpression}` : '';
    lines.push(`${col.name} ${col.sqlType} ${flags.join(' ')}${def}`.trim());
  }
  if (obj.indexes.length > 0) {
    lines.push('', '-- indexes');
    for (const idx of obj.indexes) {
      const flag = idx.isPrimaryKey ? 'PK' : idx.isUnique ? 'UNIQUE' : 'INDEX';
      lines.push(`${flag} ${idx.name} (${idx.columns.join(', ')})`);
    }
  }
  return lines.join('\n');
}

function LineRow({ line }: { line: DiffLine }) {
  const isAdd = line.op === 'add';
  const isRemove = line.op === 'remove';
  return (
    <div style={{ display: 'flex', fontFamily: 'Geist Mono, monospace', fontSize: 12 }}>
      <div style={{ flex: 1, minWidth: 0, background: isAdd ? 'rgba(42,87,81,0.08)' : 'transparent', padding: '1px 8px', whiteSpace: 'pre', overflow: 'hidden', textOverflow: 'ellipsis' }}>
        {isAdd ? <span style={{ color: 'var(--color-primary-hover, #1a2e2a)', marginRight: 6 }}>+</span> : line.sourceLine != null ? <span style={{ marginRight: 6, opacity: 0.3 }}>&nbsp;</span> : null}
        {line.sourceLine ?? ''}
      </div>
      <div style={{ width: 1, background: 'var(--color-border, rgba(0,0,0,0.08))' }} />
      <div style={{ flex: 1, minWidth: 0, background: isRemove ? 'rgba(192,57,43,0.06)' : 'transparent', padding: '1px 8px', whiteSpace: 'pre', overflow: 'hidden', textOverflow: 'ellipsis' }}>
        {isRemove ? <span style={{ color: 'var(--color-error, #c0392b)', marginRight: 6 }}>-</span> : line.targetLine != null ? <span style={{ marginRight: 6, opacity: 0.3 }}>&nbsp;</span> : null}
        {line.targetLine ?? ''}
      </div>
    </div>
  );
}

export function ObjectDiffPanel({ change, sourceLabel, targetLabel }: Props) {
  const rows = useMemo<DiffLine[]>(() => {
    if (!change) return [];
    return lineDiff(renderBody(change.source), renderBody(change.target));
  }, [change]);

  if (!change) {
    return (
      <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', gap: 12, height: '100%', padding: '32px 24px', background: 'var(--color-bg)', textAlign: 'center' }}>
        <p style={{ fontSize: 13, fontWeight: 500, color: 'var(--color-text)', margin: 0, fontFamily: 'Geist, sans-serif' }}>Nothing selected</p>
        <p style={{ fontSize: 12, color: 'var(--color-text-muted)', margin: 0, lineHeight: 1.5, maxWidth: 260, fontFamily: 'Geist, sans-serif' }}>
          Select an object from the diff tree to see its details.
        </p>
      </div>
    );
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', overflow: 'hidden', background: 'var(--color-bg)' }}>
      <header style={{ padding: '10px 16px', borderBottom: '1px solid rgba(26,46,42,0.08)', background: 'var(--color-bg)' }}>
        <div style={{ display: 'flex', gap: 8, alignItems: 'baseline' }}>
          <span style={{ fontFamily: 'Geist Mono, monospace', fontSize: 13, fontWeight: 600, color: 'var(--color-text)' }}>{change.qualifiedName}</span>
          <span style={{ fontSize: 11, color: 'var(--color-text-muted)', textTransform: 'uppercase', letterSpacing: '0.05em' }}>{change.kind}</span>
        </div>
        {change.reasons.length > 0 && (
          <ul style={{ margin: '6px 0 0', padding: 0, listStyle: 'none', display: 'flex', flexWrap: 'wrap', gap: 6 }}>
            {change.reasons.map((r) => (
              <li key={r} style={{ fontSize: 11, padding: '2px 8px', background: 'rgba(213,138,74,0.12)', color: 'var(--color-accent, #D58A4A)', borderRadius: 3, fontFamily: 'Geist Mono, monospace' }}>
                {r}
              </li>
            ))}
          </ul>
        )}
      </header>
      <div style={{ display: 'flex', padding: '4px 8px', fontSize: 11, fontFamily: 'Geist Mono, monospace', color: 'var(--color-text-muted)', background: 'var(--color-bg-elevated)', borderBottom: '1px solid rgba(26,46,42,0.08)' }}>
        <div style={{ flex: 1, padding: '0 8px' }}>SOURCE - {sourceLabel}</div>
        <div style={{ flex: 1, padding: '0 8px' }}>TARGET - {targetLabel}</div>
      </div>
      <div style={{ flex: 1, overflow: 'auto' }}>
        {rows.length === 0 ? (
          <div style={{ padding: 20, color: 'var(--color-text-muted)', fontSize: 12, fontFamily: 'Geist Mono, monospace' }}>(no body captured)</div>
        ) : (
          rows.map((row, idx) => <LineRow key={idx} line={row} />)
        )}
      </div>
    </div>
  );
}
