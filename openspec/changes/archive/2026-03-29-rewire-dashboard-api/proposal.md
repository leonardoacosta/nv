# Proposal: Rewire Dashboard API

## Change ID
`rewire-dashboard-api`

## Summary
Replace all 22 broken `daemonFetch()` API routes in the dashboard with direct Drizzle DB queries (for data the daemon owned) and direct HTTP calls to fleet services (for tool-backed data). Remove `lib/daemon.ts`, update `next.config.ts` rewrites, and update `server.ts` WebSocket proxy. Add `@nova/db` as a dashboard dependency.

## Context
- Depends on: `slim-daemon` (completed -- removed daemon HTTP API), all `add-*-svc` specs (fleet services deployed and healthy)
- Conflicts with: none (dashboard-only changes)
- Roadmap: Wave 8, post-fleet cleanup
- Current state: All 22 API routes in `apps/dashboard/app/api/` call `daemonFetch()` or construct URLs from `DAEMON_URL`. The daemon's Hono API server was deleted in `slim-daemon`. Every route returns 502 "Daemon unreachable", causing "Retry" buttons on every dashboard page.

## Motivation
The `slim-daemon` spec removed the daemon's HTTP API server, breaking all 22 dashboard API routes. The fleet services (`memory-svc :4101`, `messages-svc :4102`, `schedule-svc :4106`, `meta-svc :4108`, `tool-router :4100`) now provide the tool-backed endpoints. Data that the daemon served directly from Postgres (obligations, contacts, diary, briefings, sessions, config) should be queried via Drizzle from `@nova/db` since the dashboard already runs server-side Next.js route handlers with access to `DATABASE_URL`.

The dashboard runs in Docker on the homelab network. Fleet services run bare metal on the same host. The dashboard container reaches the host via `host.docker.internal` (already configured in `docker-compose.yml`). Fleet service URLs must be configurable via environment variables so the Docker container can resolve them.

## Requirements

### Req-1: Add @nova/db dependency
Add `@nova/db` as a workspace dependency in `apps/dashboard/package.json`. Create `apps/dashboard/lib/db.ts` that re-exports the `db` client for use in API routes. The `DATABASE_URL` env var must be passed to the Docker container.

### Req-2: Rewire DB-backed routes to direct Drizzle queries
Replace `daemonFetch()` calls with direct Drizzle queries for routes whose data lives in Postgres tables owned by `@nova/db`:

| Route | Method(s) | Table | Query |
|-------|-----------|-------|-------|
| `/api/obligations` | GET | `obligations` | Select with optional `status`/`owner` filter, order by `created_at` desc |
| `/api/obligations/[id]` | PATCH | `obligations` | Update by ID, return updated row |
| `/api/obligations/[id]/execute` | POST | `obligations` | Update status to `in_progress` |
| `/api/obligations/activity` | GET | `obligations` | Select recently updated obligations (order by `updated_at` desc, limit) |
| `/api/obligations/stats` | GET | `obligations` | Count by status/owner groupings |
| `/api/contacts` | GET, POST | `contacts` | Select with optional `relationship`/`q` filter; insert new |
| `/api/contacts/[id]` | GET, PUT, PATCH, DELETE | `contacts` | CRUD by ID |
| `/api/diary` | GET | `diary` | Select with optional `date`/`limit` filter, order by `created_at` desc |
| `/api/briefing` | GET | `briefings` | Select latest entry (order by `generated_at` desc, limit 1) |
| `/api/briefing/history` | GET | `briefings` | Select with optional `limit`, order by `generated_at` desc |
| `/api/sessions` | GET | `sessions` | Select all, order by `started_at` desc |
| `/api/cc-sessions` | GET | `sessions` | Select where `project` matches CC session pattern |
| `/api/approvals/[id]/approve` | POST | `obligations` | Update obligation status to `proposed_done` |

### Req-3: Rewire fleet-backed routes to direct service HTTP calls
Replace `daemonFetch()` calls with direct HTTP fetch to fleet service endpoints:

| Route | Method | Fleet Service | Fleet Endpoint |
|-------|--------|---------------|----------------|
| `/api/messages` | GET | messages-svc :4102 | `GET /recent?channel=&limit=` |
| `/api/memory` | GET | memory-svc :4101 | `POST /read` (with topic) or `POST /search` |
| `/api/memory` | PUT | memory-svc :4101 | `POST /write` |
| `/api/server-health` | GET | tool-router :4100 | `GET /health` |
| `/api/latency` | GET | meta-svc :4108 | `GET /health` (derive latency from fleet health) |
| `/api/stats` | GET | meta-svc :4108 | `GET /services` + aggregate from fleet |

### Req-4: Handle routes with no fleet equivalent
Some routes proxied daemon functionality that no longer exists or has moved to MCP-only access:

| Route | Decision | Rationale |
|-------|----------|-----------|
| `/api/solve` | Remove | Agent dispatch was daemon-only; MCP tool calls go through tool-router, not dashboard |
| `/api/projects` | Convert to static config | Read from `NV_PROJECTS` env var or a static JSON file |
| `/api/config` | Convert to env/static | Dashboard config is env-var driven; remove daemon config proxy |
| `/api/cold-starts` | Remove | Performance data was daemon-specific; no fleet equivalent yet |

### Req-5: Create fleet fetch helper
Create `apps/dashboard/lib/fleet.ts` exporting a `fleetFetch(service, path, init?)` helper. Service URLs are sourced from environment variables:

| Env Var | Default | Service |
|---------|---------|---------|
| `TOOL_ROUTER_URL` | `http://host.docker.internal:4100` | tool-router |
| `MEMORY_SVC_URL` | `http://host.docker.internal:4101` | memory-svc |
| `MESSAGES_SVC_URL` | `http://host.docker.internal:4102` | messages-svc |
| `META_SVC_URL` | `http://host.docker.internal:4108` | meta-svc |

Defaults use `host.docker.internal` since the dashboard runs in Docker and fleet services run bare metal on the host.

### Req-6: Remove daemon infrastructure
- Delete `apps/dashboard/lib/daemon.ts`
- Remove `DAEMON_URL` env var from `docker-compose.yml`
- Remove the catch-all API rewrite in `next.config.ts` (`/api/:path*` -> daemon)
- Update `server.ts`: remove the WebSocket proxy to the daemon (the daemon no longer serves WebSocket events). Keep the custom server for Next.js but remove `http-proxy` and related code. Remove `http-proxy` from `package.json` dependencies and `@types/http-proxy` from devDependencies.

### Req-7: Update Docker configuration
Add new env vars to `docker-compose.yml`:
- `DATABASE_URL` for Drizzle connection
- `TOOL_ROUTER_URL`, `MEMORY_SVC_URL`, `MESSAGES_SVC_URL`, `META_SVC_URL` for fleet access
- Remove `DAEMON_URL`

### Req-8: Update types/api.ts
Remove references to "daemon" in comments. Update type comments to reflect new data sources (Drizzle vs fleet). Remove types for removed routes (`solve`, `cold-starts`). Add any new response types needed for fleet service responses.

### Req-9: Keep auth routes untouched
The `/api/auth/verify` and `/api/auth/logout` routes do not use `daemonFetch()` and must remain unchanged. The `/api/session/*` routes (control, logs, message, status) use `sessionManager` (Docker CC session management) and must also remain unchanged.

## Scope
- **IN**: Rewire 22 broken API routes, add `@nova/db` dep, create fleet fetch helper, remove daemon.ts, update docker-compose.yml env vars, remove API rewrite from next.config.ts, remove WebSocket proxy from server.ts, remove http-proxy dependency, update types/api.ts
- **OUT**: New dashboard features, UI component changes (pages already handle error states), fleet service API changes, new DB schema, Traefik routing changes (fleet services are already routed), session manager changes

## Impact
| Area | Change |
|------|--------|
| `apps/dashboard/package.json` | Add `@nova/db` dep, remove `http-proxy` + `@types/http-proxy` |
| `apps/dashboard/lib/daemon.ts` | DELETE |
| `apps/dashboard/lib/fleet.ts` | NEW -- fleet service fetch helper |
| `apps/dashboard/lib/db.ts` | NEW -- re-export @nova/db client |
| `apps/dashboard/app/api/obligations/route.ts` | Rewrite: Drizzle query |
| `apps/dashboard/app/api/obligations/[id]/route.ts` | Rewrite: Drizzle query |
| `apps/dashboard/app/api/obligations/[id]/execute/route.ts` | Rewrite: Drizzle query |
| `apps/dashboard/app/api/obligations/activity/route.ts` | Rewrite: Drizzle query |
| `apps/dashboard/app/api/obligations/stats/route.ts` | Rewrite: Drizzle query |
| `apps/dashboard/app/api/contacts/route.ts` | Rewrite: Drizzle query |
| `apps/dashboard/app/api/contacts/[id]/route.ts` | Rewrite: Drizzle query |
| `apps/dashboard/app/api/diary/route.ts` | Rewrite: Drizzle query |
| `apps/dashboard/app/api/briefing/route.ts` | Rewrite: Drizzle query |
| `apps/dashboard/app/api/briefing/history/route.ts` | Rewrite: Drizzle query |
| `apps/dashboard/app/api/sessions/route.ts` | Rewrite: Drizzle query |
| `apps/dashboard/app/api/cc-sessions/route.ts` | Rewrite: Drizzle query |
| `apps/dashboard/app/api/approvals/[id]/approve/route.ts` | Rewrite: Drizzle query |
| `apps/dashboard/app/api/messages/route.ts` | Rewrite: fleet HTTP (messages-svc) |
| `apps/dashboard/app/api/memory/route.ts` | Rewrite: fleet HTTP (memory-svc) |
| `apps/dashboard/app/api/server-health/route.ts` | Rewrite: fleet HTTP (tool-router) |
| `apps/dashboard/app/api/latency/route.ts` | Rewrite: fleet HTTP (meta-svc) |
| `apps/dashboard/app/api/stats/route.ts` | Rewrite: fleet HTTP (meta-svc) |
| `apps/dashboard/app/api/solve/route.ts` | DELETE |
| `apps/dashboard/app/api/cold-starts/route.ts` | DELETE |
| `apps/dashboard/app/api/projects/route.ts` | Rewrite: static config / env var |
| `apps/dashboard/app/api/config/route.ts` | Rewrite: static config / env var |
| `apps/dashboard/next.config.ts` | Remove API rewrite to daemon |
| `apps/dashboard/server.ts` | Remove WebSocket proxy, simplify to plain Next.js server |
| `apps/dashboard/types/api.ts` | Update comments, remove dead types |
| `docker-compose.yml` | Update env vars (remove DAEMON_URL, add DATABASE_URL + fleet URLs) |

## Risks
| Risk | Mitigation |
|------|-----------|
| Drizzle type mismatch with existing API response shapes | The `types/api.ts` types were modeled on the Rust daemon's output with snake_case fields. Drizzle returns camelCase. Route handlers must map Drizzle results to match existing frontend response shapes (snake_case) to avoid breaking UI components. |
| Fleet service unreachable from Docker | Defaults use `host.docker.internal` which is already configured in docker-compose.yml. All fleet services bind to `0.0.0.0` on their ports. Test with `curl` from inside container before deploying. |
| DATABASE_URL not available in Docker | Must be added to docker-compose.yml environment block. Use the same Neon connection string used by fleet services. |
| Response shape drift between daemon API and Drizzle/fleet | Some daemon API responses included computed fields (e.g., `attempt_count`, `notes` on obligations) that are not in the Drizzle schema. These fields may need to be computed in the route handler or removed from the response type. Document any intentional shape changes. |
| Removing WebSocket proxy breaks real-time updates | The daemon WebSocket event stream (`/ws/events`) was already broken since slim-daemon. No functional regression. Future real-time features will use a different mechanism (SSE from fleet or dedicated event service). |
