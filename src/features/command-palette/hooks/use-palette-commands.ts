import { useNavigate } from 'react-router-dom';
import { useConnections } from '@/features/connections/index';
import { useOpenTabsStore } from '@/shared/stores/open-tabs';
import { useActiveConnectionStore } from '@/shared/stores/active-connection';
import { connectionDisplayName, type Connection } from '@/shared/types';

export interface PaletteCommand {
  id: string;
  label: string;
  sub?: string;
  kbd?: string;
  section: string;
  onSelect: () => void;
}

interface UsePaletteCommandsArgs {
  query: string;
  onClose: () => void;
  onExport: () => void;
  onCopy: (format: 'markdown' | 'json') => void;
  hasResults: boolean;
}

export function usePaletteCommands({ query, onClose, onExport, onCopy, hasResults }: UsePaletteCommandsArgs) {
  const navigate = useNavigate();
  const { data: connections } = useConnections();
  const tabs = useOpenTabsStore((s) => s.tabs);
  const setActiveConnection = useActiveConnectionStore((s) => s.setActiveConnection);
  const openTab = useOpenTabsStore((s) => s.openTab);

  const q = query.toLowerCase();

  const filteredTabs = tabs.filter((t) => !q || t.title.toLowerCase().includes(q));

  const filteredConnections = (connections ?? []).filter((c) => {
    if (!q) return true;
    return (
      c.server.toLowerCase().includes(q) ||
      c.database.toLowerCase().includes(q) ||
      (c.nickname ?? '').toLowerCase().includes(q)
    );
  });

  const connectTo = (conn: Connection) => {
    setActiveConnection(conn.id);
    const tabId = openTab({
      id: crypto.randomUUID(),
      connectionId: conn.id,
      title: `${conn.database} · ${conn.server}`,
      sql: '',
      isDirty: false,
      createdAt: new Date().toISOString(),
    });
    navigate(`/editor?tab=${tabId}`);
    onClose();
  };

  const matchesCmd = (term: string): boolean => !q || term.toLowerCase().includes(q);

  const systemCommands: PaletteCommand[] = [
    matchesCmd('run execute query') && {
      id: 'run-query',
      label: 'Execute query',
      sub: 'Run the active query in the editor',
      kbd: 'F5',
      section: 'Commands',
      onSelect: onClose,
    },
    matchesCmd('export results') && {
      id: 'export',
      label: 'Export results…',
      sub: 'Save as CSV, XLSX, or JSON',
      kbd: '⇧⌘E',
      section: 'Commands',
      onSelect: () => { if (!hasResults) return; onClose(); onExport(); },
    },
    matchesCmd('copy markdown table') && {
      id: 'copy-md',
      label: 'Copy last result as Markdown',
      sub: 'Copy results as a Markdown table',
      section: 'Commands',
      onSelect: () => { if (!hasResults) return; onCopy('markdown'); onClose(); },
    },
    matchesCmd('copy json') && {
      id: 'copy-json',
      label: 'Copy last result as JSON',
      sub: 'Copy results as JSON',
      section: 'Commands',
      onSelect: () => { if (!hasResults) return; onCopy('json'); onClose(); },
    },
    matchesCmd('settings preferences') && {
      id: 'settings',
      label: 'Open Settings',
      sub: 'Change theme, editor, and connection preferences',
      section: 'Commands',
      onSelect: () => { navigate('/settings'); onClose(); },
    },
  ].filter(Boolean) as PaletteCommand[];

  const tabCommands: PaletteCommand[] = filteredTabs.map((tab) => ({
    id: `tab-${tab.id}`,
    label: tab.title,
    sub: 'Open tab',
    section: 'Open Tabs',
    onSelect: () => { navigate(`/editor?tab=${tab.id}`); onClose(); },
  }));

  const connectionCommands: PaletteCommand[] = filteredConnections.map((conn) => ({
    id: `conn-${conn.id}`,
    label: connectionDisplayName(conn),
    sub: conn.nickname ? `${conn.server} · ${conn.database}` : `${conn.database} · ${conn.authMode}`,
    section: 'Switch connection',
    onSelect: () => connectTo(conn),
    _conn: conn,
  })) as (PaletteCommand & { _conn: Connection })[];

  return { tabCommands, systemCommands, connectionCommands, filteredConnections };
}
