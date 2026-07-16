import { useState } from 'react';
import { cn } from '@/shared/lib/cn';

interface TreeNodeProps {
  label: string;
  icon: React.ReactNode;
  children?: React.ReactNode;
  defaultOpen?: boolean;
  indent?: number;
}

export function TreeNode({ label, icon, children, defaultOpen = false, indent = 0 }: TreeNodeProps) {
  const [open, setOpen] = useState(defaultOpen);
  const hasChildren = Boolean(children);

  return (
    <li>
      <button
        type="button"
        style={{ display: 'flex', alignItems: 'center', gap: 0, height: 28, width: '100%', cursor: 'pointer', paddingLeft: 8 + indent * 12, paddingRight: 8, background: 'transparent', border: 'none', textAlign: 'left', transition: 'background 80ms', fontFamily: 'Geist Mono, monospace', position: 'relative' }}
        onClick={() => hasChildren && setOpen((o) => !o)}
        aria-expanded={hasChildren ? open : undefined}
        onMouseEnter={(e) => { (e.currentTarget as HTMLElement).style.background = 'rgba(244,239,231,0.05)'; }}
        onMouseLeave={(e) => { (e.currentTarget as HTMLElement).style.background = 'transparent'; }}
      >
        <span style={{ width: 20, height: 28, display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0 }}>
          {hasChildren && (
            <svg width="8" height="8" viewBox="0 0 8 8" fill="none" aria-hidden="true">
              {open
                ? <path d="M1 3l3 3 3-3" stroke="rgba(244,239,231,0.5)" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" />
                : <path d="M2 1l4 3-4 3" stroke="rgba(244,239,231,0.5)" strokeWidth="1.2" strokeLinecap="round" />}
            </svg>
          )}
        </span>
        <span style={{ width: 20, height: 28, display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0 }}>
          {icon}
        </span>
        <span style={{ fontSize: 12, color: 'rgba(244,239,231,0.7)', flex: 1, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
          {label}
        </span>
      </button>

      {open && children && (
        <ul style={{ listStyle: 'none', margin: 0, padding: 0 }} role="group">
          {children}
        </ul>
      )}
    </li>
  );
}

interface LeafNodeProps {
  label: string;
  icon: React.ReactNode;
  rowCount?: number;
  isActive?: boolean;
  indent?: number;
  onSelect?: () => void;
  showScriptActions?: boolean;
}

export function LeafNode({ label, icon, rowCount, isActive = false, indent = 0, onSelect, showScriptActions = false }: LeafNodeProps) {
  return (
    <li>
      <div
        className="group"
        onClick={onSelect}
        style={{ display: 'flex', alignItems: 'center', height: 28, cursor: 'pointer', paddingLeft: 8 + indent * 12, paddingRight: 8, background: isActive ? 'rgba(213,138,74,0.15)' : 'transparent', position: 'relative', transition: 'background 80ms', fontFamily: 'Geist Mono, monospace' }}
        onMouseEnter={(e) => { if (!isActive) (e.currentTarget as HTMLElement).style.background = 'rgba(244,239,231,0.05)'; }}
        onMouseLeave={(e) => { (e.currentTarget as HTMLElement).style.background = isActive ? 'rgba(213,138,74,0.15)' : 'transparent'; }}
        tabIndex={0}
        role="treeitem"
        aria-selected={isActive}
        onKeyDown={(e) => { if (e.key === 'Enter') onSelect?.(); }}
      >
        {isActive && (
          <span style={{ position: 'absolute', left: 0, top: 3, bottom: 3, width: 2, background: 'var(--color-accent)', borderRadius: 1 }} aria-hidden="true" />
        )}
        <span style={{ width: 20, flexShrink: 0 }} />
        <span style={{ width: 20, height: 28, display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0 }}>
          {icon}
        </span>
        <span style={{ fontSize: 12, color: isActive ? 'var(--color-bg)' : 'rgba(244,239,231,0.7)', flex: 1, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
          {label}
        </span>
        {rowCount !== undefined && (
          <span
            className="group-hover:opacity-100"
            style={{ fontSize: 10, color: 'rgba(244,239,231,0.35)', fontFamily: 'Geist Mono, monospace', padding: '0 6px', flexShrink: 0 }}
          >
            {rowCount >= 1_000_000
              ? `${(rowCount / 1_000_000).toFixed(1)}M`
              : rowCount >= 1_000
              ? `${(rowCount / 1_000).toFixed(0)}K`
              : String(rowCount)}
          </span>
        )}
        {showScriptActions && (
          <div className={cn('opacity-0 group-hover:opacity-100')} style={{ display: 'flex', gap: 2, flexShrink: 0, transition: 'opacity 100ms' }}>
            {['SELECT', 'INSERT', 'CREATE'].map((action) => (
              <button
                key={action}
                type="button"
                onClick={(e) => { e.stopPropagation(); }}
                aria-label={`Script as ${action}`}
                style={{ padding: '3px 6px', border: 'none', background: 'rgba(244,239,231,0.10)', borderRadius: 5, color: 'rgba(244,239,231,0.65)', fontSize: 10, fontWeight: 600, cursor: 'pointer', fontFamily: 'Geist Mono, monospace' }}
              >
                {action}
              </button>
            ))}
          </div>
        )}
      </div>
    </li>
  );
}
