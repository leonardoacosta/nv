import { QueryClient } from "@tanstack/react-query";

/**
 * Creates a QueryClient instance with default options tuned for the Nova dashboard.
 *
 * - 30s staleTime: dashboard receives real-time updates via WebSocket,
 *   so 30s prevents redundant refetches on tab switches while keeping data fresh.
 * - 5min gcTime: keeps inactive query data available for quick back-navigation.
 * - 3 retries with exponential backoff: resilient to transient network issues.
 * - refetchOnWindowFocus: ensures stale data is refreshed when user returns to tab.
 */
export function createQueryClient(): QueryClient {
  return new QueryClient({
    defaultOptions: {
      queries: {
        staleTime: 30_000,
        gcTime: 5 * 60_000,
        retry: 3,
        retryDelay: (attempt) => Math.min(1000 * 2 ** attempt, 10_000),
        refetchOnWindowFocus: true,
      },
      mutations: {
        retry: 0,
      },
    },
  });
}
