# Proposal: Add Meta Service

## Change ID
`add-meta-svc`

## Summary

Build the meta service at port 4008 as a Hono+MCP microservice following the scaffold-tool-service
template. Provides fleet health monitoring (pings all 8 sibling services), self-assessment
(reads memory, recent messages, and obligations to generate a reflection), and soul document
management (read/write `config/soul.md`).

## Context
- Phase: 4 (Automation Tools) | Wave: 5
- Stack: TypeScript (Hono + pino + MCP stdio)
- Depends on: `scaffold-tool-service` (service template, shared logger, health endpoint pattern)
- Lives in: `packages/tools/meta-svc/`
- Port: 4008 (registered in nova-v10 PRD)
- systemd unit: `nova-meta-svc.service` (under `nova-tools.target`)
- Related: Rust `check_services` in `crates/nv-daemon/src/tools/check.rs` (reference for health
  probe logic), `config/soul.md` (1.6KB soul document)
- Does NOT depend on Postgres directly — delegates to memory-svc (:4001) and messages-svc (:4002)
  via HTTP for data reads

## Motivation

Nova's tool fleet runs 9 independent services. Without a meta service:

1. **No fleet visibility** — no single endpoint answers "which services are up?" Operators must
   curl 9 health endpoints manually.
2. **No self-awareness** — Nova cannot reflect on her own state (recent conversations, memory
   topics, open obligations) without a dedicated tool.
3. **No soul evolution** — the soul document (`config/soul.md`) defines Nova's personality.
   Currently only editable by hand or Claude Code's Write tool. A tool-accessible endpoint lets
   Nova propose and apply personality refinements programmatically.

## Requirements

### Req-1: Project Scaffold — `packages/tools/meta-svc/`

Create the service package following the scaffold-tool-service template:

```
packages/tools/meta-svc/
  src/
    index.ts          — entry point (start HTTP server)
    server.ts         — Hono app, routes, middleware
    health.ts         — service health check logic (fleet probing)
    self-assess.ts    — self-assessment runner
    soul.ts           — soul document read/write
    mcp.ts            — MCP stdio server (tool definitions + handlers)
    logger.ts         — pino logger (name: "meta-svc")
    types.ts          — shared types
  package.json
  tsconfig.json
```

Dependencies:
```
hono            ^4
@hono/node-server ^1
pino            ^9
```

Dev dependencies:
```
@types/node     ^22
tsx             ^4
typescript      ^5
pino-pretty     ^13
```

### Req-2: Fleet Health Probing — `src/health.ts`

Export `probeFleet(): Promise<ServiceHealthReport[]>` that concurrently hits `GET /health` on all
8 sibling services:

| Service | URL |
|---------|-----|
| tool-router | `http://localhost:4000/health` |
| memory-svc | `http://localhost:4001/health` |
| messages-svc | `http://localhost:4002/health` |
| channels-svc | `http://localhost:4003/health` |
| discord-svc | `http://localhost:4004/health` |
| teams-svc | `http://localhost:4005/health` |
| schedule-svc | `http://localhost:4006/health` |
| graph-svc | `http://localhost:4007/health` |

Per-service probe:
- Timeout: 3 seconds (AbortController)
- On success (HTTP 200 + JSON body): `{ name, status: "healthy", uptime_secs, latency_ms }`
- On HTTP error: `{ name, status: "unhealthy", error: "<status> <statusText>" }`
- On network error / timeout: `{ name, status: "unreachable", error: "<message>" }`

`ServiceHealthReport` type:
```typescript
interface ServiceHealthReport {
  name: string;
  url: string;
  status: "healthy" | "unhealthy" | "unreachable";
  uptime_secs?: number;
  latency_ms: number;
  error?: string;
}
```

All probes run concurrently via `Promise.allSettled`. Probe failures never throw — every service
gets a report entry regardless.

The service registry (name + port mapping) is a const array in `health.ts`, not config-driven.
When a new service is added to the fleet, this array is updated.

### Req-3: Self-Assessment — `src/self-assess.ts`

Export `runSelfAssessment(): Promise<SelfAssessmentResult>` that gathers Nova's operational state
and produces a structured reflection.

Data gathering (all via HTTP to sibling services — no direct DB access):

1. **Memory topics** — `GET http://localhost:4001/api/memory` -> list of topic names
2. **Recent messages** — `GET http://localhost:4002/api/messages?per_page=20` -> last 20 messages
3. **Fleet health** — calls `probeFleet()` from `health.ts`

Assessment logic:
- Count memory topics, identify staleness (topics with old timestamps if available)
- Summarize recent conversation patterns (message count, channels used)
- Fleet health summary (healthy/unhealthy/unreachable counts)
- Generate `observations: string[]` — plain-text observations about current state
- Generate `suggestions: string[]` — actionable suggestions for improvement

`SelfAssessmentResult` type:
```typescript
interface SelfAssessmentResult {
  generated_at: string;          // ISO timestamp
  memory_topic_count: number;
  recent_message_count: number;
  fleet_health: {
    total: number;
    healthy: number;
    unhealthy: number;
    unreachable: number;
  };
  observations: string[];
  suggestions: string[];
}
```

Timeout: 10 seconds total for the entire assessment. If any sub-request fails, include partial
results with a note in observations.

### Req-4: Soul Management — `src/soul.ts`

Export two functions:

`readSoul(): Promise<string>` — reads `config/soul.md` from the project root. The path is
resolved relative to `process.cwd()` (the service runs from the project root via systemd
`WorkingDirectory`). Returns the raw markdown content. Throws if file not found.

`writeSoul(content: string): Promise<void>` — writes the full content to `config/soul.md`.
Creates parent directories if needed. Logs the write via pino (topic: "soul-update").

Path: `config/soul.md` (not `~/.nv/` — the soul lives in the repo, not the home directory).

### Req-5: HTTP Routes — `src/server.ts`

Hono app with these routes:

**GET /health**
Standard health endpoint (consistent with all fleet services):
```json
{ "status": "ok", "uptime_secs": 42, "version": "0.1.0" }
```

**GET /services**
Calls `probeFleet()` and returns the full `ServiceHealthReport[]` array:
```json
{
  "services": [...],
  "summary": { "total": 8, "healthy": 6, "unhealthy": 1, "unreachable": 1 }
}
```

**POST /self-assess**
Calls `runSelfAssessment()` and returns the `SelfAssessmentResult`:
```json
{
  "generated_at": "2026-03-26T...",
  "memory_topic_count": 12,
  "recent_message_count": 20,
  "fleet_health": { "total": 8, "healthy": 7, "unhealthy": 0, "unreachable": 1 },
  "observations": ["...", "..."],
  "suggestions": ["...", "..."]
}
```

**GET /soul**
Returns the raw soul document:
```json
{ "content": "# Nova -- Soul\n\n..." }
```

**POST /soul**
Body: `{ "content": "..." }` — writes the new soul content. Returns:
```json
{ "ok": true, "bytes": 1632 }
```
400 if `content` is missing or empty.

Middleware: `logger()`, `cors({ origin: '*' })` (internal fleet, no auth), `secureHeaders()`.
Global error handler returns `{ error: message }` with status 500.

### Req-6: MCP Server — `src/mcp.ts`

Stdio MCP server exposing 3 tools for Agent SDK native discovery:

**check_services**
- Description: "Ping all tool fleet services and return their health status"
- Input schema: `{}` (no parameters)
- Handler: calls `probeFleet()`, returns JSON string of the services + summary

**self_assessment_run**
- Description: "Run a self-assessment reading memory, recent messages, and fleet health"
- Input schema: `{}` (no parameters)
- Handler: calls `runSelfAssessment()`, returns JSON string of the result

**update_soul**
- Description: "Update Nova's soul document (config/soul.md)"
- Input schema: `{ changes: string }` — the full new content for the soul document
- Handler: calls `writeSoul(changes)`, returns confirmation string

MCP registration: the service is registered in `~/.claude/mcp.json` as `nova-meta` with
`command: "node"` and `args: ["packages/tools/meta-svc/dist/mcp.js"]`.

### Req-7: Entry Point — `src/index.ts`

```typescript
import { startServer } from "./server.js";

const PORT = Number(process.env.META_SVC_PORT ?? 4008);
await startServer(PORT);
```

Log startup message: `meta-svc listening on :${PORT}`.

## Scope

- **IN**: `packages/tools/meta-svc/` (all source files), fleet health probing, self-assessment,
  soul read/write, HTTP routes, MCP stdio server, package.json, tsconfig.json
- **OUT**: Postgres direct access (delegates to memory-svc and messages-svc via HTTP), systemd
  unit file creation (handled by `add-fleet-deploy`), Traefik config (handled by `add-fleet-deploy`),
  MCP registration in `~/.claude/mcp.json` (handled by `register-mcp-servers`), obligation data
  (not yet exposed by a sibling service at the time meta-svc ships), dashboard UI for fleet health

## Impact

| Area | Change |
|------|--------|
| `packages/tools/meta-svc/package.json` | New: package manifest |
| `packages/tools/meta-svc/tsconfig.json` | New: TypeScript config |
| `packages/tools/meta-svc/src/index.ts` | New: entry point |
| `packages/tools/meta-svc/src/server.ts` | New: Hono app + routes |
| `packages/tools/meta-svc/src/health.ts` | New: fleet health probing |
| `packages/tools/meta-svc/src/self-assess.ts` | New: self-assessment runner |
| `packages/tools/meta-svc/src/soul.ts` | New: soul document read/write |
| `packages/tools/meta-svc/src/mcp.ts` | New: MCP stdio server |
| `packages/tools/meta-svc/src/logger.ts` | New: pino logger |
| `packages/tools/meta-svc/src/types.ts` | New: shared types |
| `pnpm-workspace.yaml` | May need `packages/tools/*` glob (verify existing coverage) |

No changes to `packages/daemon/`, `apps/dashboard/`, `config/soul.md`, or any existing service.

## Risks

| Risk | Mitigation |
|------|-----------|
| Sibling services not running when meta-svc probes them | Probes use `Promise.allSettled` + per-service timeout; unreachable services get `status: "unreachable"`, never crash meta-svc |
| memory-svc or messages-svc API shape changes | Self-assessment degrades gracefully — includes partial results with error note in `observations` |
| Soul file path wrong in production | `process.cwd()` set by systemd `WorkingDirectory=/home/nyaptor/dev/nv`; verified in deploy spec |
| MCP stdio conflicts with HTTP server | Separate entry points: `index.ts` for HTTP, `mcp.ts` for MCP stdio; systemd runs HTTP, Claude config runs MCP |
| Fleet probe floods services on frequent calls | 3s timeout per probe, no caching — caller controls frequency; add caching if this becomes a problem |
