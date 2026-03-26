# Implementation Tasks

<!-- beads:epic:nv-TODO -->

## Phase 1: Project Setup

- [x] [1.1] Update `pnpm-workspace.yaml` -- add `"packages/tools/*"` to the packages array so tool services are pnpm workspace members [owner:api-engineer]
- [x] [1.2] Create `packages/tools/service-template/package.json` -- `name: "@nova/service-template"`, `version: "0.1.0"`, `type: "module"`, `private: true`; scripts: `dev` (tsx --watch src/index.ts), `build` (tsc), `start` (node dist/index.js), `typecheck` (tsc --noEmit); runtime deps: `hono`, `@hono/node-server`, `@modelcontextprotocol/sdk`, `pino`, `tsx`, `typescript`; dev deps: `@types/node`, `pino-pretty` [owner:api-engineer]
- [x] [1.3] Create `packages/tools/service-template/tsconfig.json` -- `strict: true`, `target: ES2022`, `module: NodeNext`, `moduleResolution: NodeNext`, `outDir: dist`, `rootDir: src`, `declaration: true`, `sourceMap: true`, `esModuleInterop: true`, `skipLibCheck: true` [owner:api-engineer]
- [x] [1.4] Create `packages/tools/service-template/.gitignore` -- ignore `dist/` and `node_modules/` [owner:api-engineer]

## Phase 2: Core Modules

- [x] [2.1] Create `packages/tools/service-template/src/config.ts` -- `ServiceConfig` type with `serviceName`, `servicePort`, `logLevel`, `corsOrigin`, `databaseUrl?`; `loadConfig(): ServiceConfig` reads from env vars `SERVICE_NAME` (default `"service-template"`), `SERVICE_PORT` (default `4000`), `LOG_LEVEL` (default `"info"`), `CORS_ORIGIN` (default `"https://nova.leonardoacosta.dev"`), `DATABASE_URL` (optional) [owner:api-engineer]
- [x] [2.2] Create `packages/tools/service-template/src/logger.ts` -- `createLogger(name, options?)` factory wrapping pino; `pino-pretty` transport when `NODE_ENV !== "production"`; level from `LOG_LEVEL` env; accepts optional `destination` parameter for stderr redirection in MCP mode; exports `Logger` type re-export from pino [owner:api-engineer]
- [x] [2.3] Create `packages/tools/service-template/src/tools.ts` -- `ToolDefinition` type: `{ name: string, description: string, inputSchema: Record<string, unknown>, handler: (input: Record<string, unknown>) => Promise<string> }`; `ToolRegistry` class with `register(tool)`, `get(name)`, `list()`, `execute(name, input)`; `execute` throws on unknown tool name; register one example tool `ping` that returns `"pong"` [owner:api-engineer]

## Phase 3: Transport Layers

- [x] [3.1] Create `packages/tools/service-template/src/http.ts` -- Hono app factory `createHttpApp(registry, config)`: middleware stack (hono/logger, hono/cors with `config.corsOrigin`, hono/secure-headers); global error handler returning `{ error, status }` JSON; `GET /health` returns `{ status: "ok", service, uptime_secs, version }`; `POST /tools/:name` reads JSON body, calls `registry.execute(name, input)`, returns `{ result, error }` [owner:api-engineer]
- [x] [3.2] Create `packages/tools/service-template/src/mcp.ts` -- `startMcpServer(registry, config)` function: creates `@modelcontextprotocol/sdk` stdio server; iterates `registry.list()` to register each tool with its `name`, `description`, `inputSchema`; handler delegates to `registry.execute()`; logger writes to stderr (fd 2) to avoid corrupting MCP stdio protocol [owner:api-engineer]

## Phase 4: Entry Point

- [x] [4.1] Create `packages/tools/service-template/src/index.ts` -- loads config via `loadConfig()`; creates logger; creates `ToolRegistry` and registers `ping` tool; checks `process.argv` for `--mcp` flag: if present, calls `startMcpServer()`; otherwise starts Hono HTTP server via `@hono/node-server` `serve()` on configured port; SIGTERM/SIGINT handlers for graceful shutdown; logs startup banner `{ service, port, transport: "http"|"mcp" }` [owner:api-engineer]

## Phase 5: Deploy Template

- [x] [5.1] Create `packages/tools/service-template/deploy/service-template.service` -- systemd unit: `Description=Nova Tool Service: %i`, `After=network-online.target`, `PartOf=nova-tools.target`, `Type=simple`, `ExecStart=doppler run --project nova --config prd -- node %h/.local/lib/nova-tools/%i/dist/index.js`, `Environment=NODE_ENV=production`, `Environment=PATH=%h/.local/bin:%h/.local/share/pnpm:/usr/local/bin:/usr/bin:/bin`, `Environment=HOME=%h`, `Restart=on-failure`, `RestartSec=5`, `TimeoutStopSec=30`, `WantedBy=nova-tools.target` [owner:api-engineer]

---

## Validation Gates

| Phase | Gate |
|-------|------|
| 1 Project setup | `pnpm install` succeeds from repo root with no errors |
| 2 Core modules | `cd packages/tools/service-template && pnpm typecheck` passes |
| 3 Transport layers | `pnpm typecheck` still passes after adding http.ts and mcp.ts |
| 4 Entry point | `pnpm build` produces `dist/index.js`; `node dist/index.js` starts HTTP server and responds to `curl localhost:4000/health` with `{ status: "ok" }` |
| 5 Deploy template | `systemd-analyze verify --user deploy/service-template.service` reports no fatal errors (warnings about %i are expected) |
| **Final** | `pnpm typecheck` passes; `pnpm build` succeeds; HTTP health check responds; `curl -X POST localhost:4000/tools/ping -H 'Content-Type: application/json' -d '{}'` returns `{ result: "pong", error: null }` |
