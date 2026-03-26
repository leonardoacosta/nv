# Proposal: Fix Stubbed Proxy Routes

## Change ID
`fix-stubbed-proxy-routes`

## Summary
Four Next.js API proxy routes (`/api/config`, `/api/memory`, `/api/projects`, `/api/solve`) return HTTP 501 stubs instead of proxying to the daemon. Two routes (`config`, `projects`) can be wired immediately because the daemon endpoints exist; the other two (`memory`, `solve`) require new daemon handlers first.

## Context
- Extends: `apps/dashboard/app/api/config/route.ts`, `apps/dashboard/app/api/memory/route.ts`, `apps/dashboard/app/api/projects/route.ts`, `apps/dashboard/app/api/solve/route.ts`
- Extends: `crates/nv-daemon/src/http.rs` (new `GET|PUT /api/memory` and `POST /api/solve` handlers)
- Related: `fix-dashboard-content-rendering` added `GET /api/config`, `PUT /api/config`, and `GET /api/projects` to the daemon — those daemon endpoints are complete
- Depends on: `fix-daemon-url-port` must be applied first (correct port in `DAEMON_URL`)

## Motivation
The `fix-dashboard-content-rendering` spec wired up the daemon side for config and projects but the corresponding Next.js proxy routes were never updated from their 501 stubs. The memory and solve routes have no daemon counterpart at all. The dashboard's memory page (`/memory`) and the projects "Solve with Nexus" flow are therefore completely non-functional.

## Requirements

### Req-1: Wire existing daemon endpoints (config + projects)
`apps/dashboard/app/api/config/route.ts` and `apps/dashboard/app/api/projects/route.ts` must forward to the daemon using `daemonFetch`. No daemon-side changes are needed.

- `GET /api/config` → `daemonFetch("/api/config")`, forward response body and status.
- `PUT /api/config` → `daemonFetch("/api/config", { method: "PUT", body, headers })`, forward response body and status.
- `GET /api/projects` → `daemonFetch("/api/projects")`, forward response body and status.

All three handlers must catch network errors and return `{ error: "Daemon unreachable" }` with status 502 (consistent with existing proxy routes like `/api/briefing`).

#### Scenario: config GET proxied to daemon
Given the daemon is running and `GET /api/config` returns `{ "fields": [...] }`, when the dashboard calls `GET /api/config`, then it receives the daemon response with status 200.

#### Scenario: config PUT proxied to daemon
Given a valid config update payload, when the dashboard calls `PUT /api/config` with `{ "fields": { ... } }`, then `daemonFetch` is called with method PUT, the body is forwarded, and the daemon response is returned.

#### Scenario: projects GET proxied to daemon
Given the daemon is running, when `GET /api/projects` is called, then `{ projects: [...] }` is returned with status 200.

#### Scenario: daemon unreachable returns 502
Given the daemon is not running, when any of the three routes are called, then the response is `{ "error": "Daemon unreachable" }` with status 502.

### Req-2: Add daemon endpoints for memory (GET + PUT)
The daemon has no `GET /api/memory` or `PUT /api/memory` route. These must be added to `crates/nv-daemon/src/http.rs` using the existing `Memory` struct from `crates/nv-daemon/src/memory.rs`.

- `GET /api/memory` — no `?topic` param: returns `{ topics: [<filenames>] }` (list of topic names). With `?topic=<name>`: returns `{ topic: <name>, content: <string> }`.
- `PUT /api/memory` — accepts `{ topic: string, content: string }` body, writes the topic file via `Memory::write_topic` (or equivalent), returns `{ topic: <name>, written: <byte_count> }`.

The response shapes match the TypeScript types already defined in `apps/dashboard/types/api.ts` (`MemoryListResponse`, `MemoryTopicResponse`, `PutMemoryRequest`, `PutMemoryResponse`).

`HttpState` must expose the memory base path or a `Memory` instance so handlers can access it.

#### Scenario: list topics
Given the memory directory contains `projects.md` and `decisions.md`, when `GET /api/memory` is called without a `?topic` param, then the response is `{ "topics": ["projects", "decisions"] }` (filenames without extension, or with — match what `Memory` exposes).

#### Scenario: read topic
Given a memory topic `projects` exists, when `GET /api/memory?topic=projects` is called, then the response is `{ "topic": "projects", "content": "<file content>" }`.

#### Scenario: write topic
Given a valid `PUT /api/memory` body `{ "topic": "projects", "content": "# Projects\n..." }`, then the file is written and the response is `{ "topic": "projects", "written": <N> }` with status 200.

#### Scenario: missing topic returns 404
Given `?topic=nonexistent` is requested and the file does not exist, when `GET /api/memory?topic=nonexistent` is called, then the response is HTTP 404 with `{ "error": "Topic not found" }`.

### Req-3: Wire memory proxy route
Once Req-2 is complete, `apps/dashboard/app/api/memory/route.ts` must proxy to the daemon.

- `GET /api/memory` — forward optional `?topic=` query param: `daemonFetch("/api/memory?topic=<val>")` when present, else `daemonFetch("/api/memory")`.
- `PUT /api/memory` — forward body: `daemonFetch("/api/memory", { method: "PUT", body, headers })`.

Standard 502 fallback on network error.

#### Scenario: list forwarded
Given `GET /api/memory` with no query param, then the proxy calls `daemonFetch("/api/memory")` and returns the daemon response.

#### Scenario: topic query forwarded
Given `GET /api/memory?topic=projects`, then the proxy calls `daemonFetch("/api/memory?topic=projects")` and returns the daemon response.

### Req-4: Add daemon endpoint for solve (POST) and wire proxy route
The daemon has no `POST /api/solve` route. The `/projects` page POSTs `{ project, error, context }` and expects `{ session_id }` in response.

The daemon handler should accept this payload and start a Claude Code session (or equivalent background task) for the specified project and error. It returns `{ session_id: <uuid> }` immediately and runs the session asynchronously.

If the `ask_handler` or session-start infrastructure in `http.rs` is reusable, use it. Otherwise create a minimal stub that returns `{ session_id: <uuid> }` synchronously (a placeholder that prevents the 501 from blocking UI development).

`apps/dashboard/app/api/solve/route.ts` must then proxy `POST /api/solve` to the daemon.

#### Scenario: solve POST proxied
Given `POST /api/solve` with `{ "project": "nv", "error": "build failed", "context": "Cargo.toml:12" }`, when the proxy route is called, then `daemonFetch("/api/solve", { method: "POST", ... })` is called and the daemon response is returned.

#### Scenario: solve returns session_id
Given the daemon handler is implemented, when it receives a valid solve request, then it responds with `{ "session_id": "<uuid>" }` and status 200.

## Scope
- **IN**: `apps/dashboard/app/api/{config,memory,projects,solve}/route.ts` — replace 501 stubs with real proxies
- **IN**: `crates/nv-daemon/src/http.rs` — add `GET /api/memory`, `PUT /api/memory`, `POST /api/solve` handlers
- **IN**: `HttpState` in `http.rs` — expose memory path/instance if not already present
- **OUT**: Changes to dashboard UI pages (shapes already match daemon types via `types/api.ts`)
- **OUT**: Authentication or authorization on any of these endpoints
- **OUT**: Changes to existing daemon routes (`/api/config`, `/api/projects` are already correct)

## Impact
| Area | Change |
|------|--------|
| `apps/dashboard/app/api/config/route.ts` | Replace 501 stub with `daemonFetch` proxy (GET + PUT) |
| `apps/dashboard/app/api/projects/route.ts` | Replace 501 stub with `daemonFetch` proxy (GET) |
| `apps/dashboard/app/api/memory/route.ts` | Replace 501 stub with `daemonFetch` proxy (GET + PUT) |
| `apps/dashboard/app/api/solve/route.ts` | Replace 501 stub with `daemonFetch` proxy (POST) |
| `crates/nv-daemon/src/http.rs` | Add `GET/PUT /api/memory` handlers, add `POST /api/solve` handler, register routes in `build_router` |
| `crates/nv-daemon/src/state.rs` or `http.rs` | Expose `Memory` instance or base path in `HttpState` |

## Risks
| Risk | Mitigation |
|------|-----------|
| `Memory` struct not thread-safe or not clonable for `HttpState` | Wrap in `Arc<Mutex<Memory>>` or store only the `base_path: PathBuf` and construct `Memory` per request |
| `POST /api/solve` daemon implementation scope creep (full session management) | Start with a minimal placeholder returning a generated UUID; full session wiring is out of scope for this spec |
| `fix-daemon-url-port` not yet merged | This spec depends on it — do not apply until port is corrected |
