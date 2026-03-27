# Proposal: Add TanStack Query for Client-Side Data Fetching

## Change ID
`add-tanstack-query`

## Summary
Add `@tanstack/react-query` as the client-side data fetching and server state management layer, replacing raw `fetch` + `useEffect` + `useState` patterns across all 18 dashboard pages with cached, deduplicated queries and structured loading/error/empty state handling.

## Context
- Extends: `apps/dashboard/package.json`, `apps/dashboard/components/AppShell.tsx`, `apps/dashboard/lib/api-client.ts`, all 18 page files under `apps/dashboard/app/`
- Related: `@tanstack/react-virtual` is already a dependency (list virtualization). A parallel tRPC proposal will produce `queryOptions()` / `mutationOptions()` -- TanStack Query is the consumer for that future integration.

## Motivation
Every dashboard page independently implements data fetching with raw `fetch` + `useEffect` + `useState`, resulting in duplicated loading/error state logic, no request deduplication, no background refetching, no cache sharing between pages, and inconsistent error handling. The home page alone manages 15+ `useState` calls and a `Promise.allSettled` across 6 endpoints. TanStack Query eliminates this boilerplate, provides automatic caching and background refetch, and establishes the foundation for the incoming tRPC integration which produces `queryOptions()` wrappers that TanStack Query consumes directly.

## Requirements

### Req-1: Query Client Provider
Install `@tanstack/react-query` and `@tanstack/react-query-devtools`. Create a `QueryClientProvider` wrapper in the app shell with sensible defaults: 30-second stale time (dashboard data refreshes frequently via WebSocket supplements), 3 retries with exponential backoff, and `refetchOnWindowFocus` enabled. Include React Query Devtools in development only.

### Req-2: Standardized State Pattern
Establish reusable Loading, Error, and Empty state components following the canonical pattern: Loading (skeleton) -> Error (with retry) -> Empty (with CTA) -> Data. These components must match the existing Geist/ds-token design system used throughout the dashboard.

### Req-3: Custom Query Hook Factory
Create a `useApiQuery` wrapper around `useQuery` that integrates with the existing `apiFetch()` bearer-token injection and 401 redirect logic. This wrapper serves as the migration bridge -- pages switch from raw fetch to `useApiQuery`, and later when tRPC lands, they switch from `useApiQuery` to `trpc.*.queryOptions()`. Similarly, create `useApiMutation` for write operations.

### Req-4: Page Migration
Migrate all 18 dashboard pages from raw `fetch` + `useEffect` + `useState` to `useApiQuery` / `useApiMutation`. Priority order: home page (most complex, 6 parallel fetches), sessions, obligations, messages (high traffic), then remaining pages. Each migration must preserve existing functionality including auto-refresh intervals, WebSocket event integration, and URL search param state.

### Req-5: Server-Side Prefetching
For pages that benefit from instant first paint, implement server-side prefetching using `dehydrate` / `HydrationBoundary`. The root layout becomes a server component that prefetches critical data, and client components read from the hydrated cache -- eliminating the loading spinner on initial navigation. Target: home page and briefing page as initial candidates.

### Req-6: Query Invalidation Strategy
Define a consistent invalidation strategy: mutations invalidate related query keys on success, WebSocket events trigger targeted refetches for real-time data, and the existing auto-refresh intervals are replaced by TanStack Query's `refetchInterval` option where appropriate. Document the query key convention for forward-compatibility with tRPC's `queryKey()` pattern.

### Req-7: tRPC Forward-Compatibility
Structure query keys and hook patterns so the transition to tRPC `queryOptions()` / `mutationOptions()` is a drop-in replacement per endpoint. The `useApiQuery` wrapper should accept the same shape that `queryOptions()` will eventually produce, minimizing the migration surface when tRPC is added.

## Scope
- **IN**: Package installation, QueryClientProvider setup, devtools, custom hooks (`useApiQuery`, `useApiMutation`), reusable state components (Skeleton, ErrorState, EmptyState), migration of all 18 pages, server prefetch for home + briefing, query invalidation patterns, query key conventions
- **OUT**: tRPC installation or router creation (separate spec), React Server Components conversion of existing client pages beyond prefetch wrappers, WebSocket-to-query bridge (existing `DaemonEventContext` stays as-is, queries just refetch on WS events), new API endpoints, database changes

## Impact
| Area | Change |
|------|--------|
| `apps/dashboard/package.json` | Add `@tanstack/react-query`, `@tanstack/react-query-devtools` |
| `apps/dashboard/components/AppShell.tsx` | Wrap children in `QueryClientProvider` |
| `apps/dashboard/lib/query-client.ts` | New: QueryClient factory with defaults |
| `apps/dashboard/lib/hooks/use-api-query.ts` | New: `useApiQuery` / `useApiMutation` wrappers |
| `apps/dashboard/components/layout/` | New: `QuerySkeleton`, `QueryErrorState`, `QueryEmptyState` |
| `apps/dashboard/app/*/page.tsx` | All 18 pages: replace fetch+useEffect with useApiQuery |
| `apps/dashboard/app/layout.tsx` | Server prefetch + HydrationBoundary for select pages |

## Risks
| Risk | Mitigation |
|------|-----------|
| Bundle size increase (~13KB gzip) | Already have `@tanstack/react-virtual`; shared runtime reduces marginal cost. Tree-shaking removes devtools in production. |
| Migration regressions (18 pages) | Migrate in batch order (home first as proof, then high-traffic, then remaining). Each batch gate verifies build + typecheck. |
| Auto-refresh behavior change | Pages using `setInterval` for polling switch to `refetchInterval` -- test that interval timing and pause-on-blur behavior matches expectations. |
| Server prefetch hydration mismatch | Only prefetch stable data (not time-relative like "3m ago"). Timestamps use `suppressHydrationWarning` (already in use). |
| tRPC migration friction | Query key convention mirrors tRPC's `[["procedure","name"], {input}]` shape so keys are compatible without bulk rename. |
