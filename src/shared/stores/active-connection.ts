import { create } from 'zustand';
import { persist } from 'zustand/middleware';

interface ActiveConnectionState {
  activeConnectionId: string | null;
  setActiveConnection: (id: string | null) => void;
}

// Default target for new query tabs. Persisted so it survives app restarts.
export const useActiveConnectionStore = create<ActiveConnectionState>()(
  persist(
    (set) => ({
      activeConnectionId: null,
      setActiveConnection: (id) => set({ activeConnectionId: id }),
    }),
    { name: 'qb-active-connection' },
  ),
);
