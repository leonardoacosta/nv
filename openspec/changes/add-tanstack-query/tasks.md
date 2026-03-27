# Implementation Tasks

<!-- beads:epic:nv-wwwp -->

## DB Batch

_(No database changes required -- this is a client-side refactor.)_

## API Batch

- [ ] [2.1] [P-1] Install `@tanstack/react-query` and `@tanstack/react-query-devtools` in `apps/dashboard/package.json` and run `pnpm install` [owner:api-engineer] [beads:nv-dr9i]
- [ ] [2.2] [P-1] Create `apps/dashboard/lib/query-client.ts` -- QueryClient factory with 30s staleTime, 5min gcTime, 3 retries, exponential backoff, refetchOnWindowFocus [owner:api-engineer] [beads:nv-7gsk]
- [ ] [2.3] [P-1] Create `apps/dashboard/lib/hooks/use-api-query.ts` -- `useApiQuery<T>(path, options?)` wrapping `useQuery` + `apiFetch`, and `useApiMutation<T, V>(path, options?)` wrapping `useMutation` + `apiFetch` [owner:api-engineer] [beads:nv-b0kd]
- [ ] [2.4] [P-1] Create `apps/dashboard/lib/query-keys.ts` -- query key factory (`queryKeys.api(path, params?)`) with documentation of key convention and tRPC migration notes [owner:api-engineer] [beads:nv-gg24]

## UI Batch

- [ ] [3.1] [P-1] Create reusable `QuerySkeleton`, `QueryErrorState`, and `QueryEmptyState` components in `apps/dashboard/components/layout/` following Loading -> Error -> Empty -> Data pattern with ds-token classes [owner:ui-engineer] [beads:nv-jgem]
- [ ] [3.2] [P-1] Wrap `AppShell` children in `QueryClientProvider` using the factory from `query-client.ts`; add `ReactQueryDevtools` gated on `process.env.NODE_ENV === "development"` [owner:ui-engineer] [beads:nv-rgme]
- [ ] [3.3] [P-1] Migrate home page (`app/page.tsx`): replace 6x `apiFetch` + `Promise.allSettled` + 15 useState calls with 6 independent `useApiQuery` calls; replace `setInterval` auto-refresh with `refetchInterval: 10_000`; use `QuerySkeleton`/`QueryErrorState` for state handling [owner:ui-engineer] [beads:nv-0ono]
- [ ] [3.4] [P-1] Migrate sessions page (`app/sessions/page.tsx`): replace `fetchSessions` + useState with `useApiQuery` keyed by filter params; preserve URL search param sync and pagination [owner:ui-engineer] [beads:nv-l025]
- [ ] [3.5] [P-1] Migrate obligations page (`app/obligations/page.tsx`): replace fetch + useState with `useApiQuery`; convert create/update/delete operations to `useApiMutation` with `invalidateQueries` on success [owner:ui-engineer] [beads:nv-gkj3]
- [ ] [3.6] [P-1] Migrate messages page (`app/messages/page.tsx`): replace fetch + useState with `useApiQuery`; preserve grouping logic and pagination [owner:ui-engineer] [beads:nv-obty]
- [ ] [3.7] [P-2] Migrate remaining 14 pages (approvals, settings, memory, briefing, chat, contacts, integrations, nexus, usage, diary, projects, automations, sessions/[id], login) from raw fetch to `useApiQuery` / `useApiMutation` [owner:ui-engineer] [beads:nv-ofdm]
- [ ] [3.8] [P-2] Wire WebSocket events from `DaemonEventContext` to trigger `queryClient.invalidateQueries` for relevant query keys, replacing manual `fetchData()` callbacks on WS events [owner:ui-engineer] [beads:nv-zpk9]
- [ ] [3.9] [P-2] Implement server-side prefetching for home page: create server-side `apiFetch` variant using `next/headers` cookies, add `HydrationBoundary` in layout, prefetch critical queries (activity-feed, obligations, messages) [owner:ui-engineer] [beads:nv-r6qd]
- [ ] [3.10] [P-2] Implement server-side prefetching for briefing page: prefetch briefing data on server, wrap in `HydrationBoundary` for instant first paint [owner:ui-engineer] [beads:nv-6lh8]

## E2E Batch

- [ ] [4.1] Verify all 18 pages load correctly with query-based data fetching -- no regressions in loading states, error handling, or empty states [owner:e2e-engineer] [beads:nv-psbo]
- [ ] [4.2] Verify query invalidation: create obligation from home page quick-add bar, confirm obligations list and activity feed update without manual refresh [owner:e2e-engineer] [beads:nv-mzne]
- [ ] [4.3] Verify server prefetch: navigate to home page, confirm no loading skeleton flash on initial load (data present from server prefetch) [owner:e2e-engineer] [beads:nv-b7i4]
