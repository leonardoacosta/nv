# Proposal: Add HTTP API Server

## Change ID
`add-http-api`

## Summary

Add a Hono HTTP API server to the TypeScript daemon (`src/api/server.ts`) that exposes obligation
data, message history, config, diary, memory, and a tool-call bridge to the dashboard and future
MCP clients. Includes a WebSocket event bus at `/ws/events` for real-time obligation activity
and session updates.

## Context
- Phase: Foundation | Wave: 3
- Stack: TypeScript (Hono + `@hono/node-server`)
- Depends on: `scaffold-ts-daemon` (project structure, entry point, env config),
  `setup-postgres-drizzle` (DB connection pool, schema, query helpers)
- Extends: `src/api/server.ts` (new), `src/index.ts` (entry point — registers the HTTP server)
- Related: `apps/dashboard/` Next.js app consumes these routes; `replace-anthropic-with-agent-sdk`
  spec defines the `POST /api/tool-call` contract that this spec implements on the receiver side
- CORS: `nova.leonardoacosta.dev` only

## Motivation

The TypeScript daemon needs an HTTP surface so the Next.js dashboard can read live data without
going through the Rust daemon. The Rust `http.rs` serves as the reference implementation —
this spec ports that surface to TypeScript in a clean, minimal form that is easy to extend.

Key motivations:

1. **Dashboard data source** — obligations, messages, config, diary, and memory are all read by
   the Next.js dashboard. Without this API, the dashboard cannot display TypeScript daemon data.
2. **Real-time activity feed** — the WebSocket event bus replaces polling for obligation lifecycle
   events. The dashboard subscribes once and receives push updates.
3. **MCP tool bridge** — `POST /api/tool-call` lets the Python Agent SDK sidecar execute any
   registered tool via HTTP. This is the glue between the Agent SDK and the TypeScript tools layer.
4. **Health endpoint** — required for Docker health checks, load balancer probes, and systemd
   `sd-notify` equivalent.

## Requirements

### Req-1: Hono App Scaffold — `src/api/server.ts`

Create `src/api/server.ts` exporting a Hono app instance and a `startApiServer(port: number)`
function.

Dependencies to add to `package.json`:
```
hono            ^4
@hono/node-server ^1
```

App-level middleware:
- `logger()` from `hono/logger` — structured request logging
- `cors({ origin: 'https://nova.leonardoacosta.dev', credentials: true })` — single allowed origin
- `secureHeaders()` from `hono/secure-headers`
- Global JSON error handler — catch unhandled errors, return `{ error: message, status: 500 }`

### Req-2: Health Endpoint

```
GET /health
→ { status: "ok", uptime_secs: number, version: string }
```

- `uptime_secs`: `Math.floor(process.uptime())`
- `version`: read from `package.json` at startup (inject as a startup-time constant, not a
  per-request `fs.readFileSync` call)

### Req-3: Obligations Endpoints

```
GET /api/obligations
  → ObligationRow[]
  Query params: ?status=open|pending|done   (optional, unfiltered if absent)
               ?owner=string                (optional)

GET /api/obligations/stats
  → { total: number, by_status: Record<string, number>, by_owner: Record<string, number> }
```

Both routes query the `obligations` Postgres table via Drizzle (from `setup-postgres-drizzle`).
Return empty arrays / zero counts when the table has no rows — never 404.

### Req-4: Messages Endpoint

```
GET /api/messages
  → { messages: MessageRow[], total: number, page: number, per_page: number }
  Query params: ?page=1 (default 1), ?per_page=50 (default 50, max 200)
               ?channel=string  (optional)
```

Paginated query against the `messages` Postgres table. Sorted newest-first (`created_at DESC`).
Returns structured pagination envelope — not a bare array — so the dashboard can implement
"load more".

### Req-5: Config Endpoint

```
GET /api/config
  → masked config object
```

Read the daemon's config (loaded via `scaffold-ts-daemon`). Before returning, mask any field
whose key contains `token`, `key`, `secret`, `password`, or `api_key` (case-insensitive) by
replacing the value with `"[redacted]"`. Return the full config tree with sensitive values masked.

### Req-6: Diary Endpoint

```
GET /api/diary
  → { entries: DiaryEntry[], date: string }
  Query params: ?date=YYYY-MM-DD  (default: today in UTC)
```

Read diary entries from the `diary_entries` Postgres table for the given date range
(`date >= start_of_day AND date < start_of_next_day`). Return ISO timestamps on all entries.

### Req-7: Memory Endpoints

```
GET /api/memory
  → { topics: string[] }              (no ?topic param)
  → { topic: string, content: string } (with ?topic=name)

PUT /api/memory
  Body: { topic: string, content: string }
  → { ok: true }
```

Memory is stored as files on disk (from the `scaffold-ts-daemon` memory directory, e.g.
`~/.nova/memory/<topic>.md`). The `GET` without `?topic` lists all `.md` filenames in the
memory directory (strip the `.md` extension). The `GET` with `?topic` reads and returns the
file content. `PUT` writes the content to `<topic>.md`, creating the file if it does not exist.

Error if `topic` param contains `/` or `..` — return `{ error: "invalid topic name" }` with
status 400 to prevent path traversal.

### Req-8: Tool-Call Endpoint

```
POST /api/tool-call
  Body: { tool_name: string, input: Record<string, unknown> }
  → { result: string, error: null }
  → { result: null, error: string }
```

Local-only guard: if `X-Forwarded-For` is present OR the request origin is not `127.0.0.1` /
`::1`, return 403. This endpoint is intended only for the Python Agent SDK sidecar running on
the same host.

Dispatch: call `executeToolByName(tool_name, input)` from the tools registry
(provided by `scaffold-ts-daemon`). Any thrown error is caught and returned as
`{ result: null, error: err.message }` with status 200 (never 500 — the caller expects the
error in the response body, not the HTTP status code).

### Req-9: WebSocket Event Bus — `/ws/events`

```
GET /ws/events  (Upgrade: websocket)
```

Hono's built-in WebSocket support via `upgradeWebSocket` from `hono/ws`.

On connection: send a snapshot of the last 50 obligation activity events as a single
`{ type: "snapshot", events: ObligationActivityEvent[] }` JSON message.

Ongoing: broadcast `{ type: "event", event: ObligationActivityEvent }` to all connected clients
whenever a new obligation activity event is emitted.

Event emission API (internal): export `emitObligationEvent(event: ObligationActivityEvent)`
from `src/api/server.ts`. The daemon's obligation runner imports this and calls it at each
lifecycle transition (detected, started, tool_called, completed, failed).

Ring buffer: keep the last 200 events in memory (same as the Rust `ActivityRingBuffer`). The
buffer feeds both the snapshot on new connections and the history endpoint if added later.

### Req-10: Entry Point Integration

`src/index.ts` (from `scaffold-ts-daemon`) should start the API server after the DB pool is
ready:

```typescript
import { startApiServer } from './api/server.js';
// after db init:
await startApiServer(Number(process.env.API_PORT ?? 3443));
```

The port `3443` matches the existing Rust daemon port so the Next.js dashboard proxy
(`DAEMON_URL=http://nv-daemon:3443`) requires no configuration change.

## Scope
- **IN**: Hono app, 8 REST routes, 1 WebSocket route, CORS, masking, pagination, path-traversal
  guard, tool-call bridge, ring buffer, event emission API, `startApiServer` export
- **OUT**: Auth / API key protection (dashboard is Tailscale-only), rate limiting, OpenAPI schema
  generation, metrics endpoint, POST/PATCH/DELETE on obligations or messages (read-only for now
  except memory PUT and tool-call POST)

## Impact
| Area | Change |
|------|--------|
| `src/api/server.ts` | New: Hono app, all routes, WebSocket, ring buffer, event emitter |
| `src/index.ts` | Add `startApiServer()` call after DB init |
| `package.json` | Add `hono` and `@hono/node-server` dependencies |

## Risks
| Risk | Mitigation |
|------|-----------|
| Hono WebSocket support with `@hono/node-server` requires specific setup | Use `createNodeWebSocket` helper from `@hono/node-server/ws`; document in code |
| Memory path traversal | Explicit `topic.includes('/')` and `topic.includes('..')` guard before any fs call |
| Tool-call endpoint exposed to network | Local-only guard checks `req.header('x-forwarded-for')` absence and peer IP; returns 403 otherwise |
| CORS misconfiguration breaks dashboard | Single origin string (not wildcard) set at app level; test in verify tasks |
| Config masking misses nested keys | Recursive mask walk over all string-valued leaves whose key matches the sensitive pattern |
