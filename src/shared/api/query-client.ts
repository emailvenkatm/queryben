import { QueryClient } from '@tanstack/react-query';

// Tauri commands = "the server". Retry once (errors are usually deterministic),
// no refetch on window focus (webview focus tells us nothing about staleness).
export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 0,
      gcTime: 5 * 60 * 1000,
      retry: 1,
      refetchOnWindowFocus: false,
    },
    mutations: {
      retry: 0,
    },
  },
});
