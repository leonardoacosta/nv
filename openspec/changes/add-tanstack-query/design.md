# Design: Add TanStack Query

## Architecture Overview

### Provider Stack

```
RootLayout (RSC)
  └─ AppShell ("use client")
       └─ QueryClientProvider  ← NEW
            └─ DaemonEventProvider (existing)
                 └─ Sidebar + main content
            └─ ReactQueryDevtools (dev only)
```

`QueryClientProvider` wraps inside `AppShell` so it shares the client boundary. The login page bypasses the provider (already excluded by AppShell's `isLoginPage` guard).

### QueryClient Configuration

```typescript
// lib/query-client.ts
const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,        // 30s — dashboard data refreshes via WS + polling
      gcTime: 5 * 60_000,       // 5min garbage collection
      retry: 3,
      retryDelay: (attempt) => Math.min(1000 * 2 ** attempt, 10_000),
      refetchOnWindowFocus: true,
    },
    mutations: {
      retry: 0,                 // Mutations should not auto-retry
    },
  },
});
```

### Hook Architecture

```
apiFetch() (existing)           ← Bearer token + 401 redirect
  ↑
useApiQuery(path, options?)     ← Wraps useQuery + apiFetch
useApiMutation(path, options?)  ← Wraps useMutation + apiFetch
  ↑
Page components                 ← Consume hooks
  ↑
trpc.*.queryOptions() (future)  ← Drop-in replacement for useApiQuery
```

**`useApiQuery` signature:**
```typescript
function useApiQuery<T>(
  path: string,
  options?: {
    params?: Record<string, string>;
    enabled?: boolean;
    refetchInterval?: number | false;
    select?: (data: T) => unknown;
    staleTime?: number;
  },
): UseQueryResult<T>;
```

**Query key convention:**
```typescript
// Current: ["api", "/api/obligations"]
// With params: ["api", "/api/sessions", { page: "1", limit: "25" }]
// Future tRPC: [["obligation", "getAll"], { input: { limit: 25 } }]
```

The `["api", path, params?]` tuple is intentionally distinct from tRPC's key shape so both can coexist during migration without collisions.

### State Pattern Components

Three reusable components following the Loading -> Error -> Empty -> Data pattern:

| Component | Props | Visual |
|-----------|-------|--------|
| `QuerySkeleton` | `rows?: number`, `height?: string` | Pulse-animated rows matching ds-gray-100 |
| `QueryErrorState` | `message: string`, `onRetry?: () => void` | AlertCircle icon + retry button |
| `QueryEmptyState` | `title?: string`, `description?: string`, `onCreate?: () => void` | Inbox icon + CTA |

All components use existing ds-token classes (`bg-ds-gray-100`, `text-ds-gray-900`, etc.) -- no new design tokens needed.

### Migration Strategy

**Phase 1 (DB/API batch):** No database or API changes. This is purely a client-side refactor.

**Phase 2 (UI batch):** Migrate pages in order of complexity:
1. **Home page** (`app/page.tsx`) -- Most complex. 6 parallel fetches become 6 `useApiQuery` calls. `Promise.allSettled` replaced by independent query subscriptions. Auto-refresh `setInterval` replaced by `refetchInterval: 10_000`.
2. **Sessions** (`app/sessions/page.tsx`) -- Pagination + filters. Query key includes filter params for automatic cache separation.
3. **Obligations** (`app/obligations/page.tsx`) -- CRUD with mutations. First use of `useApiMutation` + invalidation.
4. **Messages** (`app/messages/page.tsx`) -- Pagination + grouping.
5. **Remaining 14 pages** -- Follow established patterns.

**Phase 3 (Server prefetch):** Convert `app/layout.tsx` to prefetch home page data. Add `HydrationBoundary` wrapper.

### Server Prefetch Architecture

```
app/layout.tsx (RSC, existing)
  ├─ prefetchQuery(["api", "/api/activity-feed"])     ← fire-and-forget
  ├─ prefetchQuery(["api", "/api/obligations"])
  ├─ prefetchQuery(["api", "/api/messages?limit=50"])
  └─ <HydrationBoundary state={dehydrate(queryClient)}>
       <AppShell>{children}</AppShell>
     </HydrationBoundary>
```

Server prefetch requires a server-side `apiFetch` variant that can inject auth tokens from cookies (via `next/headers`). This is scoped to the home and briefing pages only.

### Query Invalidation Map

| Mutation | Invalidates |
|----------|-------------|
| Create obligation | `["api", "/api/obligations"]`, `["api", "/api/activity-feed"]` |
| Update obligation | `["api", "/api/obligations"]` |
| Send message | `["api", "/api/messages"]` |
| Update automation | `["api", "/api/automations"]` |
| Update settings | `["api", "/api/settings"]` |

WebSocket events from `DaemonEventContext` trigger `queryClient.invalidateQueries` for the relevant key, replacing the current pattern of manually re-running `fetchData()` callbacks.

### tRPC Migration Path

When tRPC is added, the migration per page is:

```diff
- const { data, isLoading, error } = useApiQuery<ObligationsGetResponse>("/api/obligations");
+ const { data, isLoading, error } = useQuery(trpc.obligation.getAll.queryOptions());
```

The `useApiQuery` wrapper is intentionally thin so deletion is trivial. Query invalidation switches from:

```diff
- queryClient.invalidateQueries({ queryKey: ["api", "/api/obligations"] });
+ queryClient.invalidateQueries({ queryKey: trpc.obligation.getAll.queryKey() });
```

## Trade-offs

| Decision | Alternative | Rationale |
|----------|-------------|-----------|
| 30s staleTime | 0 (always refetch) | Dashboard has WS for real-time; 30s prevents redundant fetches on tab switches while keeping data fresh. |
| Separate `useApiQuery` wrapper | Direct `useQuery` everywhere | Centralizes `apiFetch` integration, makes tRPC swap a single-point change. |
| `["api", path]` key convention | Flat string keys | Structured keys enable partial invalidation (`["api"]` invalidates everything) and distinguish from future tRPC keys. |
| Server prefetch only for home + briefing | All pages | Most pages are deep-linked; users rarely hit them without prior navigation. Home + briefing are the entry points worth optimizing. |
