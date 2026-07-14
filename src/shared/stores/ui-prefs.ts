import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import type { UiPrefs } from '@/shared/types';

interface UiPrefsState {
  prefs: UiPrefs;
  updatePrefs: (patch: Partial<UiPrefs>) => void;
}

const DEFAULT_PREFS: UiPrefs = {
  editorFontSize: 14,
  editorWordWrap: false,
  resultsMaxRows: 5000,
  connectionTimeoutSec: 30,
  autoUpdateEnabled: true,
  theme: 'light',
};

export const useUiPrefsStore = create<UiPrefsState>()(
  persist(
    (set) => ({
      prefs: DEFAULT_PREFS,
      updatePrefs: (patch) =>
        set((state) => ({ prefs: { ...state.prefs, ...patch } })),
    }),
    { name: 'qb-ui-prefs' },
  ),
);
