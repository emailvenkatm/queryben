// In-memory store for staged edits in browse-mode tabs. Not persisted — a
// crash or restart drops pending work by design, matching DBeaver's behavior.
// Users have to click Commit to make anything durable.

import { create } from 'zustand';

export type PendingChangeKind = 'update' | 'insert' | 'delete';

export interface PendingChange {
  id: string;
  tabId: string;
  kind: PendingChangeKind;
  rowId: string;
  columnName?: string;
  oldValue?: unknown;
  newValue?: unknown;
  rowValues?: Record<string, unknown>;
  primaryKeyValues?: Record<string, unknown>;
  sql: string;
}

interface PendingChangesState {
  changes: PendingChange[];
  stage: (change: PendingChange) => void;
  unstage: (id: string) => void;
  clearForTab: (tabId: string) => void;
  getByTab: (tabId: string) => PendingChange[];
}

export const usePendingChangesStore = create<PendingChangesState>((set, get) => ({
  changes: [],

  stage: (change) => {
    set((state) => {
      // Coalesce sequential UPDATEs on the same cell: if the user edits then
      // re-edits, replace the earlier record so the tray shows one card.
      if (change.kind === 'update' && change.columnName) {
        const existingIdx = state.changes.findIndex(
          (c) =>
            c.tabId === change.tabId &&
            c.kind === 'update' &&
            c.rowId === change.rowId &&
            c.columnName === change.columnName,
        );
        if (existingIdx >= 0) {
          const existing = state.changes[existingIdx];
          // Preserve the original oldValue so the tray shows the pre-edit state.
          const merged: PendingChange = { ...change, oldValue: existing?.oldValue };
          const next = [...state.changes];
          next[existingIdx] = merged;
          return { changes: next };
        }
      }
      return { changes: [...state.changes, change] };
    });
  },

  unstage: (id) => set((state) => ({ changes: state.changes.filter((c) => c.id !== id) })),

  clearForTab: (tabId) =>
    set((state) => ({ changes: state.changes.filter((c) => c.tabId !== tabId) })),

  getByTab: (tabId) => get().changes.filter((c) => c.tabId === tabId),
}));
