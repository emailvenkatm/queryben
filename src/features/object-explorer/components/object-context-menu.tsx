import { useEffect, useMemo, useRef, useState } from 'react';
import { cn } from '@/shared/lib/cn';
import type { ObjectContextTarget, ScriptAction } from '../types';

export type { ObjectContextTarget } from '../types';

interface ObjectContextMenuProps {
  x: number;
  y: number;
  target: ObjectContextTarget;
  onClose: () => void;
  onDesignTable?: () => void;
  onNewTable?: () => void;
  onImportData?: () => void;
  onScriptAs?: (action: ScriptAction) => void;
}

interface MenuAction {
  label: string;
  onClick: () => void;
  disabled?: boolean;
  submenu?: MenuAction[];
}

export function ObjectContextMenu({ x, y, target, onClose, onDesignTable, onNewTable, onImportData, onScriptAs }: ObjectContextMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);
  const [openSubmenu, setOpenSubmenu] = useState<string | null>(null);

  useEffect(() => {
    const handler = (evt: MouseEvent): void => {
      if (menuRef.current && !menuRef.current.contains(evt.target as Node)) onClose();
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [onClose]);

  const actions = useMemo<MenuAction[]>(() => {
    const list: MenuAction[] = [];

    if (target.kind === 'schema') {
      if (onNewTable) list.push({ label: 'New table…', onClick: () => onNewTable() });
      if (onImportData) list.push({ label: 'Import data…', onClick: () => onImportData() });
      return list;
    }

    const scriptAction = (action: ScriptAction, label: string): MenuAction => ({
      label,
      onClick: () => onScriptAs?.(action),
      disabled: !onScriptAs,
    });

    const scriptSubmenu: MenuAction[] = target.kind === 'table'
      ? [
          scriptAction('create', 'CREATE'),
          scriptAction('alter', 'ALTER'),
          scriptAction('drop', 'DROP'),
          scriptAction('dropAndCreate', 'DROP and CREATE'),
          scriptAction('selectTop', 'SELECT TOP 1000'),
          scriptAction('insertTemplate', 'INSERT template'),
        ]
      : [
          scriptAction('create', 'CREATE'),
          scriptAction('alter', 'ALTER'),
          scriptAction('drop', 'DROP'),
          scriptAction('dropAndCreate', 'DROP and CREATE'),
        ];

    if (target.kind === 'table') {
      if (onDesignTable) list.push({ label: 'Design table…', onClick: () => onDesignTable() });
      if (onImportData) list.push({ label: 'Import data…', onClick: () => onImportData() });
    }
    if (onScriptAs) list.push({ label: 'Script as', onClick: () => {}, submenu: scriptSubmenu });
    return list;
  }, [target, onDesignTable, onNewTable, onImportData, onScriptAs]);

  useEffect(() => {
    if (actions.length === 0) onClose();
  }, [actions.length, onClose]);

  const style: React.CSSProperties = {
    left: Math.min(x, window.innerWidth - 220),
    top: Math.min(y, window.innerHeight - actions.length * 32 - 8),
  };

  return (
    <div
      ref={menuRef}
      role="menu"
      aria-label={`Actions for ${target.kind} ${target.name || target.schema}`}
      className={cn('fixed z-50 min-w-[200px] rounded-md border border-border bg-popover py-1 shadow-md')}
      style={style}
    >
      {actions.map((action) => {
        const hasSubmenu = Boolean(action.submenu);
        const isOpen = openSubmenu === action.label;
        return (
          <div
            key={action.label}
            role="menuitem"
            aria-haspopup={hasSubmenu || undefined}
            aria-expanded={hasSubmenu ? isOpen : undefined}
            onMouseEnter={() => hasSubmenu && setOpenSubmenu(action.label)}
            onMouseLeave={() => hasSubmenu && setOpenSubmenu(null)}
            style={{ position: 'relative' }}
          >
            <button
              type="button"
              disabled={action.disabled}
              className={cn(
                'flex w-full items-center justify-between gap-2 px-3 py-1.5 text-sm text-popover-foreground',
                action.disabled ? 'opacity-40 cursor-not-allowed' : 'hover:bg-accent/10 focus-visible:bg-accent/10 focus-visible:outline-none',
              )}
              onClick={() => {
                if (action.disabled || hasSubmenu) return;
                action.onClick();
                onClose();
              }}
            >
              <span>{action.label}</span>
              {hasSubmenu && <span aria-hidden="true" style={{ fontSize: 10, opacity: 0.6 }}>▸</span>}
            </button>
            {hasSubmenu && isOpen && action.submenu && (
              <div
                role="menu"
                className={cn('absolute z-50 min-w-[200px] rounded-md border border-border bg-popover py-1 shadow-md')}
                style={{ left: '100%', top: 0, marginLeft: 4 }}
              >
                {action.submenu.map((sub) => (
                  <button
                    key={sub.label}
                    type="button"
                    role="menuitem"
                    disabled={sub.disabled}
                    className={cn('flex w-full items-center gap-2 px-3 py-1.5 text-sm text-popover-foreground', sub.disabled ? 'opacity-40 cursor-not-allowed' : 'hover:bg-accent/10 focus-visible:bg-accent/10 focus-visible:outline-none')}
                    onClick={() => { if (sub.disabled) return; sub.onClick(); onClose(); }}
                  >
                    {sub.label}
                  </button>
                ))}
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
