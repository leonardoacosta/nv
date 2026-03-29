# Implementation Tasks

<!-- beads:epic:TBD -->

## DB Batch

(No schema changes -- all tables already exist in @nova/db)

## API Batch

### Group A: Infrastructure (foundation for all other tasks)

- [x] [2.1] Add `@nova/db` as workspace dependency in `apps/dashboard/package.json`. Create `apps/dashboard/lib/db.ts` that imports and re-exports `db` from `@nova/db`. Verify `DATABASE_URL` env var is read by the client. [owner:api-engineer]
- [x] [2.2] Create `apps/dashboard/lib/fleet.ts` -- export `fleetFetch(service: string, path: string, init?: RequestInit)` helper that resolves service URLs from env vars (`TOOL_ROUTER_URL`, `MEMORY_SVC_URL`, `MESSAGES_SVC_URL`, `META_SVC_URL`) with `host.docker.internal` defaults. Include 5-second timeout per request. [owner:api-engineer]
- [x] [2.3] Remove the catch-all API rewrite (`/api/:path* -> DAEMON_URL`) from `apps/dashboard/next.config.ts`. Keep redirects unchanged. [owner:api-engineer]
- [x] [2.4] Simplify `apps/dashboard/server.ts` -- remove WebSocket proxy to daemon, remove `http-proxy` import and proxy setup, remove `verifyWsToken`, remove `DAEMON_URL`/`DAEMON_WS_URL` constants. Keep the custom HTTP server for Next.js request handling. Remove `http-proxy` from dependencies and `@types/http-proxy` from devDependencies in `package.json`. [owner:api-engineer]

### Group B: DB-backed routes (obligations)

- [x] [2.5] Rewrite `apps/dashboard/app/api/obligations/route.ts` -- replace `daemonFetch` with Drizzle query on `obligations` table. GET: select all obligations with optional `status`/`owner` filters, order by `created_at` desc. Map camelCase Drizzle fields to snake_case response to match `DaemonObligation` type (omit `notes` and `attempt_count` fields that were daemon-computed). [owner:api-engineer]
- [x] [2.6] Rewrite `apps/dashboard/app/api/obligations/[id]/route.ts` -- PATCH: update obligation by ID using Drizzle `update().set().where().returning()`. Map body fields from snake_case to camelCase for Drizzle, return updated row in snake_case. [owner:api-engineer]
- [x] [2.7] Rewrite `apps/dashboard/app/api/obligations/[id]/execute/route.ts` -- POST: update obligation status to `in_progress` and set `lastAttemptAt` to now using Drizzle. Remove daemon fallback logic. [owner:api-engineer]
- [x] [2.8] Rewrite `apps/dashboard/app/api/obligations/activity/route.ts` -- GET: select recently updated obligations ordered by `updated_at` desc with optional `limit` param. Return as activity events matching `ObligationActivity` shape. [owner:api-engineer]
- [x] [2.9] Rewrite `apps/dashboard/app/api/obligations/stats/route.ts` -- GET: count obligations grouped by status and owner using Drizzle `sql` template. Return `ObligationStats` shape. [owner:api-engineer]
- [x] [2.10] Rewrite `apps/dashboard/app/api/approvals/[id]/approve/route.ts` -- POST: update obligation status to `proposed_done` using Drizzle. Remove `daemonFetch` import. [owner:api-engineer]

### Group C: DB-backed routes (contacts, diary, briefings, sessions)

- [x] [2.11] Rewrite `apps/dashboard/app/api/contacts/route.ts` -- GET: select contacts with optional `relationship`/`q` (name search) filters. POST: insert new contact. Use Drizzle, map to snake_case response. [owner:api-engineer]
- [x] [2.12] Rewrite `apps/dashboard/app/api/contacts/[id]/route.ts` -- GET, PUT, PATCH, DELETE: full CRUD on contacts by ID using Drizzle. Map fields between camelCase (Drizzle) and snake_case (API response). [owner:api-engineer]
- [x] [2.13] Rewrite `apps/dashboard/app/api/diary/route.ts` -- GET: select diary entries with optional `date` (filter by day) and `limit` params, order by `created_at` desc. Map to `DiaryGetResponse` shape. [owner:api-engineer]
- [x] [2.14] Rewrite `apps/dashboard/app/api/briefing/route.ts` -- GET: select latest briefing (order by `generated_at` desc, limit 1). Map to `BriefingGetResponse` shape. [owner:api-engineer]
- [x] [2.15] Rewrite `apps/dashboard/app/api/briefing/history/route.ts` -- GET: select briefings with optional `limit`, order by `generated_at` desc. Map to `BriefingHistoryGetResponse` shape. [owner:api-engineer]
- [x] [2.16] Rewrite `apps/dashboard/app/api/sessions/route.ts` -- GET: select all sessions from `sessions` table, order by `started_at` desc. Map to `SessionsGetResponse` shape. [owner:api-engineer]
- [x] [2.17] Rewrite `apps/dashboard/app/api/cc-sessions/route.ts` -- GET: select sessions that represent CC sessions (filter by status or project pattern). Map to `CcSessionsGetResponse` shape. [owner:api-engineer]

### Group D: Fleet-backed routes

- [x] [2.18] Rewrite `apps/dashboard/app/api/messages/route.ts` -- GET: call `MESSAGES_SVC_URL/recent` with `channel`/`limit` params via `fleetFetch`. Transform fleet response `{ result, error }` to match `MessagesGetResponse` shape. [owner:api-engineer]
- [x] [2.19] Rewrite `apps/dashboard/app/api/memory/route.ts` -- GET: call `MEMORY_SVC_URL/read` (POST with topic body) or return topic list. PUT: call `MEMORY_SVC_URL/write` (POST with topic+content body). Transform fleet response to match existing `MemoryListResponse`/`MemoryTopicResponse`/`PutMemoryResponse` shapes. [owner:api-engineer]
- [x] [2.20] Rewrite `apps/dashboard/app/api/server-health/route.ts` -- GET: call `TOOL_ROUTER_URL/health` via `fleetFetch`. Transform fleet health response (services map with status/latency) to match `ServerHealthGetResponse` shape. [owner:api-engineer]
- [x] [2.21] Rewrite `apps/dashboard/app/api/latency/route.ts` -- GET: call `META_SVC_URL/health` or `TOOL_ROUTER_URL/health` and extract per-service latency data. Return latency metrics. [owner:api-engineer]
- [x] [2.22] Rewrite `apps/dashboard/app/api/stats/route.ts` -- GET: call `META_SVC_URL/services` via `fleetFetch` to get tool usage stats. Map response to match `StatsGetResponse` shape (or simplify if daemon-era fields are unavailable). [owner:api-engineer]

### Group E: Route removal and config routes

- [x] [2.23] Delete `apps/dashboard/app/api/solve/route.ts` -- agent dispatch is no longer a dashboard concern. [owner:api-engineer]
- [x] [2.24] Delete `apps/dashboard/app/api/cold-starts/route.ts` -- daemon-specific performance data has no fleet equivalent. [owner:api-engineer]
- [x] [2.25] Rewrite `apps/dashboard/app/api/projects/route.ts` -- GET: return project list from `NV_PROJECTS` env var (JSON string) or a hardcoded default. Remove `daemonFetch`. [owner:api-engineer]
- [x] [2.26] Rewrite `apps/dashboard/app/api/config/route.ts` -- GET: return dashboard-relevant config from env vars. PUT: return 501 Not Implemented (config changes should go through env vars/deploys, not API). Remove `daemonFetch`. [owner:api-engineer]

### Group F: Cleanup

- [x] [2.27] Delete `apps/dashboard/lib/daemon.ts`. Verify no remaining imports of `daemonFetch` or `DAEMON_URL` in the codebase. [owner:api-engineer]
- [x] [2.28] Update `apps/dashboard/types/api.ts` -- remove "daemon"/"Rust"/"Axum" references in comments, update source attribution to reflect Drizzle/fleet origins, remove types for deleted routes (solve, cold-starts). [owner:api-engineer]
- [x] [2.29] Update `docker-compose.yml` -- remove `DAEMON_URL` env var, add `DATABASE_URL`, `TOOL_ROUTER_URL`, `MEMORY_SVC_URL`, `MESSAGES_SVC_URL`, `META_SVC_URL` env vars pointing to `host.docker.internal` ports. [owner:api-engineer]

## UI Batch

(No UI changes -- pages already handle loading/error states and will render correctly once API routes return data)

## E2E Batch

- [x] [4.1] Verify dashboard builds cleanly: `pnpm typecheck` passes, no imports of `daemonFetch` or `DAEMON_URL` remain, no references to deleted files (`lib/daemon.ts`, `solve/route.ts`, `cold-starts/route.ts`). [owner:e2e-engineer]
- [ ] [4.2] Verify DB-backed routes return data: curl `/api/obligations`, `/api/contacts`, `/api/diary`, `/api/briefing`, `/api/sessions` from inside the Docker container and confirm 200 responses with valid JSON matching expected shapes. [owner:e2e-engineer]
- [ ] [4.3] Verify fleet-backed routes return data: curl `/api/messages`, `/api/memory`, `/api/server-health` from inside the Docker container and confirm 200 responses (requires fleet services running on host). [owner:e2e-engineer]
- [ ] [4.4] Verify removed routes return 404: curl `/api/solve` and `/api/cold-starts` and confirm they are no longer served. [owner:e2e-engineer]
- [ ] [4.5] Verify WebSocket proxy is removed: attempt WebSocket upgrade to `/ws/events` and confirm connection is refused or not upgraded. [owner:e2e-engineer]
