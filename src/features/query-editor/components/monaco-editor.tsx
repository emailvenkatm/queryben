import { lazy, Suspense, useEffect, useRef } from 'react';
import { useUiPrefsStore } from '@/shared/stores/ui-prefs';
import { useActiveConnectionStore } from '@/shared/stores/active-connection';
import { useSchemaSnapshot, updateSchemaSnapshot } from '../hooks/sql-intellisense/index';
import { registerSqlCompletions } from '../hooks/sql-intellisense/register-completions';

const Editor = lazy(() => import('@monaco-editor/react').then((m) => ({ default: m.Editor })));

interface MonacoEditorProps {
  value: string;
  onChange: (value: string) => void;
  onRun?: () => void;
}

export function MonacoEditor({ value, onChange, onRun }: MonacoEditorProps) {
  const prefs = useUiPrefsStore((s) => s.prefs);
  const editorRef = useRef<unknown>(null);
  const activeConnectionId = useActiveConnectionStore((s) => s.activeConnectionId);
  const snapshot = useSchemaSnapshot(activeConnectionId);

  useEffect(() => { updateSchemaSnapshot(snapshot); }, [snapshot]);

  const handleMount = (editor: unknown, monaco: unknown) => {
    editorRef.current = editor;
    if (monaco) registerSqlCompletions(monaco as Parameters<typeof registerSqlCompletions>[0]);
    if (onRun && editor && typeof editor === 'object' && 'addAction' in editor) {
      const e = editor as { addAction: (action: { id: string; label: string; keybindings: number[]; run: () => void }) => void };
      e.addAction({ id: 'queryben.run', label: 'Run Query', keybindings: [60, 2048 | 3], run: onRun });
    }
  };

  return (
    <Suspense
      fallback={
        <div style={{ display: 'flex', height: '100%', alignItems: 'center', justifyContent: 'center' }}>
          <span style={{ fontSize: 12, color: 'var(--color-text-muted)' }}>Loading editor…</span>
        </div>
      }
    >
      <Editor
        height="100%"
        language="sql"
        value={value}
        onChange={(v) => onChange(v ?? '')}
        onMount={handleMount}
        options={{
          fontSize: prefs.editorFontSize,
          fontFamily: "'GeistMono', 'Cascadia Code', monospace",
          wordWrap: prefs.editorWordWrap ? 'on' : 'off',
          minimap: { enabled: false },
          scrollBeyondLastLine: false,
          lineNumbers: 'on',
          renderLineHighlight: 'line',
          cursorBlinking: 'smooth',
          smoothScrolling: true,
          tabSize: 2,
          insertSpaces: true,
          formatOnPaste: true,
          automaticLayout: true,
          padding: { top: 12, bottom: 12 },
        }}
        theme="vs"
      />
    </Suspense>
  );
}
