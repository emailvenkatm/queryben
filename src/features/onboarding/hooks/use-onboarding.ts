import { useCallback, useMemo, useState } from 'react';

const STORAGE_KEY = 'queryben.onboarding.v1';

interface State {
  completedAt: string | null;
  skippedAt: string | null;
  importedFromAds: boolean;
}

function read(): State | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as State;
    if (typeof parsed === 'object' && parsed !== null && ('completedAt' in parsed || 'skippedAt' in parsed)) {
      return parsed;
    }
    return null;
  } catch {
    return null;
  }
}

function write(next: State): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
  } catch {
    // Private mode or quota — wizard reopens next launch; acceptable.
  }
}

export function useOnboarding() {
  const [state, setState] = useState<State | null>(read);

  const isFirstRun = useMemo(() => {
    if (!state) return true;
    return state.completedAt === null && state.skippedAt === null;
  }, [state]);

  const markComplete = useCallback((opts?: { importedFromAds?: boolean }) => {
    const next: State = {
      completedAt: new Date().toISOString(),
      skippedAt: null,
      importedFromAds: opts?.importedFromAds ?? false,
    };
    write(next);
    setState(next);
  }, []);

  const skipAll = useCallback(() => {
    const next: State = { completedAt: null, skippedAt: new Date().toISOString(), importedFromAds: false };
    write(next);
    setState(next);
  }, []);

  return { isFirstRun, markComplete, skipAll };
}

// Exposed for the shell agent to gate routing without importing the full hook.
export function hasSeenOnboarding(): boolean {
  const s = read();
  if (!s) return false;
  return s.completedAt !== null || s.skippedAt !== null;
}
