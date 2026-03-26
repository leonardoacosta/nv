# Implementation Tasks

<!-- beads:epic:pending -->

## DB Batch

(no database changes)

## API Batch

- [x] [2.1] Add `toolRouterUrl` to `Config` interface in `packages/daemon/src/config.ts` — source from `TOOL_ROUTER_URL` env var or `[daemon] tool_router_url` in TOML, default `http://localhost:4000` [owner:api-engineer]
- [x] [2.2] Replace `ALLOWED_TOOLS` in `packages/daemon/src/brain/agent.ts` with a tool-router proxy — add an `mcpServers` config to the Agent SDK `query()` options pointing at the tool-router's MCP endpoint, or implement a custom tool handler that POSTs to `${config.toolRouterUrl}/call` with `{ tool, input }` and returns the result. Remove the static `ALLOWED_TOOLS` array. Accept `toolRouterUrl` via constructor config. [owner:api-engineer]
- [x] [2.3] Delete `packages/daemon/src/api/server.ts` entirely — contains Hono app, WebSocket server, ActivityRingBuffer, executeToolByName, and all HTTP route handlers [owner:api-engineer]
- [x] [2.4] Delete `packages/daemon/src/http/routes/diary.ts` and `packages/daemon/src/http/routes/briefing.ts` — these endpoints move to fleet services [owner:api-engineer]
- [x] [2.5] Update `packages/daemon/src/index.ts` — remove `import { startApiServer }` and the `startApiServer(apiPort)` call + `API_PORT` env read + associated log line. Remove the second-to-last section (API server block, lines 253-257). [owner:api-engineer]
- [x] [2.6] Remove `hono`, `@hono/node-server`, `ws` from `dependencies` and `@types/ws` from `devDependencies` in `packages/daemon/package.json`. Run `pnpm install` to update lockfile. [owner:api-engineer]

## UI Batch

(no UI changes)

## E2E Batch

- [x] [4.1] Verify daemon starts cleanly after refactor — `pnpm typecheck` passes, no dangling imports to deleted files, no references to `startApiServer` or `api/server` remain in daemon source [owner:e2e-engineer]
