# Implementation Tasks

## DB Batch
(No DB tasks)

## API Batch
- [x] [2.1] [P-1] Add `GET /registry` endpoint to all 9 tool services (memory-svc, messages-svc, channels-svc, discord-svc, teams-svc, schedule-svc, graph-svc, meta-svc, azure-svc) — expose existing MCP tool definitions over HTTP with `service`, `tools[]`, and `healthUrl` fields [owner:api-engineer]
- [x] [2.2] [P-1] Add `[tool_router]` section to `config/nv.toml` with all 9 service URLs and `refresh_interval_s = 60` [owner:api-engineer]
- [x] [2.3] [P-2] Rewrite `packages/tools/tool-router/src/registry.ts` — replace static `SERVICES`/`TOOL_MAP` with dynamic registry that reads service URLs from `nv.toml`, queries `GET /registry` on each at startup with 3x retry (5s interval), builds `TOOL_MAP` from aggregated responses, and preserves existing `getServiceForTool()`/`getAllServices()`/`getFullRegistry()` signatures [owner:api-engineer]
- [x] [2.4] [P-2] Update `packages/tools/tool-router/src/index.ts` — call registry initialization before `serve()`, validate response shape from each service, log `"Registered N tools from M services"` at INFO, handle partial availability (skip unavailable services with WARN) [owner:api-engineer]
- [x] [2.5] [P-3] Add periodic refresh to tool-router — `setInterval` using `refresh_interval_s` from config, re-query all services, detect added/removed tools, atomic map swap, log changes at INFO, mark unresponsive services as stale (retain last-known tools), clear stale flag on recovery [owner:api-engineer]

## UI Batch
(No UI tasks)

## E2E Batch
- [x] [4.1] Test: verify `GET /registry` returns valid response from a tool service (correct shape: `service`, `tools[]` with `name`/`description`/`inputSchema`, `healthUrl`) [owner:e2e-engineer]
- [x] [4.2] Test: verify tool-router startup registers tools from all available services and `getServiceForTool()` resolves correctly [owner:e2e-engineer]
- [x] [4.3] Test: verify periodic refresh detects a newly added tool on a running service and updates the registry [owner:e2e-engineer]
- [x] [4.4] Test: verify stale service handling — service goes down during refresh, tools retained but marked stale; service recovers, stale flag clears [owner:e2e-engineer]
