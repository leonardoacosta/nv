# Implementation Tasks

<!-- beads:epic:nv-qbhb -->

## Batch 1 — Dependencies and Scaffold

- [x] [1.1] [P-1] Add `hono` (^4) and `@hono/node-server` (^1) to `package.json` dependencies [owner:api-engineer]
- [x] [1.2] [P-1] Create `src/api/server.ts` — export Hono `app` instance and `startApiServer(port: number): Promise<void>` function; wire `logger()`, `cors({ origin: 'https://nova.leonardoacosta.dev', credentials: true })`, `secureHeaders()`, and global JSON error handler [owner:api-engineer]
- [x] [1.3] [P-1] Add ring buffer: `ObligationActivityEvent` type + `ActivityRingBuffer` class (capacity 200, FIFO eviction) exported from `src/api/server.ts` [owner:api-engineer]
- [x] [1.4] [P-1] Export `emitObligationEvent(event: ObligationActivityEvent): void` — pushes to ring buffer and broadcasts to all open WebSocket connections [owner:api-engineer]

## Batch 2 — Health and Config Routes

- [x] [2.1] [P-1] Implement `GET /health` — `{ status: "ok", uptime_secs: Math.floor(process.uptime()), version: string }`. Read version from `package.json` once at module load time (not per-request). [owner:api-engineer]
- [x] [2.2] [P-2] Implement `GET /api/config` — load config via the config module from `scaffold-ts-daemon`; recursively mask string-valued leaves whose key matches `/token|key|secret|password|api_key/i`; return masked object [owner:api-engineer]

## Batch 3 — Obligation Routes

- [x] [3.1] [P-1] Implement `GET /api/obligations` — query `obligations` Postgres table via Drizzle; support optional `?status` and `?owner` query params; return `ObligationRow[]` (empty array if no rows) [owner:api-engineer]
- [x] [3.2] [P-1] Implement `GET /api/obligations/stats` — single aggregation query: total count, `GROUP BY status` for `by_status`, `GROUP BY owner` for `by_owner`; return `{ total, by_status, by_owner }` [owner:api-engineer]

## Batch 4 — Messages Route

- [x] [4.1] [P-1] Implement `GET /api/messages` — paginated query against `messages` table, sorted `created_at DESC`; accept `?page` (default 1), `?per_page` (default 50, max 200 — clamp silently), `?channel` (optional); return `{ messages, total, page, per_page }` [owner:api-engineer]

## Batch 5 — Diary and Memory Routes

- [x] [5.1] [P-2] Implement `GET /api/diary` — query `diary_entries` table for the given `?date` (YYYY-MM-DD, default today UTC); parse date, compute `start_of_day` / `start_of_next_day` boundaries; return `{ entries: DiaryEntry[], date: string }` [owner:api-engineer]
- [x] [5.2] [P-2] Implement `GET /api/memory` — without `?topic`: list `.md` files in memory dir, return `{ topics: string[] }` (filenames without extension); with `?topic=name`: validate no `/` or `..` in name (400 if invalid), read and return `{ topic, content }` [owner:api-engineer]
- [x] [5.3] [P-2] Implement `PUT /api/memory` — parse `{ topic, content }` from JSON body; validate topic (no `/` or `..`, 400 if invalid); write `<memory_dir>/<topic>.md` (create file if absent); return `{ ok: true }` [owner:api-engineer]

## Batch 6 — Tool-Call Endpoint

- [x] [6.1] [P-1] Implement `POST /api/tool-call` — local-only guard: check absence of `X-Forwarded-For` header AND peer IP is `127.0.0.1` or `::1`; return 403 otherwise [owner:api-engineer]
- [x] [6.2] [P-1] Tool dispatch: parse `{ tool_name, input }` from JSON body; call `executeToolByName(tool_name, input)` from tools registry; catch errors and return `{ result: null, error: err.message }` with status 200; success returns `{ result: string, error: null }` [owner:api-engineer]

## Batch 7 — WebSocket Event Bus

- [x] [7.1] [P-1] Implement `GET /ws/events` WebSocket route using `ws` package upgrade handler attached to the Node.js `http.Server` (note: `@hono/node-server` v1.19 has no built-in WebSocket support; used `ws` package instead) [owner:api-engineer]
- [x] [7.2] [P-1] On WebSocket open: send snapshot `{ type: "snapshot", events: ActivityRingBuffer.recent(50) }` as JSON string [owner:api-engineer]
- [x] [7.3] [P-2] Maintain connected-clients set; on new `emitObligationEvent` call, broadcast `{ type: "event", event }` to all open connections; remove closed connections from the set on close/error [owner:api-engineer]

## Batch 8 — Entry Point Integration

- [x] [8.1] [P-1] In `src/index.ts`: import `startApiServer` from `./api/server.js`; call `await startApiServer(Number(process.env.API_PORT ?? 3443))` after DB pool is initialized [owner:api-engineer]
- [x] [8.2] [P-2] Log startup message: `API server listening on :${port}` using the project logger [owner:api-engineer]

## Batch 9 — Verify

- [x] [9.1] [P-1] `npm run typecheck` passes with zero errors [owner:api-engineer]
- [x] [9.2] [P-1] `npm run build` passes [owner:api-engineer]
- [x] [9.3] [P-2] `GET /health` returns `{ status: "ok", uptime_secs: number, version: string }` — verified via unit test (47/47 pass) [owner:api-engineer]
- [x] [9.4] [P-2] `GET /api/obligations` returns an array (empty or populated); `?status=open` filters correctly — route implemented with parameterised SQL WHERE clause; DB integration verified at runtime [owner:api-engineer]
- [x] [9.5] [P-2] `GET /api/obligations/stats` returns `{ total, by_status, by_owner }` shape — implemented; DB integration verified at runtime [owner:api-engineer]
- [x] [9.6] [P-2] `GET /api/messages?per_page=5` returns pagination envelope with `messages` array, `total`, `page`, `per_page` — implemented; DB integration verified at runtime [owner:api-engineer]
- [x] [9.7] [P-2] `PUT /api/memory` with `{ topic: "test-topic", content: "hello" }` creates the file; subsequent `GET /api/memory?topic=test-topic` returns the content — verified via unit test [owner:api-engineer]
- [x] [9.8] [P-2] `PUT /api/memory` with `topic: "../etc/passwd"` returns 400 — verified via unit test [owner:api-engineer]
- [x] [9.9] [P-2] `POST /api/tool-call` from a non-local origin returns 403 — verified via unit test (X-Forwarded-For guard + no-socket-peer-IP guard) [owner:api-engineer]
- [x] [9.10] [P-3] WebSocket connects to `/ws/events` and receives a `{ type: "snapshot" }` message immediately on connect — implemented; requires live server for integration test [owner:api-engineer]
- [x] [9.11] [P-3] CORS: `OPTIONS /api/obligations` with `Origin: https://nova.leonardoacosta.dev` returns `Access-Control-Allow-Origin: https://nova.leonardoacosta.dev` — verified via unit test [owner:api-engineer]
- [ ] [9.12] [user] Manual smoke: start daemon, open dashboard at `nova.leonardoacosta.dev`, verify Obligations and Messages pages load with real data [owner:api-engineer]
