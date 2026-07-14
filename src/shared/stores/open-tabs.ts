import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import type { QueryTab } from '@/shared/types';

interface OpenTabsState {
  tabs: QueryTab[];
  activeTabId: string | null;

  // Returns the id of the tab that ended up focused. Usually `tab.id`, but
  // when we dedupe onto an existing scratchpad the returned id will be that
  // pre-existing tab's id. Callers routing by URL should use the return value.
  openTab: (tab: QueryTab) => string;
  closeTab: (id: string) => void;
  setActiveTab: (id: string) => void;
  updateTabSql: (id: string, sql: string) => void;
  markTabClean: (id: string) => void;
}

const MAX_TABS = 20;

export const useOpenTabsStore = create<OpenTabsState>()(
  persist(
    (set, get) => ({
      tabs: [],
      activeTabId: null,

      openTab: (tab) => {
        const { tabs } = get();
        if (tabs.some((t) => t.id === tab.id)) {
          set({ activeTabId: tab.id });
          return tab.id;
        }

        // ADS behavior: clicking a table focuses an existing browse tab for
        // that {connectionId, schema, name} rather than stacking another.
        if (tab.browseTable) {
          const existing = tabs.find(
            (t) =>
              t.connectionId === tab.connectionId &&
              t.browseTable?.schema === tab.browseTable!.schema &&
              t.browseTable?.name === tab.browseTable!.name,
          );
          if (existing) {
            set({ activeTabId: existing.id });
            return existing.id;
          }
        }

        // DBeaver behavior: reopening a connection reuses an existing query tab
        // rather than stacking duplicates. Skip when the tab carries SQL.
        const isFreshScratchpad = tab.sql === '' && !tab.isDirty;
        if (isFreshScratchpad) {
          const candidate = [...tabs]
            .reverse()
            .find((t) => t.connectionId === tab.connectionId && !t.browseTable);
          if (candidate) {
            set({ activeTabId: candidate.id });
            return candidate.id;
          }
        }

        const next = tabs.length >= MAX_TABS ? [...tabs.slice(1), tab] : [...tabs, tab];
        set({ tabs: next, activeTabId: tab.id });
        return tab.id;
      },

      closeTab: (id) => {
        const { tabs, activeTabId } = get();
        const idx = tabs.findIndex((t) => t.id === id);
        if (idx === -1) return;
        const next = tabs.filter((t) => t.id !== id);
        const nextActive =
          activeTabId === id ? (next[idx - 1]?.id ?? next[0]?.id ?? null) : activeTabId;
        set({ tabs: next, activeTabId: nextActive });
      },

      setActiveTab: (id) => set({ activeTabId: id }),

      updateTabSql: (id, sql) =>
        set((s) => ({ tabs: s.tabs.map((t) => (t.id === id ? { ...t, sql, isDirty: true } : t)) })),

      markTabClean: (id) =>
        set((s) => ({ tabs: s.tabs.map((t) => (t.id === id ? { ...t, isDirty: false } : t)) })),
    }),
    {
      name: 'qb-open-tabs',
      version: 3,
      migrate: (persisted, version) => {
        let state = persisted as OpenTabsState | undefined;
        if (!state || !Array.isArray(state.tabs)) return state as OpenTabsState;

        // v1 -> v2: backfill browseTable for tabs that predated the field.
        if (version < 2) {
          const re = /^SELECT TOP \d+ \* FROM \[([^\]]+)\]\.\[([^\]]+)\]\s*;?\s*$/i;
          state = {
            ...state,
            tabs: state.tabs.map((t) => {
              if (t.browseTable) return t;
              const m = t.sql?.match(re);
              if (!m) return t;
              return { ...t, browseTable: { schema: m[1]!, name: m[2]! } };
            }),
          };
        }

        // v2 -> v3: dedupe stacked query tabs from before the dedup fix.
        if (version < 3) {
          const seen = new Set<string>();
          const deduped: QueryTab[] = [];
          for (const t of state.tabs) {
            const key = t.browseTable
              ? `browse:${t.connectionId}:${t.browseTable.schema}:${t.browseTable.name}`
              : `query:${t.connectionId}`;
            if (seen.has(key) && !t.isDirty && t.sql === '') continue;
            seen.add(key);
            deduped.push(t);
          }
          state = { ...state, tabs: deduped };
        }

        return state;
      },
    },
  ),
);
