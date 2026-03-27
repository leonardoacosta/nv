# Implementation Tasks
<!-- beads:epic:TBD -->

## API Batch

- [x] [1.1] [P-1] Create `apps/dashboard/app/api/sessions/analytics/route.ts` -- new GET endpoint that queries `sessions` table via Drizzle: `sessions_today` (count where `started_at >= startOfToday`), `sessions_7d` (group by date for last 7 days with count per day), `avg_duration_mins` (average of `EXTRACT(EPOCH FROM (stopped_at - started_at)) / 60` where `stopped_at IS NOT NULL`), `project_breakdown` (group by project, count, order by count desc, limit 8), `total_sessions` (total count). Return `SessionAnalyticsResponse` [owner:api-engineer]
- [x] [1.2] [P-1] Create `apps/dashboard/app/api/sessions/[id]/route.ts` -- new GET endpoint that queries `db.select().from(sessions).where(eq(sessions.id, params.id))`, maps row to `SessionDetail` shape (`id`, `service` derived from `command` field, `status` mapped from DB status, `messages: 0`, `tools_executed: 0`, `started_at`, `ended_at` from `stopped_at`, `project`), returns 404 `{ error: "Session not found" }` if no row [owner:api-engineer]
- [x] [1.3] [P-2] Add `SessionAnalyticsResponse` type to `apps/dashboard/types/api.ts` -- `{ sessions_today: number, sessions_7d: { date: string, count: number }[], avg_duration_mins: number, project_breakdown: { project: string, count: number }[], total_sessions: number }` [owner:api-engineer]

## UI Batch

- [ ] [2.1] [P-1] Add DaemonOfflineBanner component in `apps/dashboard/app/sessions/page.tsx` -- renders below filter bar when `DaemonEventContext.status` is "reconnecting" (amber bg) or "disconnected" (gray bg); shows WifiOff icon, status text ("Daemon reconnecting..." / "Daemon offline"), secondary line "Showing historical sessions. Live updates paused.", Retry button that calls `fetchSessions()`; auto-hides when status returns to "connected" [owner:ui-engineer]
- [ ] [2.2] [P-1] Modify `SessionsPage` data flow in `apps/dashboard/app/sessions/page.tsx` -- decouple session list rendering from WebSocket status: always fetch from `/api/sessions` on mount and render results; keep `useDaemonEvents` subscription to merge real-time updates by session ID (real-time takes priority over DB data, use Map keyed by ID for dedup); never clear session list on WebSocket disconnect [owner:ui-engineer]
- [ ] [2.3] [P-1] Add SessionAnalytics component in `apps/dashboard/app/sessions/page.tsx` -- fetches `GET /api/sessions/analytics` independently; renders 4 StatTile grid: "Today" (sessions_today with MiniChart sparkline of sessions_7d), "Avg Duration" (formatted Xh Ym or Xm), "Total" (total_sessions), "Projects" (count of distinct entries in project_breakdown); below tiles: horizontal bar chart for project_breakdown (inline SVG, project name left, count right, bar width proportional to max) [owner:ui-engineer]
- [ ] [2.4] [P-1] Replace empty state messages in `apps/dashboard/app/sessions/page.tsx` -- three variants: (1) no data + connected: Layers icon + "No sessions recorded yet. Sessions will appear automatically when agent commands run." (2) no data + disconnected: WifiOff icon + "No sessions found. The daemon is offline." + Retry button (3) no filter match: Search icon + "No sessions match your filters." + "Clear filters" button that resets searchInput, projectFilter, statusFilter to defaults [owner:ui-engineer]
- [ ] [2.5] [P-2] Modify `ProjectSessionsTable` in `apps/dashboard/app/sessions/page.tsx` -- add staleness indicator below CC Sessions header showing "Last updated Xm ago" using timestamp from last successful fetch; add `stopped_at` column for completed CC sessions formatted as relative time; wire refresh to same trigger as main session refresh button; preserve last-fetched data on disconnect instead of clearing [owner:ui-engineer]
- [ ] [2.6] [P-2] Add graceful degradation styling to `apps/dashboard/app/sessions/page.tsx` -- three visual states: (1) connected: no banner, green connected dot in page subtitle area (2) disconnected + data: banner above list, sessions still visible and filterable, subtle "historical" label on session cards (3) disconnected + no data: banner with prominent message, analytics section shows zero-value tiles gracefully [owner:ui-engineer]

## Verify

- [ ] [3.1] `cd apps/dashboard && pnpm typecheck` passes -- zero TypeScript errors [owner:ui-engineer]
- [ ] [3.2] `cd apps/dashboard && pnpm build` passes -- production build succeeds [owner:ui-engineer]
- [ ] [3.3] [user] Manual test: sessions page shows historical sessions from DB when daemon is offline (WebSocket disconnected)
- [ ] [3.4] [user] Manual test: daemon offline banner appears with correct state text and Retry button when WebSocket is disconnected, hides when reconnected
- [ ] [3.5] [user] Manual test: session analytics tiles show today count, avg duration, total sessions, project count with sparkline
- [ ] [3.6] [user] Manual test: project breakdown bar chart renders with correct proportions for top projects
- [ ] [3.7] [user] Manual test: `/sessions/[id]` loads session detail from DB for a historical (completed) session
- [ ] [3.8] [user] Manual test: empty states show correct contextual messages (no data vs disconnected vs no filter match)
- [ ] [3.9] [user] Manual test: CC Sessions table shows staleness indicator and preserves data when daemon disconnects
