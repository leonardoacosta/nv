# Proposal: Fix Dashboard API Proxy Routes

## Change ID
`fix-dashboard-api-proxy`

## Summary
Audit every Next.js API proxy route in `apps/dashboard/app/api/` against the actual daemon HTTP
endpoints in `crates/nv-daemon/src/http.rs`, then add missing proxy routes and fix broken ones so
all dashboard pages load without 404/502 errors.

## Context
- Extends: `apps/dashboard/app/api/` (existing proxy routes), `crates/nv-daemon/src/http.rs` (source of truth for daemon endpoints)
- Related: none (no relevant archived specs)

## Motivation
Several dashboard pages show API errors at runtime because their proxy routes either target a daemon
endpoint path that does not exist, or no proxy route exists at all. The daemon's `build_router` in
`http.rs` is the authoritative list of HTTP endpoints; the dashboard proxies must mirror it exactly.

## Requirements

### Req-1: Add Missing Proxy Routes

The following daemon endpoints exist but have no corresponding Next.js proxy route. Each must be
added under `apps/dashboard/app/api/`:

| Daemon endpoint | Next.js route file | Method(s) | Called by page |
|---|---|---|---|
| `GET /stats` | `app/stats/route.ts` | GET | `/usage` calls `fetch("/stats")` — needs non-`/api/` path |
| `GET /api/briefing` | `app/api/briefing/route.ts` | GET | `/briefing` |
| `GET /api/briefing/history` | `app/api/briefing/history/route.ts` | GET | `/briefing` |
| `GET /api/cold-starts` | `app/api/cold-starts/route.ts` | GET | `/cold-starts` |
| `POST /api/approvals/{id}/approve` | `app/api/approvals/[id]/approve/route.ts` | POST | `/approvals` |
| `GET /api/contacts` + `POST /api/contacts` | `app/api/contacts/route.ts` | GET, POST | (future `/contacts` page) |
| `GET /api/contacts/{id}` + `PUT` + `DELETE` | `app/api/contacts/[id]/route.ts` | GET, PUT, DELETE | (future `/contacts` page) |
| `GET /api/diary` | `app/api/diary/route.ts` | GET | `/diary` |

#### Scenario: Briefing page loads without 404

Given the daemon is running and has a morning briefing stored,
when the `/briefing` page mounts,
then `fetch("/api/briefing")` and `fetch("/api/briefing/history?limit=10")` both return 200 with
JSON data, and the briefing content is displayed.

#### Scenario: Diary page loads without 404

Given the daemon is running,
when the `/diary` page mounts,
then `fetch("/api/diary?date=YYYY-MM-DD&limit=100")` returns 200 with JSON, and diary entries are
displayed.

#### Scenario: Cold-starts page loads without 404

Given the daemon is running,
when the `/cold-starts` page mounts,
then `fetch("/api/cold-starts?limit=200")` returns 200 with JSON data.

#### Scenario: Usage page loads without 404

Given the daemon is running,
when the `/usage` page mounts,
then `fetch("/stats")` (note: no `/api/` prefix) returns 200 with JSON from the daemon's
`/stats` endpoint, and usage data is displayed.

#### Scenario: Approvals page can approve an obligation

Given an open obligation with id `abc`,
when the user clicks approve on the `/approvals` page,
then `fetch("/api/approvals/abc/approve", { method: "POST" })` returns 200, and the obligation
status updates to done.

### Req-2: Fix Broken Proxy Routes

The following proxy routes exist but target daemon paths that are not registered in
`build_router`, causing guaranteed 404s from the daemon:

| Next.js proxy | Calls daemon path | Issue |
|---|---|---|
| `app/api/memory/route.ts` | `/api/memory` | Daemon has no `/api/memory` endpoint |
| `app/api/config/route.ts` | `/api/config` | Daemon has no `/api/config` endpoint |
| `app/api/server-health/route.ts` | `/api/server-health` | Daemon has no `/api/server-health`; health is at `/health` |
| `app/api/projects/route.ts` | `/api/projects` | Daemon has no `/api/projects` endpoint |
| `app/api/sessions/route.ts` | `/api/sessions` | Daemon has no `/api/sessions` endpoint |
| `app/api/solve/route.ts` | `/api/solve` | Daemon has no `/api/solve` endpoint |

For each of these, one of two resolutions applies:

**Option A — endpoint exists on daemon under a different path:** Fix the proxy to call the correct
daemon path (e.g. `app/api/server-health/route.ts` should proxy to `/health`).

**Option B — endpoint does not exist on daemon at all:** Add the daemon endpoint to `http.rs`
alongside the proxy, OR stub the proxy to return `501 Not Implemented` with a descriptive
message until the daemon endpoint is built, so pages show a clear error instead of a silent 404.

The `app/api/server-health/route.ts` → `/health` fix falls under Option A (daemon `/health`
exists). All others (`memory`, `config`, `projects`, `sessions`, `solve`) fall under Option B
and should be stubbed as `501 Not Implemented` until corresponding daemon endpoints are
implemented (those are separate, larger features out of scope here).

#### Scenario: Server-health proxy returns daemon /health data

Given the daemon is running,
when the `/nexus` page mounts and fetches `/api/server-health`,
then the proxy forwards to daemon `/health` and returns 200 with health data.

#### Scenario: Broken proxies return 501 instead of silent 404

Given `memory`, `config`, `projects`, `sessions`, `solve` proxies have no backing daemon endpoint,
when any page calls those routes,
then the response is HTTP 501 with `{ "error": "Not implemented — daemon endpoint pending" }`
instead of a silent 404 or generic error.

### Req-3: Consistent Proxy Pattern

All proxy routes must follow the established pattern already used by working proxies
(`messages`, `obligations`, `latency`, `cc-sessions`):

- Use `daemonFetch` from `@/lib/daemon` (not manual `new URL(path, DAEMON_URL)`)
- Forward relevant query parameters explicitly
- Forward request body as JSON for POST/PUT/PATCH methods
- Mirror daemon status code in the response (`{ status: res.status }`)
- Return `{ error: "Daemon unreachable" }` with status 502 on network failure

#### Scenario: Proxy forwards query params

Given a GET request to `/api/briefing/history?limit=5`,
when the proxy handles it,
then the daemon is called at `/api/briefing/history?limit=5` with the limit forwarded.

#### Scenario: Proxy mirrors non-200 status codes

Given the daemon returns 404 for a missing diary entry,
when the proxy receives that response,
then the Next.js proxy returns 404 (not 200 with an error body).

## Scope
- **IN**: Add the 8 missing proxy routes listed in Req-1; fix `server-health` path mismatch; stub 5 broken proxies as 501; standardize all new routes to use `daemonFetch`
- **OUT**: Implementing the missing daemon endpoints (`memory`, `config`, `projects`, `sessions`, `solve`); WebSocket `/ws/events` proxy (not a REST route); creating the `/contacts` dashboard page (contacts proxy is added but no page); any changes to daemon `http.rs` beyond the `server-health` fix

## Impact
| Area | Change |
|------|--------|
| `apps/dashboard/app/api/` | +8 new route files, 1 path fix, 5 stub replacements |
| Dashboard pages | `/briefing`, `/diary`, `/cold-starts`, `/usage`, `/approvals` pages unblocked |
| `/nexus` page | `server-health` proxy fixed — health widget works |
| No daemon changes | All fixes are dashboard-side only (except no daemon change needed) |

## Risks
| Risk | Mitigation |
|------|-----------|
| `memory`/`config`/`projects`/`sessions`/`solve` pages may rely on the proxy not erroring | Returning 501 is safer than 404 — pages show a clear "not implemented" error rather than an ambiguous failure |
| `/stats` route at `app/stats/route.ts` (not under `app/api/`) may conflict with future page routing | Prefer `app/api/stats/route.ts` and update the usage page's `fetch("/api/stats")` call to match, keeping all proxies consistently under `/api/` |
