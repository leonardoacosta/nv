# Implementation Tasks

## Phase 1: Package Scaffold

- [x] [1.1] [P-1] Create `packages/tools/schedule-svc/package.json` — name `@nova/schedule-svc`, version `0.1.0`, type `module`, private `true`. Scripts: `dev` (tsx --watch src/index.ts), `build` (tsc), `start` (node dist/index.js), `typecheck` (tsc --noEmit). Dependencies: `hono`, `@hono/node-server`, `@modelcontextprotocol/sdk`, `pino`, `tsx`, `typescript`, `@nova/db: workspace:*`, `drizzle-orm`. Dev deps: `@types/node`, `pino-pretty`. [owner:api-engineer]
- [x] [1.2] [P-1] Create `packages/tools/schedule-svc/tsconfig.json` — strict, target ES2022, module NodeNext, moduleResolution NodeNext, outDir dist, rootDir src, declaration true, sourceMap true, esModuleInterop true, skipLibCheck true. Matches service-template pattern. [owner:api-engineer]
- [x] [1.3] [P-1] Create `packages/tools/schedule-svc/src/config.ts` — load `SERVICE_NAME` (default `schedule-svc`), `SERVICE_PORT` (default `4006`), `LOG_LEVEL` (default `info`), `CORS_ORIGIN` (default `https://nova.leonardoacosta.dev`), `DATABASE_URL` (required, throw if missing) from env. Export `ServiceConfig` type and `loadConfig()`. [owner:api-engineer]
- [x] [1.4] [P-1] Create `packages/tools/schedule-svc/src/logger.ts` — `createLogger(name)` factory wrapping pino. pino-pretty transport in development only. Level from `LOG_LEVEL` env. Matches daemon logger pattern. [owner:api-engineer]

## Phase 2: Tool Implementations

- [x] [2.1] [P-1] Create `packages/tools/schedule-svc/src/tools/cron.ts` — export `validateCron(expr: string): boolean` that validates 5-field standard cron expressions (minute hour day-of-month month day-of-week). Each field allows: numbers, `*`, ranges (`1-5`), steps (`*/15`), and lists (`1,3,5`). No external dependencies. [owner:api-engineer]
- [x] [2.2] [P-1] Create `packages/tools/schedule-svc/src/tools/reminders.ts` — implement `setReminder({ description, due_at })`, `cancelReminder({ id })`, `listReminders({ status? })`. Use Drizzle queries via `@nova/db` (import db, reminders table, eq/and/isNull from drizzle-orm). Each function returns a string result. setReminder inserts and returns the new id. cancelReminder sets cancelled=true. listReminders queries with active filter (cancelled=false AND delivered_at IS NULL) or all, ordered by due_at ASC. [owner:api-engineer]
- [x] [2.3] [P-1] Create `packages/tools/schedule-svc/src/tools/schedules.ts` — implement `addSchedule({ name, cron, action })`, `modifySchedule({ id, updates })`, `removeSchedule({ id })`, `listSchedules({ active? })`. Use Drizzle queries via `@nova/db`. addSchedule validates cron via validateCron before insert, catches unique constraint error for name. modifySchedule validates cron if provided before update. removeSchedule deletes by id. listSchedules queries with enabled filter or all, ordered by name ASC. [owner:api-engineer]
- [x] [2.4] [P-1] Create `packages/tools/schedule-svc/src/tools/sessions.ts` — implement `startSession({ name, metadata? })`, `stopSession({ name? })`. Use Drizzle queries via `@nova/db`. startSession inserts with project=name, command=JSON.stringify(metadata) or empty string, status=running. stopSession finds most recent running session (matching project name if provided, or any if omitted) via ORDER BY started_at DESC LIMIT 1, updates status=stopped and stopped_at=now(). Returns duration in minutes. [owner:api-engineer]
- [x] [2.5] [P-1] Create `packages/tools/schedule-svc/src/tools/registry.ts` — instantiate a `ToolRegistry`, register all 9 tools with name, description, JSON Schema inputSchema, and handler function. Export the registry instance for use by both HTTP and MCP transports. [owner:api-engineer]

## Phase 3: Transport Layer

- [x] [3.1] [P-1] Create `packages/tools/schedule-svc/src/http.ts` — Hono app with middleware (logger, cors, secure-headers, global error handler). Routes: GET /health, POST /reminders (set_reminder), DELETE /reminders/:id (cancel_reminder), GET /reminders (list_reminders with ?status query param), POST /schedules (add_schedule), PATCH /schedules/:id (modify_schedule), DELETE /schedules/:id (remove_schedule), GET /schedules (list_schedules with ?active query param), POST /sessions/start (start_session), POST /sessions/stop (stop_session). Each route maps request body/params to tool handler and returns JSON. Export createApp() function. [owner:api-engineer]
- [x] [3.2] [P-1] Create `packages/tools/schedule-svc/src/mcp.ts` — MCP stdio server using `@modelcontextprotocol/sdk`. Register all 9 tools from the registry with their inputSchema and handler. Logger must write to stderr (fd 2) in MCP mode to avoid corrupting stdio protocol. Export startMcpServer() function. [owner:api-engineer]
- [x] [3.3] [P-1] Create `packages/tools/schedule-svc/src/index.ts` — entry point: load config, create logger, detect `--mcp` flag in process.argv. If MCP mode: start MCP stdio server. Otherwise: start Hono HTTP server on configured port via @hono/node-server. Register SIGTERM/SIGINT handlers for graceful shutdown. [owner:api-engineer]

## Phase 4: Validation

- [x] [4.1] [P-2] Run `pnpm install` from project root to resolve workspace dependencies, then run `pnpm --filter @nova/schedule-svc typecheck` — verify zero type errors. [owner:api-engineer]
- [x] [4.2] [P-2] Run `pnpm --filter @nova/schedule-svc build` — verify tsc compiles successfully and `dist/` is produced. [owner:api-engineer]

---

## Validation Gates

| Phase | Gate |
|-------|------|
| 1 Scaffold | `pnpm install` from root resolves `@nova/db: workspace:*` without errors |
| 2 Tools | `pnpm --filter @nova/schedule-svc typecheck` passes with zero errors |
| 3 Transport | `pnpm --filter @nova/schedule-svc build` produces `dist/index.js` |
| **Final** | Service starts on port 4006 (`SERVICE_PORT=4006 DATABASE_URL=... node dist/index.js`), `GET /health` returns `{ status: "ok", service: "schedule-svc" }` |
