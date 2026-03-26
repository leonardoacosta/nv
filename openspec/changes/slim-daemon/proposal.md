# Proposal: Slim Daemon

## Change ID
`slim-daemon`

## Summary
Refactor the daemon to remove the embedded Hono API server and inline tool logic, keeping only the Telegram polling, Agent SDK dispatch, watcher callbacks, briefing scheduler, and obligation handling. Tool calls route through the tool-router at :4000 instead of being handled locally.

## Context
- Depends on: `add-tool-router` (must be deployed at :4000 first)
- Conflicts with: `wire-conversation-history` (both touch `packages/daemon/src/`)
- Roadmap: Wave 6, Phase 5 (Daemon Refactor)
- Current daemon: `packages/daemon/src/index.ts` (265 lines), `packages/daemon/src/api/server.ts` (392 lines), `packages/daemon/src/brain/agent.ts` (145 lines)

## Motivation
The daemon currently serves two unrelated roles: (1) Telegram message pump + agent dispatch, and (2) HTTP API server for the dashboard. This coupling means a restart for agent logic also kills the dashboard API, and the growing API surface (obligations, messages, diary, briefing, memory, config, tool-call, WebSocket events) inflates the daemon's dependency footprint. With the tool fleet architecture (v10), dashboard endpoints belong in the fleet services. The daemon should be a lean message pump: receive messages, dispatch to the agent, relay responses.

## Requirements

### Req-1: Remove Hono API server
Delete `packages/daemon/src/api/server.ts` and all HTTP route handlers under `packages/daemon/src/http/`. Remove the `startApiServer()` call from `index.ts`. Remove the `@hono/node-server`, `hono`, and `ws` dependencies from `package.json`.

### Req-2: Remove executeToolByName placeholder
The `executeToolByName` function in `api/server.ts` and the `POST /api/tool-call` endpoint are dead code (the function always throws). Remove them entirely.

### Req-3: Remove ActivityRingBuffer and WebSocket event infrastructure
The `ObligationActivityEvent`, `ActivityRingBuffer`, `emitObligationEvent`, and WebSocket client registry in `api/server.ts` are dashboard-facing infrastructure. Remove them. If obligation executor imports `emitObligationEvent`, stub it to a no-op log or remove the call.

### Req-4: Route agent tool calls through tool-router
Update `NovaAgent` to configure the Agent SDK to route tool calls to the fleet. Add a `TOOL_ROUTER_URL` config value (default `http://localhost:4000`). Replace the static `ALLOWED_TOOLS` list with MCP server configuration pointing at the tool-router, or configure a custom tool handler that proxies calls to `POST http://localhost:4000/call`.

### Req-5: Keep core daemon responsibilities intact
The following must remain untouched and functional after the refactor:
- Telegram polling via `TelegramAdapter`
- Agent dispatch via `NovaAgent.processMessage()`
- Proactive watcher start/stop and callback routing
- Morning briefing scheduler
- Obligation store + confirm/reopen callback routing
- Graceful shutdown (SIGTERM/SIGINT)
- Database pool for features that need it (watcher, briefing, obligations)

### Req-6: Remove second database pool
`api/server.ts` creates its own `Pool` in `startApiServer()`, duplicating the pool created in `index.ts`. After removing the API server, only the single pool in `index.ts` remains.

### Req-7: Update config for tool-router URL
Add `toolRouterUrl` to the `Config` interface, sourced from `TOOL_ROUTER_URL` env var or `[daemon] tool_router_url` in `nv.toml`. Default: `http://localhost:4000`.

## Scope
- **IN**: Remove API server, remove tool-call endpoint, remove WebSocket/ring-buffer, remove Hono deps, add tool-router proxy in agent, update config
- **OUT**: Dashboard API migration (separate specs per fleet service), conversation history (separate spec `wire-conversation-history`), MCP server registration (separate spec `register-mcp-servers`), briefing/watcher/obligation logic changes

## Impact
| Area | Change |
|------|--------|
| `packages/daemon/src/api/server.ts` | DELETE entire file |
| `packages/daemon/src/http/routes/diary.ts` | DELETE (moves to fleet) |
| `packages/daemon/src/http/routes/briefing.ts` | DELETE (moves to fleet) |
| `packages/daemon/src/index.ts` | Remove `startApiServer` import/call, remove `API_PORT` |
| `packages/daemon/src/brain/agent.ts` | Replace `ALLOWED_TOOLS` with tool-router proxy |
| `packages/daemon/src/config.ts` | Add `toolRouterUrl` field |
| `packages/daemon/package.json` | Remove `hono`, `@hono/node-server`, `ws` deps + `@types/ws` |
| `packages/daemon/src/features/obligations/executor.ts` | Remove `emitObligationEvent` import if present |

## Risks
| Risk | Mitigation |
|------|-----------|
| Dashboard breaks (calls daemon API) | Dashboard currently proxies through Next.js API routes to `DAEMON_URL`. After this spec, those routes return 502 until fleet services are deployed. This is expected — fleet services (`add-memory-svc`, `add-messages-svc`, etc.) provide the replacement endpoints. Coordinate deployment order. |
| Tool-router not running when daemon starts | Agent SDK calls will fail with connection refused. Add a health-check log on startup that warns if tool-router is unreachable. Non-blocking — the daemon still starts. |
| emitObligationEvent consumers break | Grep for all imports of `emitObligationEvent`. If obligation executor uses it, replace with a log statement. Dashboard WebSocket consumers will get nothing until the fleet provides an equivalent event stream. |
| Agent SDK tool configuration change | Test that `NovaAgent.processMessage()` still completes a round-trip with the tool-router proxy. The proxy is a simple `fetch()` to `:4000/call`. |
