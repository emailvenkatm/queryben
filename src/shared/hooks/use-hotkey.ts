import { useEffect, useRef } from 'react';

type ModifierKey = 'meta' | 'ctrl' | 'shift' | 'alt';

interface HotkeyOptions {
  // 'meta' = Cmd on macOS, Win key on Windows.
  modifiers?: ModifierKey[];
  preventDefault?: boolean;
  target?: HTMLElement | null;
  enabled?: boolean;
}

export function useHotkey(
  key: string,
  options: HotkeyOptions,
  handler: (event: KeyboardEvent) => void,
): void {
  const { modifiers = [], preventDefault = true, target = null, enabled = true } = options;

  const ref = useRef(handler);
  ref.current = handler;

  useEffect(() => {
    if (!enabled) return;

    const el: EventTarget = target ?? document;

    const onKeyDown = (evt: Event): void => {
      if (!(evt instanceof KeyboardEvent)) return;

      const modMatch = modifiers.every((mod) => {
        if (mod === 'meta') return evt.metaKey;
        if (mod === 'ctrl') return evt.ctrlKey;
        if (mod === 'shift') return evt.shiftKey;
        if (mod === 'alt') return evt.altKey;
        return false;
      });

      if (!modMatch || evt.key !== key) return;
      if (preventDefault) evt.preventDefault();
      ref.current(evt);
    };

    el.addEventListener('keydown', onKeyDown);
    return () => el.removeEventListener('keydown', onKeyDown);
  }, [key, modifiers, preventDefault, target, enabled]);
}
