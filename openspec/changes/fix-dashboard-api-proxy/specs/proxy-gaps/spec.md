# Capability: Dashboard API Proxy Gaps

## Why
Several dashboard pages return 404 or 502 errors because Next.js proxy routes are missing or
target non-existent daemon endpoints. This capability adds missing proxies and fixes broken ones
so all dashboard pages can communicate with the daemon.

## ADDED Requirements

### Requirement: Briefing proxy
`GET /api/briefing` SHALL proxy to daemon `GET /api/briefing` using `daemonFetch`, mirroring the daemon's status code. On network failure it MUST return 502 with `{ "error": "Daemon unreachable" }`.

#### Scenario: Briefing proxy returns daemon response
Given the daemon has a morning briefing stored, when `GET /api/briefing` is called on the dashboard, then the proxy responds with the daemon's status code and JSON body.

#### Scenario: Briefing proxy handles unreachable daemon
Given the daemon is unreachable, when `GET /api/briefing` is called, then the proxy responds with 502 and `{ "error": "Daemon unreachable" }`.

### Requirement: Briefing history proxy
`GET /api/briefing/history` SHALL proxy to daemon `GET /api/briefing/history`, forwarding the `limit` query parameter. On network failure it MUST return 502.

#### Scenario: Briefing history forwards limit param
Given a request to `/api/briefing/history?limit=10`, when the proxy handles it, then the daemon is called at `/api/briefing/history?limit=10` and the response is mirrored.

### Requirement: Cold-starts proxy
`GET /api/cold-starts` SHALL proxy to daemon `GET /api/cold-starts`, forwarding the `limit` query parameter. On network failure it MUST return 502.

#### Scenario: Cold-starts proxy forwards limit param
Given a request to `/api/cold-starts?limit=200`, when the proxy handles it, then the daemon is called at `/api/cold-starts?limit=200`.

### Requirement: Stats proxy
`GET /api/stats` SHALL proxy to daemon `GET /stats` (note: daemon path has no `/api/` prefix). The `/usage` page MUST be updated to call `fetch("/api/stats")` instead of `fetch("/stats")` to keep all proxies consistently under `/api/`.

#### Scenario: Stats proxy routes to daemon /stats
Given the daemon is running, when `GET /api/stats` is called on the dashboard, then the proxy calls the daemon at `/stats` (not `/api/stats`) and returns the response.

#### Scenario: Usage page calls /api/stats
Given the refactored usage page, when the page fetches stats, then it calls `fetch("/api/stats")` consistent with all other proxy routes.

### Requirement: Approvals approve proxy
`POST /api/approvals/[id]/approve` SHALL proxy to daemon `POST /api/approvals/{id}/approve`, forwarding the request body as JSON and mirroring the daemon's status code.

#### Scenario: Approve obligation via proxy
Given an obligation with id `abc-123`, when `POST /api/approvals/abc-123/approve` is called, then the proxy forwards to daemon `/api/approvals/abc-123/approve` and mirrors the daemon status code.

### Requirement: Contacts collection proxy
`GET /api/contacts` SHALL proxy to daemon `GET /api/contacts`, forwarding `relationship` and `q` query parameters. `POST /api/contacts` SHALL proxy to daemon `POST /api/contacts` forwarding the JSON body.

#### Scenario: List contacts with query filter
Given a request to `/api/contacts?relationship=work`, when the proxy handles it, then the daemon is called at `/api/contacts?relationship=work`.

#### Scenario: Create contact
Given a `POST /api/contacts` with body `{ "name": "Alice" }`, when the proxy handles it, then the daemon is called with the same JSON body and the daemon's status code (201) is mirrored.

### Requirement: Contacts item proxy
`GET /api/contacts/[id]` SHALL proxy to daemon `GET /api/contacts/{id}`. `PUT /api/contacts/[id]` SHALL proxy to daemon `PUT /api/contacts/{id}` forwarding JSON body. `DELETE /api/contacts/[id]` SHALL proxy to daemon `DELETE /api/contacts/{id}`.

#### Scenario: Update contact
Given a `PUT /api/contacts/abc` with body `{ "notes": "updated" }`, when the proxy handles it, then the daemon is called at `/api/contacts/abc` with the same body and the status code is mirrored.

#### Scenario: Delete contact
Given a `DELETE /api/contacts/abc`, when the proxy handles it, then the daemon DELETE endpoint is called and the response is returned.

### Requirement: Diary proxy
`GET /api/diary` SHALL proxy to daemon `GET /api/diary`, forwarding `date` and `limit` query parameters. On network failure it MUST return 502.

#### Scenario: Diary proxy forwards date and limit
Given a request to `/api/diary?date=2026-03-25&limit=100`, when the proxy handles it, then the daemon is called at `/api/diary?date=2026-03-25&limit=100`.

## MODIFIED Requirements

### Requirement: Server-health proxy path fix
`apps/dashboard/app/api/server-health/route.ts` MUST be changed to proxy to daemon `/health` instead of `/api/server-health`. The daemon's build_router has no `/api/server-health` route; the health endpoint is registered at `/health`.

#### Scenario: Server-health proxy calls correct daemon path
Given the daemon is running, when `GET /api/server-health` is called on the dashboard, then the proxy calls the daemon at `/health` (not `/api/server-health`) and returns 200 with health data.

### Requirement: Stub broken proxies as 501
The proxy routes for `memory`, `config`, `projects`, `sessions`, and `solve` MUST each return HTTP 501 with `{ "error": "Not implemented — daemon endpoint pending" }` for all methods. These routes currently forward to daemon paths that do not exist in build_router, causing opaque 404 responses.

#### Scenario: Memory proxy returns 501
Given a `GET /api/memory` or `PUT /api/memory` request, when the proxy handles it, then the response is HTTP 501 with `{ "error": "Not implemented — daemon endpoint pending" }`.

#### Scenario: Config proxy returns 501
Given a `GET /api/config` or `PUT /api/config` request, when the proxy handles it, then the response is HTTP 501 with `{ "error": "Not implemented — daemon endpoint pending" }`.

#### Scenario: Projects proxy returns 501
Given a `GET /api/projects` request, when the proxy handles it, then the response is HTTP 501 with `{ "error": "Not implemented — daemon endpoint pending" }`.

#### Scenario: Sessions proxy returns 501
Given a `GET /api/sessions` request, when the proxy handles it, then the response is HTTP 501 with `{ "error": "Not implemented — daemon endpoint pending" }`.

#### Scenario: Solve proxy returns 501
Given a `POST /api/solve` request, when the proxy handles it, then the response is HTTP 501 with `{ "error": "Not implemented — daemon endpoint pending" }`.
