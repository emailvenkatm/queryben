import { useState } from 'react';
import { Outlet, useLocation } from 'react-router-dom';
import { useHotkey } from '@/shared/hooks/use-hotkey';
import { useActiveConnectionStore } from '@/shared/stores/active-connection';
import { Sidebar } from './sidebar';

interface AppShellProps {
  // Feature agents inject their composed widgets here rather than this file
  // importing features directly — keeps the shell decoupled.
  commandPalette?: (props: { open: boolean; onOpenChange: (v: boolean) => void }) => React.ReactNode;
  objectTree?: React.ReactNode;
  accountAvatar?: React.ReactNode;
}

export function AppShell({ commandPalette, objectTree, accountAvatar }: AppShellProps) {
  const [paletteOpen, setPaletteOpen] = useState(false);
  const activeConnectionId = useActiveConnectionStore((s) => s.activeConnectionId);
  const location = useLocation();
  const isEditorRoute = location.pathname.includes('/editor');

  useHotkey('k', { modifiers: ['meta'] }, () => setPaletteOpen((o) => !o));

  return (
    <div className="flex h-screen flex-col overflow-hidden" style={{ background: 'var(--color-bg)' }}>
      <div className="flex flex-1 overflow-hidden">
        <Sidebar
          onOpenCommandPalette={() => setPaletteOpen(true)}
          accountAvatar={accountAvatar}
        />

        {activeConnectionId && isEditorRoute && objectTree && (
          <div
            style={{ width: 228, background: 'var(--color-bg-sidebar)', flexShrink: 0, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}
            aria-label="Object Explorer"
          >
            {objectTree}
          </div>
        )}

        <main className="flex-1 overflow-hidden flex flex-col">
          <Outlet />
        </main>
      </div>

      {commandPalette?.({ open: paletteOpen, onOpenChange: setPaletteOpen })}
    </div>
  );
}
