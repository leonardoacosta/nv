# Implementation Tasks

<!-- beads:epic:nv-bf9 -->

## Batch 1: Docker Container Image

- [x] [1.1] [P-1] Create `apps/dashboard/docker/cc-session/Dockerfile` — FROM node:20-slim, install `@anthropic-ai/claude-code` globally via npm, create sandbox home dir, expose stdin/stdout for stream-json, set ENTRYPOINT to `claude --input-format stream-json --output-format stream-json --dangerously-skip-permissions --no-session-persistence` [owner:api-engineer]
- [x] [1.2] [P-1] Create `apps/dashboard/docker/cc-session/.dockerignore` — exclude node_modules, .env files [owner:api-engineer]
- [x] [1.3] [P-2] Add `docker:build` and `docker:push` scripts to `apps/dashboard/package.json` for the cc-session image [owner:api-engineer]
- [x] [1.4] [P-2] Create `apps/dashboard/docker/cc-session/docker-compose.yml` (or `compose.yaml`) — service definition with named volumes (`cc-auth`, `cc-sandbox`), network attachment, env var passthrough (ANTHROPIC_MODEL), restart policy `on-failure:3` [owner:api-engineer]

## Batch 2: Session Manager Service

- [x] [2.1] [P-1] Create `apps/dashboard/lib/session-manager.ts` — `SessionManager` singleton class, wraps Docker CLI via child_process exec, tracks state: `active | idle | starting | stopping | error` [owner:api-engineer]
- [x] [2.2] [P-1] Implement `SessionManager.start()` — `docker start nova-cc-session`, poll until running (max 30s), drain init events from container stdout, transition state to `active` [owner:api-engineer]
- [x] [2.3] [P-1] Implement `SessionManager.stop()` — `docker stop nova-cc-session` with 10s graceful timeout, transition state to `stopped` [owner:api-engineer]
- [x] [2.4] [P-1] Implement `SessionManager.restart()` — stop then start, reset error counter and restart counter [owner:api-engineer]
- [x] [2.5] [P-1] Implement `SessionManager.getStatus()` — return `{ state, uptime_secs, last_message_at, message_count, error_message?, restart_count }` by inspecting container state + in-memory counters [owner:api-engineer]
- [x] [2.6] [P-2] Implement health polling loop — runs every 15s, checks container state via Docker API, detects unexpected exit and triggers auto-restart (max 3 within 5 minutes before setting state=error), detects idle (no message >30 min) [owner:api-engineer]
- [x] [2.7] [P-2] Implement `SessionManager.sendMessage(text, context)` — write stream-json input line to container stdin via `docker exec` or attached stream, read stdout until `result` event, accumulate text blocks, return reply string and processing_ms [owner:api-engineer]
- [x] [2.8] [P-2] Implement `SessionManager.getLogs(lines)` — `docker logs --tail N nova-cc-session`, return as string array [owner:api-engineer]
- [x] [2.9] [P-3] Export singleton `sessionManager` from `session-manager.ts`, initialized on module load [owner:api-engineer]

## Batch 3: Dashboard API Routes

- [x] [3.1] [P-1] Create `apps/dashboard/app/api/session/message/route.ts` — POST handler: validate `Authorization: Bearer` header against `DASHBOARD_SECRET` env var, parse `{ message_id, chat_id, text, context }`, call `sessionManager.sendMessage()`, return `{ reply, session_state, processing_ms }` or 503 if session not ready [owner:api-engineer]
- [x] [3.2] [P-1] Create `apps/dashboard/app/api/session/status/route.ts` — GET handler: return `sessionManager.getStatus()` as JSON; no auth required (dashboard-internal use) [owner:api-engineer]
- [x] [3.3] [P-2] Create `apps/dashboard/app/api/session/control/route.ts` — POST handler: `{ action: "start" | "stop" | "restart" }`, calls corresponding `SessionManager` method, returns updated status; requires Bearer auth [owner:api-engineer]
- [x] [3.4] [P-2] Create `apps/dashboard/app/api/session/logs/route.ts` — GET handler: returns `sessionManager.getLogs(50)` as `{ lines: string[] }`; dashboard-internal [owner:api-engineer]
- [x] [3.5] [P-3] Add request timeout handling to message route — abort controller with 120s timeout, return 504 if exceeded [owner:api-engineer]

## Batch 4: Dashboard UI

- [ ] [4.1] [P-1] Create `apps/dashboard/src/app/session/page.tsx` — server component shell, fetches initial status via `sessionManager.getStatus()`, renders `<SessionDashboard>` client component [owner:ui-engineer]
- [ ] [4.2] [P-1] Create `apps/dashboard/src/components/SessionDashboard.tsx` — client component: state badge, uptime counter, last activity, message count, Start/Stop/Restart buttons (disabled based on state), log viewer, error panel [owner:ui-engineer]
- [ ] [4.3] [P-2] Implement auto-refresh in `SessionDashboard` — poll `/api/session/status` every 5s with `useEffect` + `setInterval`, update UI state without full page reload [owner:ui-engineer]
- [ ] [4.4] [P-2] Implement log viewer — `<LogViewer>` sub-component, polls `/api/session/logs` every 5s, renders last 50 lines in monospace scroll container, auto-scrolls to bottom [owner:ui-engineer]
- [ ] [4.5] [P-2] Create `apps/dashboard/src/components/SessionWidget.tsx` — compact status widget for home page: state badge, last activity timestamp, message count, Restart button, link to `/session` page [owner:ui-engineer]
- [ ] [4.6] [P-3] Wire `SessionWidget` into existing dashboard home page [owner:ui-engineer]
- [ ] [4.7] [P-3] Add `/session` route to dashboard navigation [owner:ui-engineer]

## Batch 5: Daemon Message Forwarding

- [ ] [5.1] [P-1] Add `dashboard_url: Option<String>` and `dashboard_secret: Option<String>` fields to daemon config struct in `crates/nv-daemon/src/config.rs`; populate from `DASHBOARD_URL` and `DASHBOARD_SECRET` env vars [owner:api-engineer]
- [ ] [5.2] [P-1] Add `DashboardClient` struct to `crates/nv-daemon/src/` — thin HTTP client wrapping `reqwest`, with `forward_message(text, chat_id, context) -> Result<String>` that POST to `/api/session/message` with Bearer auth and 120s timeout [owner:api-engineer]
- [ ] [5.3] [P-1] In `crates/nv-daemon/src/orchestrator.rs`, add dashboard forwarding path: for triggers classified as `Query` or `Command`, if `DashboardClient` is configured and healthy, forward via `DashboardClient::forward_message()` and send reply to Telegram directly [owner:api-engineer]
- [ ] [5.4] [P-2] Implement fallback logic in orchestrator: if `DashboardClient::forward_message()` returns error or 503, fall back to existing `WorkerPool` dispatch; log warning with error detail [owner:api-engineer]
- [ ] [5.5] [P-2] Add dashboard reachability check on daemon startup — ping `/api/session/status`, log result, set initial `dashboard_healthy` flag [owner:api-engineer]
- [ ] [5.6] [P-3] Add `DashboardClient` as optional field to `SharedDeps` in `worker.rs` for any future worker-initiated dashboard calls [owner:api-engineer]

## Verify

- [ ] [6.1] cargo build passes [owner:api-engineer]
- [ ] [6.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [ ] [6.3] pnpm build (dashboard) passes [owner:ui-engineer]
- [ ] [6.4] Unit tests: `DashboardClient::forward_message` — mock server returns 200 with reply, verify response parsing; mock returns 503, verify fallback triggers [owner:api-engineer]
- [ ] [6.5] Unit tests: `SessionManager.getStatus()` — state transitions (starting → active → idle → error) [owner:api-engineer]
- [ ] [6.6] Unit tests: message route validates Bearer token — missing token returns 401, wrong token returns 403 [owner:api-engineer]
- [ ] [6.7] [user] Manual test: start CC session from dashboard, send Telegram message, verify response arrives via dashboard path (check daemon logs show "forwarding to dashboard") [owner:api-engineer]
- [ ] [6.8] [user] Manual test: stop dashboard, send Telegram message, verify fallback to local worker fires and response still arrives [owner:api-engineer]
- [ ] [6.9] [user] Manual test: force-kill CC container, verify dashboard detects crash, auto-restarts (check restart_count increments), session returns to active [owner:api-engineer]
- [ ] [6.10] [user] Manual test: session page shows correct state, log viewer updates in real time, Stop/Start/Restart buttons change state as expected [owner:ui-engineer]
