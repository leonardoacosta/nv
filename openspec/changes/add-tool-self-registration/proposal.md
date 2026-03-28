# Proposal: Add Tool Self-Registration

## Change ID
`add-tool-self-registration`

## Summary
Replace the static tool-to-service mapping in tool-router with dynamic self-registration at startup. Currently, `packages/tools/tool-router/src/registry.ts` hardcodes which tools belong to which service and their URLs via a `SERVICES` constant and derived `TOOL_MAP`. Adding a new tool requires modifying the router -- a coupling that should not exist. Services should declare their own capabilities; the router should discover them.

## Context
- Rewrite target: `packages/tools/tool-router/src/registry.ts` -- static `SERVICES` map (9 entries) and `TOOL_MAP` (52 tools)
- Related: `packages/tools/tool-router/src/index.ts` -- server startup, currently no registration phase
- Related: `config/nv.toml` -- already lists all 9 services under `[tools.mcp_servers]` with paths; will gain a `[tool_router]` section for HTTP URLs
- Each service is a Hono HTTP server with an existing `/health` endpoint
- Each service already defines its MCP tool list internally (for `--mcp` stdio mode) but does not expose it over HTTP
- Adding a new tool to any service currently requires two steps: (1) implement the tool in the service, (2) update `registry.ts` in the router

## Motivation
The tool fleet is growing. Each new tool or service requires a router update, creating unnecessary coupling between service implementations and the routing layer. Services should own their tool declarations -- the router should discover them, not define them. This also enables health-aware routing: services that fail registration are known-unavailable at dispatch time, rather than failing silently when a request arrives.

## Requirements

### Req-1: Service registration endpoint
Each tool service exposes `GET /registry` on its existing HTTP port. Returns:

```json
{
  "service": "memory-svc",
  "tools": [
    {
      "name": "read_memory",
      "description": "Read a memory entry by key",
      "inputSchema": { "type": "object", "properties": { "key": { "type": "string" } }, "required": ["key"] }
    }
  ],
  "healthUrl": "http://127.0.0.1:4101/health"
}
```

Fields:
- `service` (string): stable service identifier matching the `name` in `nv.toml`
- `tools` (array of `ToolDefinition`): each entry has `name`, `description`, and `inputSchema` (JSON Schema object)
- `healthUrl` (string): absolute URL to the service health endpoint

This data already exists in each service's MCP tool definitions (used for stdio mode). The endpoint standardizes its exposure over HTTP across all 9 services.

#### Scenario: service responds with full registry
`GET http://127.0.0.1:4106/registry` returns 200 with `service: "schedule-svc"` and 9 tool definitions.

#### Scenario: service is starting up
If the service is not yet listening, the connection is refused. The router handles this via retry (Req-2).

### Req-2: Dynamic registry in tool-router
Replace the static `SERVICES` constant and `TOOL_MAP` IIFE in `registry.ts` with a dynamic registry:

- On startup: query `GET /registry` on all service URLs listed in `nv.toml` `[tool_router]` config
- Build `TOOL_MAP` from aggregated responses
- Service URLs are configured in TOML, not hardcoded -- services cannot register from unknown locations
- Retry failed registrations 3 times with 5-second intervals between attempts
- Log at startup: `"Registered N tools from M services"` at INFO level
- Exported functions `getServiceForTool()`, `getAllServices()`, and `getFullRegistry()` continue to work with the same signatures -- consumers see no API change

#### Scenario: all services healthy
All 9 services respond to `/registry`. Router logs `"Registered 52 tools from 9 services"` and starts accepting dispatch requests.

#### Scenario: partial availability
2 of 9 services are down at startup. Router retries each 3 times, then skips them with WARN. Router starts with tools from the 7 available services. Missing tools return the existing "tool not found" error on dispatch.

### Req-3: Periodic refresh
- Every 60 seconds (configurable via `refresh_interval_s` in `nv.toml`): re-query `GET /registry` on all configured services
- Detect new tools added to a service, tools removed from a service, and services that restarted
- Log changes at INFO level: `"Service X: added tool Y"` / `"Service X: removed tool Z"` / `"Service X: now available (was stale)"`
- Update `TOOL_MAP` atomically -- swap the entire map reference, not incremental mutations -- to avoid race conditions with concurrent dispatch

#### Scenario: tool added to running service
`schedule-svc` deploys with a new `get_schedule` tool. Within 60 seconds, the router picks it up and logs `"Service schedule-svc: added tool get_schedule"`.

#### Scenario: service restarts
`meta-svc` restarts. During the outage window, existing tools remain in the map (marked stale per Req-4). After restart, the next refresh cycle detects it is healthy again and refreshes its tool list.

### Req-4: Fallback for unresponsive services
- If a service fails `/registry` at startup (after all retries): log WARN, skip that service entirely -- do not include its tools in the map
- If a service fails `/registry` during a refresh cycle: keep the last-known tools in the map, mark the service as `"stale"`
- Stale services retain their tools for dispatch but the staleness is visible in the router's own `/registry` response (existing endpoint) and in logs
- If a stale service recovers on a subsequent refresh, clear the stale flag and log recovery

#### Scenario: transient network failure during refresh
`graph-svc` returns a connection error during one refresh cycle. Its 21 tools remain in the map. The router logs `WARN: Service graph-svc failed refresh, marked stale`. Next cycle, `graph-svc` responds successfully -- staleness clears.

#### Scenario: service permanently removed
A service is removed from `nv.toml` and never responds. On the next restart, the router does not query it. Its tools are no longer in the map.

### Req-5: Config change
In `config/nv.toml`, add a `[tool_router]` section with service URLs. This replaces the hardcoded URLs in `registry.ts`:

```toml
[tool_router]
refresh_interval_s = 60

[[tool_router.services]]
name = "memory-svc"
url = "http://127.0.0.1:4101"

[[tool_router.services]]
name = "messages-svc"
url = "http://127.0.0.1:4102"

[[tool_router.services]]
name = "channels-svc"
url = "http://127.0.0.1:4103"

[[tool_router.services]]
name = "discord-svc"
url = "http://127.0.0.1:4104"

[[tool_router.services]]
name = "teams-svc"
url = "http://127.0.0.1:4105"

[[tool_router.services]]
name = "schedule-svc"
url = "http://127.0.0.1:4106"

[[tool_router.services]]
name = "graph-svc"
url = "http://127.0.0.1:4107"

[[tool_router.services]]
name = "meta-svc"
url = "http://127.0.0.1:4108"

[[tool_router.services]]
name = "azure-svc"
url = "http://127.0.0.1:4109"
```

The existing `[tools.mcp_servers]` section remains unchanged -- it configures stdio MCP mode for the Agent SDK. The new `[tool_router]` section configures HTTP mode for the router.

#### Scenario: adding a new service
Operator adds a new `[[tool_router.services]]` entry. On next router restart, the new service is queried and its tools appear in the map. No code changes required.

## Scope
- **IN**: `packages/tools/tool-router/src/registry.ts` (rewrite from static to dynamic), `packages/tools/tool-router/src/index.ts` (startup registration + refresh timer), each of the 9 tool service `index.ts` files (add `GET /registry` endpoint), `config/nv.toml` (add `[tool_router]` section)
- **OUT**: Tool implementations (unchanged), dispatch logic in `routes/dispatch.ts` (unchanged once registry is populated), dashboard, MCP stdio mode, the `[tools.mcp_servers]` config section

## Impact
| Area | Change |
|------|--------|
| `packages/tools/tool-router/src/registry.ts` | Rewrite -- replace static `SERVICES`/`TOOL_MAP` with dynamic registry that queries services at startup and refreshes periodically |
| `packages/tools/tool-router/src/index.ts` | Extended -- add startup registration sequence before `serve()`, add `setInterval` for periodic refresh |
| `packages/tools/memory-svc/src/index.ts` | Extended -- add `GET /registry` route |
| `packages/tools/messages-svc/src/index.ts` | Extended -- add `GET /registry` route |
| `packages/tools/channels-svc/src/index.ts` | Extended -- add `GET /registry` route |
| `packages/tools/discord-svc/src/index.ts` | Extended -- add `GET /registry` route |
| `packages/tools/teams-svc/src/index.ts` | Extended -- add `GET /registry` route |
| `packages/tools/schedule-svc/src/index.ts` | Extended -- add `GET /registry` route |
| `packages/tools/graph-svc/src/index.ts` | Extended -- add `GET /registry` route |
| `packages/tools/meta-svc/src/index.ts` | Extended -- add `GET /registry` route |
| `packages/tools/azure-svc/src/index.ts` | Extended -- add `GET /registry` route |
| `config/nv.toml` | Modified -- add `[tool_router]` section with service URLs and refresh interval |

## Risks
| Risk | Mitigation |
|------|-----------|
| Service down at startup = missing tools | Retry 3 times with 5s intervals, then skip with WARN (not fatal); periodic refresh recovers when service comes up |
| Registry endpoint returns stale data | Periodic refresh (default 60s) catches changes; atomic map swap prevents partial updates |
| Race condition during map swap | Atomic swap -- replace entire map object reference; concurrent dispatch reads see either the old or new map, never a partial state |
| Ordering: tool-router starts before services | Retry loop at startup handles late-starting services; periodic refresh fills gaps within 60s |
| Config/code divergence between `[tools.mcp_servers]` and `[tool_router.services]` | Both sections exist for different transports (stdio vs HTTP); document the distinction in config comments |
| Service returns malformed `/registry` response | Validate response shape before merging; skip service with WARN on validation failure |
