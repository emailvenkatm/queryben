import { useState } from 'react';
import { DEFAULT_PREFS, type UiPrefs } from '../types';

const KEY = 'queryben.ui-prefs.v1';

function load(): UiPrefs {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return DEFAULT_PREFS;
    return { ...DEFAULT_PREFS, ...(JSON.parse(raw) as Partial<UiPrefs>) };
  } catch {
    return DEFAULT_PREFS;
  }
}

function save(prefs: UiPrefs): void {
  localStorage.setItem(KEY, JSON.stringify(prefs));
}

export function useUiPrefs(): { prefs: UiPrefs; update: (delta: Partial<UiPrefs>) => void } {
  const [prefs, setPrefs] = useState<UiPrefs>(load);

  const update = (delta: Partial<UiPrefs>): void => {
    setPrefs((prev) => {
      const next = { ...prev, ...delta };
      save(next);
      return next;
    });
  };

  return { prefs, update };
}
