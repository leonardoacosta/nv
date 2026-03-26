# Proposal: Scaffold Tool Service Template

## Change ID
`scaffold-tool-service`

## Summary

Create a reusable Hono+MCP service scaffold in `packages/tools/service-template/` that all 8 tool
services will be built from. Provides dual HTTP+MCP transport, shared middleware, pino logging,
config loading, and a systemd unit template.

## Context

- Phase: 1 -- Foundation | Wave: 1 (no dependencies)
- Feature area: infrastructure
- Roadmap: `docs/plan/nova-v10/wave-plan.json`
- 10 downstream specs depend on this: `add-tool-router`, `add-fleet-deploy`, `add-memory-svc`,
  `add-messages-svc`, `add-channels-svc`, `add-discord-svc`, `add-teams-svc`, `add-graph-svc`,
  `add-schedule-svc`, `add-meta-svc`
- Existing daemon at `packages/daemon/` uses Hono + pino + `@anthropic-ai/claude-agent-sdk` --
  the template follows the same conventions
- Existing tool CLIs at `packages/tools/{discord,teams}-cli/` are standalone npm projects -- the
  new services will be pnpm workspace members under `packages/tools/*`

## Motivation

Nova v10 decomposes the monolithic daemon into 9 independently deployable tool services. Every
service shares the same boilerplate: Hono HTTP server, MCP stdio server, health endpoint, CORS,
error handling, logging, config. Without a shared template, each service reimplements this
boilerplate, creating drift and wasted effort.

The template is a **copy-and-customize** scaffold, not a shared library. Each service copies the
template directory, renames it, and adds its domain-specific tools. This avoids shared-library
coupling while ensuring a consistent starting point.

## Requirements

### Req-1: Package Manifest

Create `packages/tools/service-template/package.json`:

- `name: "@nova/service-template"`, `version: "0.1.0"`, `type: "module"`, `private: true`
- Scripts: `dev` (tsx --watch src/index.ts), `build` (tsc), `start` (node dist/index.js),
  `typecheck` (tsc --noEmit)
- Runtime dependencies: `hono`, `@hono/node-server`, `@modelcontextprotocol/sdk`, `pino`, `tsx`,
  `typescript`
- Dev dependencies: `@types/node`, `pino-pretty`

### Req-2: TypeScript Config

Create `packages/tools/service-template/tsconfig.json`:

- `strict: true`, `target: ES2022`, `module: NodeNext`, `moduleResolution: NodeNext`
- `outDir: dist`, `rootDir: src`, `declaration: true`, `sourceMap: true`
- `esModuleInterop: true`, `skipLibCheck: true`

Matches the daemon's tsconfig conventions exactly.

### Req-3: Entry Point with Dual Transport

Create `packages/tools/service-template/src/index.ts`:

1. Load service config (name, port from env)
2. Create pino logger
3. Start Hono HTTP server on configured port
4. Detect stdio mode: if `--mcp` flag is passed, start the MCP stdio server instead of HTTP
5. Graceful shutdown on SIGTERM/SIGINT

The entry point decides transport at startup:
- Default: HTTP server (for systemd service, dashboard access)
- `--mcp` flag: MCP stdio server (for Claude Agent SDK `mcp.json` registration)

Both transports call the same tool handler functions -- the tool logic is transport-agnostic.

### Req-4: HTTP Server (Hono)

Create `packages/tools/service-template/src/http.ts`:

- Hono app with middleware stack:
  - `hono/logger` for request logging
  - `hono/cors` with configurable origin (default: `https://nova.leonardoacosta.dev`)
  - `hono/secure-headers`
  - Global error handler returning `{ error, status }` JSON
- Routes:
  - `GET /health` -- returns `{ status: "ok", service: "<name>", uptime_secs, version }`
  - `POST /tools/:name` -- dispatches to registered tool handlers, returns `{ result, error }`

### Req-5: MCP Server (stdio)

Create `packages/tools/service-template/src/mcp.ts`:

- Uses `@modelcontextprotocol/sdk` to create a stdio MCP server
- Registers the same tool definitions used by the HTTP transport
- Each tool has: `name`, `description`, `inputSchema` (JSON Schema), `handler`

### Req-6: Tool Registry

Create `packages/tools/service-template/src/tools.ts`:

- `ToolDefinition` type: `{ name, description, inputSchema, handler: (input) => Promise<string> }`
- `ToolRegistry` class with `register(tool)`, `get(name)`, `list()`, `execute(name, input)`
- Both HTTP and MCP transports consume the same registry instance
- Template includes one example tool: `ping` -- returns `"pong"` (proves the wiring works)

### Req-7: Config Loading

Create `packages/tools/service-template/src/config.ts`:

- Reads from environment variables (injected by Doppler at runtime):
  - `SERVICE_NAME` -- defaults to `"service-template"`
  - `SERVICE_PORT` -- defaults to `4000`
  - `LOG_LEVEL` -- defaults to `"info"`
  - `CORS_ORIGIN` -- defaults to `"https://nova.leonardoacosta.dev"`
  - `DATABASE_URL` -- optional, not all services need DB access
- Exports `ServiceConfig` type and `loadConfig(): ServiceConfig`

### Req-8: Logger

Create `packages/tools/service-template/src/logger.ts`:

- Reuses the daemon's logger pattern: `createLogger(name)` factory wrapping pino
- `pino-pretty` transport in development only (`NODE_ENV !== "production"`)
- Level from `LOG_LEVEL` env var

### Req-9: systemd Unit Template

Create `packages/tools/service-template/deploy/service-template.service`:

```ini
[Unit]
Description=Nova Tool Service: %i
After=network-online.target
Wants=network-online.target
PartOf=nova-tools.target

[Service]
Type=simple
WorkingDirectory=%h/.local/lib/nova-tools/%i
ExecStart=doppler run --project nova --config prd -- node %h/.local/lib/nova-tools/%i/dist/index.js
Environment=NODE_ENV=production
Environment=PATH=%h/.local/bin:%h/.local/share/pnpm:/usr/local/bin:/usr/bin:/bin
Environment=HOME=%h
Restart=on-failure
RestartSec=5
TimeoutStopSec=30

[Install]
WantedBy=nova-tools.target
```

Key decisions:
- `PartOf=nova-tools.target` -- fleet-wide start/stop via systemd target (set up in `add-fleet-deploy`)
- `%i` instance parameter -- each service copies and replaces `%i` with its name
- `doppler run` injects secrets at runtime -- no `.env` files on disk
- `WorkingDirectory` under `~/.local/lib/nova-tools/` -- mirrors daemon's `nova-ts` layout

### Req-10: Workspace Integration

Update `pnpm-workspace.yaml` to include tool services:

```yaml
packages:
  - "apps/*"
  - "packages/*"
  - "packages/tools/*"
```

This ensures all tool services under `packages/tools/` are pnpm workspace members, enabling
cross-package references (e.g., `@nova/db: workspace:*`).

## Out of Scope

- Actual domain tool implementations (each `add-*-svc` spec handles its own tools)
- Fleet deploy scripts and `nova-tools.target` (that's `add-fleet-deploy`)
- Tool router / central dispatch (that's `add-tool-router`)
- Database access patterns (services that need DB add `@nova/db` as a workspace dependency)
- Authentication / API keys for HTTP endpoints (deferred to hardening)
- Traefik routing configuration (that's `add-fleet-deploy`)
- MCP server registration in `~/.claude/mcp.json` (that's `register-mcp-servers`)

## Impact

| Area | Change |
|------|--------|
| `packages/tools/service-template/` | New directory -- complete service scaffold |
| `pnpm-workspace.yaml` | Add `packages/tools/*` glob |

No changes to existing code. No changes to the daemon, dashboard, or existing tool CLIs.

## Risks

| Risk | Mitigation |
|------|-----------|
| `@modelcontextprotocol/sdk` API may differ from expected | Check latest docs via context7 during implementation; SDK is stable since 1.x |
| Workspace glob `packages/tools/*` picks up existing CLIs | discord-cli and teams-cli already have `package.json` -- they become workspace members (harmless) |
| Template drift as services diverge | Acceptable -- template is copy-and-customize, not a shared lib. Each service owns its copy. |
| pino-pretty dev transport in stdio MCP mode | MCP stdio uses stdin/stdout -- logger must write to stderr only. pino defaults to stdout; must configure `destination: 2` in MCP mode |
