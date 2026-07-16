import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { TreeNode, LeafNode } from './tree-node';
import { ObjectContextMenu } from './object-context-menu';
import { useActiveConnectionStore } from '@/shared/stores/active-connection';
import { useOpenTabsStore } from '@/shared/stores/open-tabs';
import type { SchemaNode } from '@/shared/types';
import type { ObjectContextTarget, ScriptAction } from '../types';

// TODO: wire to object-scripter feature when ported
type ScriptObjectKind = 'table' | 'view' | 'procedure' | 'function';

interface ContextMenuState {
  x: number;
  y: number;
  target: ObjectContextTarget;
}

function SchemaIcon() {
  return (
    <svg width="13" height="13" viewBox="0 0 13 13" fill="none" aria-hidden="true">
      <rect x="1" y="3" width="11" height="8" rx="1" stroke="rgba(244,239,231,0.45)" strokeWidth="1.1" />
      <path d="M1 6h11" stroke="rgba(244,239,231,0.45)" strokeWidth="1" />
    </svg>
  );
}

function TableIcon({ active = false }: { active?: boolean }) {
  const color = active ? 'var(--color-accent)' : 'rgba(244,239,231,0.45)';
  return (
    <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true">
      <rect x="1" y="2" width="10" height="8" rx="1" stroke={color} strokeWidth="1.1" />
      <path d="M1 5h10M4.5 2v8" stroke={color} strokeWidth="1" />
    </svg>
  );
}

function ViewIcon() {
  return (
    <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true">
      <path d="M6 2.5C3.5 2.5 1.5 6 1.5 6S3.5 9.5 6 9.5 10.5 6 10.5 6 8.5 2.5 6 2.5z" stroke="rgba(244,239,231,0.5)" strokeWidth="1.1" />
      <circle cx="6" cy="6" r="1.5" stroke="rgba(244,239,231,0.5)" strokeWidth="1" />
    </svg>
  );
}

function ProcIcon() {
  return (
    <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true">
      <path d="M2 3l3 3-3 3M7 9h3" stroke="rgba(244,239,231,0.5)" strokeWidth="1.1" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

function GroupHeader({ label, count, indent }: { label: string; count: number; indent: number }) {
  return (
    <div style={{ display: 'flex', alignItems: 'center', padding: `10px ${8 + indent * 12}px 4px`, gap: 6 }}>
      <svg width="8" height="8" viewBox="0 0 8 8" fill="none" aria-hidden="true">
        <path d="M2 1l4 3-4 3" stroke="rgba(244,239,231,0.4)" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" />
      </svg>
      <span style={{ fontSize: 10, fontWeight: 600, color: 'rgba(244,239,231,0.35)', textTransform: 'uppercase', letterSpacing: '0.07em' }}>
        {label}
      </span>
      <span style={{ fontSize: 10, color: 'rgba(244,239,231,0.25)', fontFamily: 'Geist Mono, monospace' }}>
        ({count})
      </span>
    </div>
  );
}

interface SchemaSectionProps {
  schema: SchemaNode;
  filter: string;
}

export function SchemaSection({ schema, filter }: SchemaSectionProps) {
  const lc = filter.toLowerCase();
  const tables = filter ? schema.tables.filter((t) => t.name.toLowerCase().includes(lc)) : schema.tables;
  const views = filter ? schema.views.filter((v) => v.name.toLowerCase().includes(lc)) : schema.views;
  const procs = filter ? schema.procedures.filter((p) => p.name.toLowerCase().includes(lc)) : schema.procedures;
  const functions = filter
    ? (schema.functions ?? []).filter((f) => f.name.toLowerCase().includes(lc))
    : (schema.functions ?? []);

  const navigate = useNavigate();
  const activeConnectionId = useActiveConnectionStore((s) => s.activeConnectionId);
  const setActiveConnection = useActiveConnectionStore((s) => s.setActiveConnection);
  const openTab = useOpenTabsStore((s) => s.openTab);

  const [menu, setMenu] = useState<ContextMenuState | null>(null);

  const openSelectTop100 = (schemaName: string, objectName: string, isTable: boolean): void => {
    if (!activeConnectionId) return;
    setActiveConnection(activeConnectionId);
    const sql = `SELECT TOP 100 * FROM [${schemaName}].[${objectName}]`;
    const tabId = openTab({
      id: crypto.randomUUID(),
      connectionId: activeConnectionId,
      title: `${schemaName}.${objectName}`,
      sql,
      isDirty: false,
      createdAt: new Date().toISOString(),
      ...(isTable ? { browseTable: { schema: schemaName, name: objectName } } : {}),
    });
    navigate(`/editor?tab=${tabId}`);
  };

  const openContextMenu = (e: React.MouseEvent, target: ObjectContextTarget): void => {
    e.preventDefault();
    e.stopPropagation();
    setMenu({ x: e.clientX, y: e.clientY, target });
  };

  const handleDesignTable = (schemaName: string, tableName: string): void => {
    if (!activeConnectionId) return;
    navigate(`/designer/${activeConnectionId}/${schemaName}/${tableName}`);
  };

  const handleNewTable = (schemaName: string): void => {
    if (!activeConnectionId) return;
    navigate(`/designer/${activeConnectionId}/${schemaName}/NewTable?new=1`);
  };

  const handleScriptAs = async (kind: ScriptObjectKind, schemaName: string, objectName: string, action: ScriptAction): Promise<void> => {
    if (!activeConnectionId) return;
    // TODO wire to object-scripter feature when ported
    console.warn('script_object: object-scripter not yet ported', { kind, schemaName, objectName, action });
  };

  return (
    <>
      <div
        onContextMenu={(e) => {
          if (e.currentTarget !== e.target && (e.target as HTMLElement).closest('[data-object-leaf]')) return;
          openContextMenu(e, { kind: 'schema', schema: schema.name, name: '' });
        }}
      >
        <TreeNode label={schema.name} icon={<SchemaIcon />} defaultOpen={true} indent={0}>
          {tables.length > 0 && (
            <>
              <li><GroupHeader label="Tables" count={schema.tables.length} indent={2} /></li>
              {tables.map((t) => (
                <div key={`${t.schema}.${t.name}`} data-object-leaf="table" onContextMenu={(e) => openContextMenu(e, { kind: 'table', schema: t.schema, name: t.name })}>
                  <LeafNode label={t.name} icon={<TableIcon />} rowCount={t.rowCount} indent={3} onSelect={() => openSelectTop100(t.schema, t.name, true)} />
                </div>
              ))}
            </>
          )}
          {views.length > 0 && (
            <>
              <li><GroupHeader label="Views" count={schema.views.length} indent={2} /></li>
              {views.map((v) => (
                <div key={`${v.schema}.${v.name}`} data-object-leaf="view" onContextMenu={(e) => openContextMenu(e, { kind: 'view', schema: v.schema, name: v.name })}>
                  <LeafNode label={v.name} icon={<ViewIcon />} indent={3} onSelect={() => openSelectTop100(v.schema, v.name, false)} />
                </div>
              ))}
            </>
          )}
          {procs.length > 0 && (
            <>
              <li><GroupHeader label="Stored Procs" count={schema.procedures.length} indent={2} /></li>
              {procs.map((p) => (
                <div key={`${p.schema}.${p.name}`} data-object-leaf="procedure" onContextMenu={(e) => openContextMenu(e, { kind: 'procedure', schema: p.schema, name: p.name })}>
                  <LeafNode label={p.name} icon={<ProcIcon />} indent={3} />
                </div>
              ))}
            </>
          )}
          {functions.length > 0 && (
            <>
              <li><GroupHeader label="Functions" count={functions.length} indent={2} /></li>
              {functions.map((f) => (
                <div key={`${f.schema}.${f.name}`} data-object-leaf="function" onContextMenu={(e) => openContextMenu(e, { kind: 'function', schema: f.schema, name: f.name })}>
                  <LeafNode label={f.name} icon={<ProcIcon />} indent={3} />
                </div>
              ))}
            </>
          )}
        </TreeNode>
      </div>

      {menu && (
        <ObjectContextMenu
          x={menu.x}
          y={menu.y}
          target={menu.target}
          onClose={() => setMenu(null)}
          onDesignTable={menu.target.kind === 'table' ? () => handleDesignTable(menu.target.schema, menu.target.name) : undefined}
          onNewTable={menu.target.kind === 'schema' ? () => handleNewTable(menu.target.schema) : undefined}
          onImportData={menu.target.kind === 'table' || menu.target.kind === 'schema' ? () => {} : undefined}
          onScriptAs={menu.target.kind === 'schema' ? undefined : (action) => void handleScriptAs(menu.target.kind as ScriptObjectKind, menu.target.schema, menu.target.name, action)}
        />
      )}
    </>
  );
}
