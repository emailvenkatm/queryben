import { lazy, Suspense } from 'react';
import { Loader2Icon } from 'lucide-react';
import type { DdlStatement } from '../types';

const Editor = lazy(() =>
  import('@monaco-editor/react').then((mod) => ({ default: mod.Editor })),
);

interface Props {
  statements: DdlStatement[];
  isGenerating: boolean;
}

export function combineSql(statements: DdlStatement[]): string {
  if (statements.length === 0) return '-- (no pending changes)';
  return `BEGIN TRANSACTION;\n\n${statements.map((s) => s.sql).join('\n\n')}\n\nCOMMIT;`;
}

export function DdlPreview({ statements, isGenerating }: Props): React.ReactElement {
  const sql = combineSql(statements);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', background: 'var(--color-bg-elevated)', borderTop: '1px solid rgba(26,46,42,0.08)' }}>
      <div style={{ padding: '6px 12px', fontSize: 10, fontFamily: 'Geist Mono, monospace', textTransform: 'uppercase', letterSpacing: '0.08em', color: 'var(--color-text-muted)', borderBottom: '1px solid rgba(26,46,42,0.05)', display: 'flex', alignItems: 'center', gap: 8 }}>
        <span>DDL preview</span>
        {isGenerating && <Loader2Icon className="h-3 w-3 animate-spin" aria-hidden="true" style={{ color: 'var(--color-text-muted)' }} />}
        <span style={{ marginLeft: 'auto' }}>{statements.length} statement{statements.length === 1 ? '' : 's'}</span>
      </div>
      <div style={{ flex: 1, minHeight: 0 }}>
        <Suspense fallback={<div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100%' }}><Loader2Icon className="h-4 w-4 animate-spin" aria-hidden="true" /></div>}>
          <Editor
            height="100%"
            language="sql"
            value={sql}
            options={{ readOnly: true, fontSize: 12, fontFamily: "'GeistMono', 'Cascadia Code', monospace", wordWrap: 'on', minimap: { enabled: false }, scrollBeyondLastLine: false, lineNumbers: 'on', automaticLayout: true, padding: { top: 8, bottom: 8 } }}
            theme="vs"
          />
        </Suspense>
      </div>
    </div>
  );
}
