# Proposal: Sessions Resilience & Metrics

## Change ID
`sessions-resilience`

## Summary

Make the sessions page resilient to daemon disconnections by always showing historical session data from DB, replace the dead empty state with a daemon-offline banner, and add a session analytics section with daily counts, average duration, tool calls, project breakdown, and token usage trends.

## Context
- Extends: `apps/dashboard/app/sessions/page.tsx`, `apps/dashboard/app/sessions/[id]/page.tsx`, `apps/dashboard/app/api/sessions/route.ts`, `apps/dashboard/app/api/cc-sessions/route.ts`, `apps/dashboard/types/api.ts`
- Related: `rewire-dashboard-api` (API layer restructuring), `redesign-dashboard-home` (established activity feed + priority banner patterns)
- DB schema: `packages/db/src/schema/sessions.ts` -- `sessions` table with `id`, `project`, `command`, `status`, `started_at`, `stopped_at`
- WebSocket: `apps/dashboard/components/providers/DaemonEventContext.tsx` -- `DaemonEventProvider` with exponential backoff reconnection, exposes `WsStatus` ("connected" | "reconnecting" | "disconnected")
- Existing chart components: `MiniChart`, `UsageSparkline`, `LatencyChart` in `apps/dashboard/components/`

## Motivation

The sessions page is a dead end when the daemon is offline. Both `/api/sessions` and `/api/cc-sessions` query the DB directly (not the daemon), but the page renders an empty state with "Sessions will appear here when the daemon is active" when no sessions are returned. This is misleading -- sessions DO exist in the DB, the page just fails to surface them when the daemon WebSocket is disconnected.

1. **False empty state**: The page says "no sessions" when the daemon is down, but the `sessions` table has historical data. The API routes already query Postgres directly via Drizzle -- they do not depend on the daemon. The empty state is caused by the client-side rendering logic, not a data availability problem.
2. **No offline resilience**: When `DaemonEventContext` reports "reconnecting" or "disconnected", the UI shows "RECONNECTING..." with full opacity reduction. Historical data should still be visible underneath. The current approach treats daemon disconnection as total data loss.
3. **No session metrics**: There are no analytics for sessions -- no daily count, no average duration, no tool call stats, no project breakdown. The existing `UsageSparkline` and `MiniChart` components provide the building blocks but nothing aggregates session data.
4. **Dead empty state**: When no sessions match filters (or on initial load with no data), the message offers no action. It should distinguish "daemon offline, showing historical data" from "genuinely no sessions ever" and offer a reconnect action in the former case.
5. **Session detail lacks historical support**: `/sessions/[id]` fetches from `/api/sessions/${id}` but that dynamic route does not exist -- there is only `GET /api/sessions` (list). The detail page silently fails for any session ID.
6. **CC Sessions show only live data**: `ProjectSessionsTable` fetches once on mount with no auto-refresh and no indication of staleness. Historical CC session data is available in DB but not surfaced when the daemon is down.

## Requirements

### Req-1: Always Show Historical Sessions from DB

The sessions page must always display sessions from the `sessions` table regardless of daemon WebSocket status. The existing `GET /api/sessions` and `GET /api/cc-sessions` routes already query Postgres directly -- this requirement is about the client-side rendering behavior.

- On page load, fetch from `/api/sessions` and render results immediately. Do NOT gate rendering on WebSocket connection status.
- When WebSocket is connected, overlay real-time updates on top of the DB-fetched sessions (existing `useDaemonEvents` subscription for "session" events). Merge strategy: if a real-time session ID matches a DB session, use the real-time data (more current); if the real-time session is new (not in DB), prepend it to the list.
- When WebSocket is disconnected, continue showing the last-fetched DB data without clearing the session list. The existing auto-refresh (manual refresh button) still works since it calls the API route (which hits Postgres).
- Remove the condition that produces "Sessions will appear here when the daemon is active" -- replace with the daemon offline banner from Req-2 when the WebSocket is disconnected AND no sessions exist in DB.

### Req-2: Daemon Offline Banner

Replace the dead empty state with a contextual banner when `DaemonEventContext.status` is "reconnecting" or "disconnected":

- **Banner placement**: Below the filter bar, above the session list. Full width. Amber background for "reconnecting", muted/gray for "disconnected".
- **Banner content**: Icon (WifiOff or similar), status text ("Daemon reconnecting..." or "Daemon offline"), and a secondary line: "Showing historical sessions. Live updates paused."
- **Reconnect action**: A "Retry" button that forces a WebSocket reconnection attempt (if the DaemonEventContext exposes this) or at minimum refreshes the API data.
- **Auto-dismiss**: Banner disappears when WebSocket status returns to "connected".
- **Coexistence**: The banner renders above the session list, NOT instead of it. Historical sessions remain visible below the banner.

### Req-3: Graceful Degradation UX

Define three visual states for the page:

1. **Connected + data**: No banner. Full session list with real-time updates. Green connected indicator in the header or status area.
2. **Disconnected + historical data**: Amber/gray banner (Req-2) above the session list. Session list shows DB data. Filters still work. Status tabs still work. Session cards show a subtle "last synced" indicator using `started_at` timestamp.
3. **Disconnected + no data**: Banner (Req-2) with a more prominent message: "No sessions found. The daemon may not have recorded any sessions yet." Still show the analytics section (Req-4) which will show zeros/empty states gracefully.

The page should NEVER show a blank content area. There is always either data or a meaningful empty state with context.

### Req-4: Session Analytics Section

Add a session analytics section above the session list (below the filter bar, below any daemon banner). Create a new API route `GET /api/sessions/analytics` that computes aggregates from the `sessions` table.

**API endpoint** (`apps/dashboard/app/api/sessions/analytics/route.ts`):

Query the `sessions` table with Drizzle and return:

- `sessions_today`: count of sessions where `started_at >= start of today (UTC)`
- `sessions_7d`: array of 7 objects `{ date: string, count: number }` for the last 7 days (for sparkline)
- `avg_duration_mins`: average duration in minutes across all sessions with `stopped_at` set (completed sessions only), computed as `AVG(EXTRACT(EPOCH FROM (stopped_at - started_at)) / 60)`
- `project_breakdown`: array of `{ project: string, count: number }` grouped by project, sorted by count desc, top 8
- `total_sessions`: total count of all sessions in the table

Response shape: `{ sessions_today: number, sessions_7d: { date: string, count: number }[], avg_duration_mins: number, project_breakdown: { project: string, count: number }[], total_sessions: number }`

**UI rendering**:

- Use a compact 4-tile grid (matching the StatTile pattern from `sessions/[id]/page.tsx`): "Today" (count with mini sparkline of 7d trend), "Avg Duration" (formatted as Xh Ym or Xm), "Total" (all-time count), "Projects" (count of distinct projects)
- Below the tiles, a "Project Breakdown" mini bar chart showing top 8 projects by session count. Use horizontal bars (no external charting library -- inline SVG like `MiniChart`). Each bar: project name left-aligned, count right-aligned, bar width proportional to max count.
- The analytics section fetches independently from the session list and has its own loading/error states (skeleton tiles while loading, error text on failure, does not block the session list).

### Req-5: Session Detail API Route

Create `apps/dashboard/app/api/sessions/[id]/route.ts` as a dynamic route that returns a single session by ID:

- Query `db.select().from(sessions).where(eq(sessions.id, params.id))` via Drizzle
- Map the row to `SessionDetail` shape (matching what `sessions/[id]/page.tsx` expects): `id`, `service` (derive from `command` field -- "CLI" for claude commands, "Telegram" for telegram, default to the command value), `status` (map "running" to "active", others as-is), `messages` (0 -- not tracked in current schema), `tools_executed` (0 -- not tracked), `started_at`, `ended_at` (from `stopped_at`), `project`
- Return 404 with `{ error: "Session not found" }` if no row matches
- This makes the existing `sessions/[id]/page.tsx` work for historical sessions without requiring daemon connectivity

### Req-6: CC Sessions Historical Data

Modify `ProjectSessionsTable` (inline in `sessions/page.tsx`) to:

- Auto-refresh CC sessions on the same interval as the main session list (or at minimum on manual refresh)
- Show a "stale data" indicator when daemon is disconnected: small muted text below the table header like "Last updated 2m ago" using a timestamp from the last successful fetch
- When daemon is disconnected, continue showing the last-fetched data rather than clearing to empty
- Add `stopped_at` column to the CC sessions table (in addition to existing ID, Project, State, Duration, Restarts) for historical sessions, formatted as a relative time ("2h ago", "yesterday")

### Req-7: Enhanced Empty States

Replace the generic "No sessions found" messages with contextual empty states:

- **No sessions + daemon connected**: "No sessions recorded yet. Sessions will appear automatically when agent commands run."
- **No sessions + daemon disconnected**: "No sessions found. The daemon is offline -- sessions will sync when connectivity is restored." with a Retry button.
- **No sessions matching filter**: "No sessions match your filters." with a "Clear filters" button that resets search, project, and status filter to defaults.
- Each empty state uses a Lucide icon (Layers for no data, WifiOff for disconnected, Search for no filter match), muted text, and an optional action button.

## Scope
- **IN**: Client-side resilience (always show DB data), daemon offline banner, session analytics API route + UI, session detail API route (`/api/sessions/[id]`), CC sessions historical data display, enhanced empty states, graceful degradation UX
- **OUT**: Modifying the `sessions` DB schema (no new columns), daemon WebSocket protocol changes, token usage tracking (would require schema changes), tool call tracking (would require schema changes), session recording logic in the daemon, new chart library dependencies

## Impact

| Area | Change |
|------|--------|
| `apps/dashboard/app/sessions/page.tsx` | MODIFY -- add daemon offline banner, analytics section, enhanced empty states, merge real-time + DB data, CC sessions staleness indicator |
| `apps/dashboard/app/sessions/[id]/page.tsx` | MODIFY -- handle missing detail endpoint gracefully, show historical data |
| `apps/dashboard/app/api/sessions/analytics/route.ts` | NEW -- aggregated session metrics endpoint |
| `apps/dashboard/app/api/sessions/[id]/route.ts` | NEW -- single session detail endpoint from DB |
| `apps/dashboard/types/api.ts` | MODIFY -- add `SessionAnalyticsResponse` type |

## Risks

| Risk | Mitigation |
|------|-----------|
| Analytics query may be slow with large session tables | Use indexed `started_at` column for date-range filters; the 7-day query and today count use simple `WHERE started_at >= ?` conditions; `avg_duration_mins` only aggregates completed sessions with `stopped_at IS NOT NULL` |
| Session detail endpoint returns sparse data (messages=0, tools=0) | Acceptable for now -- the detail page already handles missing fields gracefully with "0" display and conditional rendering. Schema additions for richer tracking are explicitly out of scope. |
| Merge logic between real-time WebSocket sessions and DB sessions could cause duplicates | Merge by session ID: real-time data takes priority (more current), DB data is the baseline. Use a Map keyed by ID to deduplicate. |
| DaemonEventContext may not expose a reconnect action | The "Retry" button in the banner can fall back to just calling `fetchSessions()` to refresh DB data, with the reconnection happening on its own via the existing exponential backoff. |
| CC sessions auto-refresh adds polling load | Use the same manual refresh trigger as the main session list (user-initiated), not an automatic interval. Add a shared refresh timestamp to coordinate. |
