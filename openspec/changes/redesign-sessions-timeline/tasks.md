# Implementation Tasks

<!-- beads:epic:nv-wj4s -->

## DB Batch

- [x] [1.1] [P-1] Add `trigger_type` (text nullable), `message_count` (integer default 0), `tool_count` (integer default 0) columns to `sessions` table in `packages/db/src/schema/sessions.ts` [owner:db-engineer] [beads:nv-2g86]
- [x] [1.2] [P-1] Create `session_events` table in `packages/db/src/schema/session-events.ts` with columns: id (uuid PK), session_id (uuid FK to sessions.id), event_type (text), direction (text nullable), content (text nullable), metadata (jsonb nullable), created_at (timestamp with tz) -- add index on (session_id, created_at) [owner:db-engineer] [beads:nv-eya9]
- [x] [1.3] [P-2] Export `sessionEvents`, `SessionEvent`, `NewSessionEvent` from `packages/db/src/index.ts` [owner:db-engineer] [beads:nv-vfpn]
- [x] [1.4] [P-2] Run `pnpm drizzle-kit generate` to create migration for new columns and table [owner:db-engineer] [beads:nv-iyvl]

## API Batch

- [x] [2.1] [P-1] Extend `GET /api/sessions` route to accept query params: `page` (default 1), `limit` (default 25), `project`, `trigger_type`, `date_from`, `date_to` -- apply filters server-side with Drizzle `where` clauses and return `{ sessions, total, page, limit }` [owner:api-engineer] [beads:nv-jda8]
- [x] [2.2] [P-1] Create `GET /api/sessions/[id]/events` route that returns all `session_events` for a given session_id ordered by created_at ascending, with response shape `{ events: SessionEvent[] }` [owner:api-engineer] [beads:nv-s3jc]
- [x] [2.3] [P-2] Add `SessionTimelineItem`, `SessionListResponse`, `SessionEventsResponse` types to `apps/dashboard/types/api.ts` [owner:api-engineer] [beads:nv-lq3c]
- [x] [2.4] [P-2] Update `GET /api/sessions/[id]` route to include `trigger_type`, `message_count`, `tool_count` in the response [owner:api-engineer] [beads:nv-x7uf]

## UI Batch

- [x] [3.1] [P-1] Rewrite `apps/dashboard/app/sessions/page.tsx` -- remove all WebSocket/daemon logic (useDaemonEvents, useDaemonStatus, DaemonOfflineBanner, sessionMapRef, real-time merge), remove CCSessionPanel import and toggle, remove SessionDetailDrawer, remove active/idle/completed grouping; replace with paginated reverse-chronological session list fetched from `GET /api/sessions` with query params [owner:ui-engineer] [beads:nv-vsx5]
- [x] [3.2] [P-1] Build session row component showing: project name, duration (computed from started_at/stopped_at), message_count, tool_count, status badge, trigger_type badge, relative timestamp -- each row links to `/sessions/[id]` [owner:ui-engineer] [beads:nv-8bbw]
- [x] [3.3] [P-1] Build filter bar with project dropdown (populated from `GET /api/sessions` distinct projects or a dedicated endpoint), date range picker (two native date inputs), trigger type selector (all/manual/watcher/briefing), and retain existing text search -- filters update URL search params and refetch [owner:ui-engineer] [beads:nv-0vy3]
- [x] [3.4] [P-2] Add pagination control (load more button or page numbers) that fetches next page from API and appends results [owner:ui-engineer] [beads:nv-cpdy]
- [x] [3.5] [P-1] Rewrite `apps/dashboard/app/sessions/[id]/page.tsx` -- remove daemon real-time subscription, fetch session metadata from `GET /api/sessions/[id]` and events from `GET /api/sessions/[id]/events`; render session header with stat tiles (project, status, duration, trigger, model, tokens, cost) and vertical timeline of events below [owner:ui-engineer] [beads:nv-58bt]
- [x] [3.6] [P-1] Build timeline event components: MessageEvent (direction arrow + content), ToolCallEvent (tool name + truncated inputs + expand/collapse for full I/O), ApiRequestEvent (method + endpoint + status code badge with color coding) [owner:ui-engineer] [beads:nv-i6jj]
- [x] [3.7] [P-2] Add empty state for session detail when no events exist ("No interactions recorded for this session") [owner:ui-engineer] [beads:nv-fgdq]
- [x] [3.8] [P-2] Add CC Sessions summary widget to `apps/dashboard/app/page.tsx` -- compact card showing running session count, status dot, and "View all" link; fetch from `GET /api/cc-sessions` [owner:ui-engineer] [beads:nv-pkxr]
- [x] [3.9] [P-2] Add "Back to Sessions" link in session detail header that preserves filter state via URL search params [owner:ui-engineer] [beads:nv-xbjs]

## E2E Batch

- [x] [4.1] Verify sessions page loads without WebSocket dependency and displays session rows from DB [owner:e2e-engineer] [beads:nv-68c0]
- [x] [4.2] Verify session detail page loads and renders timeline events for a known session [owner:e2e-engineer] [beads:nv-oahj]
- [x] [4.3] Verify filter controls (project, date range, trigger type) update the displayed session list [owner:e2e-engineer] [beads:nv-ivcr]
