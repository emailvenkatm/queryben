import { Link, useLocation } from 'react-router-dom';
import { cn } from '@/shared/lib/cn';
import {
  IconConnections,
  IconDatabase,
  IconHistory,
  IconNotebook,
  IconQueries,
  IconSettings,
} from './icons';

interface SidebarProps {
  onOpenCommandPalette: () => void;
  accountAvatar?: React.ReactNode;
}

const NAV_ITEMS = [
  { path: '/', label: 'Connections', Icon: IconConnections },
  { path: '/queries', label: 'Saved queries', Icon: IconQueries },
  { path: '/history', label: 'Query history', Icon: IconHistory },
  { path: '/notebook', label: 'Notebooks', Icon: IconNotebook },
] as const;

function NavLink({
  path,
  label,
  Icon,
  active,
}: {
  path: string;
  label: string;
  Icon: () => React.ReactElement;
  active: boolean;
}) {
  return (
    <Link
      to={path}
      className={cn(
        'flex items-center gap-[10px] rounded-md cursor-pointer transition-colors',
        'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber',
      )}
      style={{
        padding: '7px 16px',
        fontSize: 13,
        margin: '1px 8px',
        color: active ? 'var(--color-text-inverse)' : 'rgba(244,239,231,0.65)',
        background: active ? 'rgba(244,239,231,0.10)' : 'transparent',
      }}
      aria-current={active ? 'page' : undefined}
    >
      <span style={{ width: 16, height: 16, opacity: 0.7, flexShrink: 0 }}>
        <Icon />
      </span>
      {label}
    </Link>
  );
}

export function Sidebar({ onOpenCommandPalette, accountAvatar }: SidebarProps) {
  const location = useLocation();

  const isActive = (path: string): boolean => {
    if (path === '/') return location.pathname === '/';
    return location.pathname.startsWith(path);
  };

  return (
    <aside
      className="flex flex-col shrink-0 overflow-hidden"
      style={{ width: 220, background: 'var(--color-bg-sidebar)', minHeight: 'calc(100vh - 40px)' }}
      aria-label="Main navigation"
    >
      <div style={{ padding: '20px 16px 12px' }}>
        <Link
          to="/"
          className="flex items-center gap-2 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber rounded"
          aria-label="QueryBen: go home"
        >
          <div
            style={{
              width: 28,
              height: 28,
              background: 'var(--color-accent)',
              borderRadius: 7,
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              flexShrink: 0,
              color: 'var(--color-text-inverse)',
            }}
          >
            <IconDatabase />
          </div>
          <span style={{ fontSize: 14, fontWeight: 600, color: 'var(--color-text-inverse)', letterSpacing: '-0.01em' }}>
            QueryBen
          </span>
        </Link>
      </div>

      <nav aria-label="Primary" style={{ padding: '6px 0' }}>
        {NAV_ITEMS.map(({ path, label, Icon }) => (
          <NavLink key={path} path={path} label={label} Icon={Icon} active={isActive(path)} />
        ))}
      </nav>

      <div style={{ borderTop: '1px solid rgba(244,239,231,0.08)', margin: '8px 0' }} />

      <nav aria-label="Secondary" style={{ padding: '6px 0' }}>
        <NavLink path="/settings" label="Settings" Icon={IconSettings} active={isActive('/settings')} />
      </nav>

      <div style={{ flex: 1 }} />

      {accountAvatar}

      <div style={{ padding: 16, borderTop: '1px solid rgba(244,239,231,0.08)' }}>
        <button
          type="button"
          onClick={onOpenCommandPalette}
          className="flex items-center gap-[6px] w-full text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber rounded"
          style={{ fontSize: 11, color: 'rgba(244,239,231,0.35)', background: 'none', border: 'none', cursor: 'pointer', padding: 0 }}
          aria-label="Open command palette (Cmd+K)"
        >
          <span style={{ background: 'rgba(244,239,231,0.1)', borderRadius: 3, padding: '1px 5px', fontFamily: 'Geist Mono, monospace', fontSize: 10 }}>
            K
          </span>
          Command palette
        </button>
      </div>
    </aside>
  );
}
