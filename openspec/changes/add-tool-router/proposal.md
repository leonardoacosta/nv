# Proposal: Add Tool Router Service

## Change ID
`add-tool-router`

## Summary

Build the central tool router at `packages/tools/tool-router/` on port 4000. The router dispatches
tool calls by name to the correct fleet service and aggregates health status across all registered
services.

## Context
- Phase: 1 (Foundation) | Wave: 2
- Stack: Hono, pino, TypeScript (pnpm workspace)
- Depends on: `scaffold-tool-service` (Wave 1 — provides the Hono+pino service template)
- Extends: `pnpm-workspace.yaml` (add `packages/tools/*` if not already present)
- Related: All 8 tool services (memory :4001, messages :4002, channels :4003, discord :4004,
  teams :4005, schedule :4006, graph :4007, meta :4008)
- Traefik route: `tools.nova.leonardoacosta.dev/router/*`

## Motivation

The tool fleet needs a single dispatch endpoint so callers (daemon, dashboard, agents) don't need
to know which port hosts which tool. The router maps tool names to service URLs, forwards the call,
and returns the result. It also provides a single `/health` endpoint that aggregates the status of
all 8 downstream services, giving operators a one-call fleet health view.

Without the router, every consumer must maintain a hardcoded map of tool-name-to-port, and health
checks require 8+ individual calls. The router centralizes both concerns.

## Requirements

### Req-1: Service Registry

The router MUST maintain a static registry mapping tool names to service base URLs. The registry is
a TypeScript constant (not a database or config file) that maps each tool name string to a
`{ serviceUrl: string; serviceName: string }` entry.

Initial registry covers all 8 services and their tools:

| Service | Port | Tools |
|---------|------|-------|
| memory-svc | :4001 | read_memory, write_memory, search_memory |
| messages-svc | :4002 | get_recent_messages, search_messages |
| channels-svc | :4003 | list_channels, send_to_channel |
| discord-svc | :4004 | discord_list_guilds, discord_list_channels, discord_read_messages |
| teams-svc | :4005 | teams_list_chats, teams_read_chat, teams_messages, teams_channels, teams_presence, teams_send |
| schedule-svc | :4006 | set_reminder, cancel_reminder, list_reminders, add_schedule, modify_schedule, remove_schedule, list_schedules, start_session, stop_session |
| graph-svc | :4007 | calendar_today, calendar_upcoming, calendar_next, ado_projects, ado_pipelines, ado_builds |
| meta-svc | :4008 | check_services, self_assessment_run, update_soul |

### Req-2: POST /dispatch Endpoint

The router MUST expose `POST /dispatch` that accepts:

```json
{ "tool": "read_memory", "input": { "key": "personality" } }
```

Behavior:
1. Look up `tool` in the registry
2. If not found, return 404 with `{ "error": "unknown_tool", "tool": "<name>" }`
3. Forward as `POST {serviceUrl}/tools/{tool}` with the `input` as the request body
4. Return the downstream response body and status code to the caller
5. If the downstream service is unreachable, return 502 with
   `{ "error": "service_unavailable", "service": "<name>", "tool": "<tool>" }`

### Req-3: GET /health Endpoint

The router MUST expose `GET /health` that:

1. Calls `GET {serviceUrl}/health` on each of the 8 registered services (in parallel)
2. Returns aggregated status:

```json
{
  "status": "healthy" | "degraded" | "unhealthy",
  "services": {
    "memory-svc": { "status": "healthy", "url": "http://localhost:4001", "latency_ms": 12 },
    "messages-svc": { "status": "unreachable", "url": "http://localhost:4002", "latency_ms": null }
  },
  "healthy_count": 7,
  "total_count": 8
}
```

- `"healthy"` if all services respond 200
- `"degraded"` if at least one is unreachable or non-200
- `"unhealthy"` if no services respond

Health check has a 3-second per-service timeout.

### Req-4: GET /registry Endpoint

The router MUST expose `GET /registry` that returns the full tool-to-service mapping, so callers
can discover which tools exist and where they live.

### Req-5: Hono Server on Port 4000

Standard Hono setup with pino logging, CORS enabled, listening on `:4000`. Follow the same pattern
as the daemon's API server (Hono + @hono/node-server + pino).

### Req-6: Workspace Integration

The `packages/tools/tool-router/` package MUST be a valid pnpm workspace member. If
`packages/tools/*` is not in `pnpm-workspace.yaml`, update it.

## Scope
- **IN**: Service registry, /dispatch endpoint, /health aggregation, /registry endpoint, Hono
  server, pino logging, package.json, tsconfig.json, workspace integration
- **OUT**: MCP transport (router is HTTP-only per PRD), authentication/rate limiting, systemd unit
  file (handled by `add-fleet-deploy`), Traefik config (handled by `add-fleet-deploy`), downstream
  service implementation

## Impact
| Area | Change |
|------|--------|
| `packages/tools/tool-router/` | New package (src/index.ts, src/registry.ts, src/routes/) |
| `pnpm-workspace.yaml` | Add `packages/tools/*` if missing |

## Risks
| Risk | Mitigation |
|------|-----------|
| Downstream services don't exist yet during development | Router starts and serves /health with all services as "unreachable" — gracefully degraded |
| Registry becomes stale as services add/remove tools | Registry is a single file (`registry.ts`) — easy to update; /health will flag missing services |
| Dispatch latency adds overhead to tool calls | Single HTTP hop; fetch forwarding adds <5ms in practice on localhost |
