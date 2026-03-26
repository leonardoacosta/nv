# Implementation Tasks

<!-- beads:epic:TBD -->

## Dependencies

- `scaffold-tool-service`

## Batch 1 -- Project Scaffold

- [x] [1.1] [P-1] Create `packages/tools/meta-svc/package.json` with name `@nova/meta-svc`, type `module`, scripts (dev, build, start, typecheck), dependencies (hono ^4, @hono/node-server ^1, pino ^9), devDependencies (@types/node ^22, tsx ^4, typescript ^5, pino-pretty ^13) [owner:api-engineer]
- [x] [1.2] [P-1] Create `packages/tools/meta-svc/tsconfig.json` — target ES2022, module NodeNext, moduleResolution NodeNext, outDir dist, rootDir src, strict true, skipLibCheck true [owner:api-engineer]
- [x] [1.3] [P-1] Create `packages/tools/meta-svc/src/logger.ts` — pino logger named `meta-svc`, same pattern as daemon logger (pino-pretty in dev, structured JSON in prod) [owner:api-engineer]
- [x] [1.4] [P-1] Create `packages/tools/meta-svc/src/types.ts` — export `ServiceHealthReport` interface (name, url, status, uptime_secs?, latency_ms, error?), `SelfAssessmentResult` interface (generated_at, memory_topic_count, recent_message_count, fleet_health, observations, suggestions) [owner:api-engineer]
- [x] [1.5] [P-2] Verify `pnpm-workspace.yaml` covers `packages/tools/*` — if not, the existing `packages/*` glob should already match; confirm with `pnpm ls --filter @nova/meta-svc` after install [owner:api-engineer]

## Batch 2 -- Fleet Health Probing

- [x] [2.1] [P-1] Create `packages/tools/meta-svc/src/health.ts` — define `SERVICE_REGISTRY` const array with 8 entries (tool-router :4000 through graph-svc :4007), each with `name` and `port` [owner:api-engineer]
- [x] [2.2] [P-1] Implement `probeService(name, url): Promise<ServiceHealthReport>` — fetch with 3s AbortController timeout, measure latency via `performance.now()`, parse JSON body for `uptime_secs`, catch network/timeout errors and return `status: "unreachable"` [owner:api-engineer]
- [x] [2.3] [P-1] Implement `probeFleet(): Promise<ServiceHealthReport[]>` — `Promise.allSettled` over all 8 `probeService` calls, map settled results to `ServiceHealthReport[]` (rejected promises become `status: "unreachable"`) [owner:api-engineer]

## Batch 3 -- Soul Management

- [x] [3.1] [P-1] Create `packages/tools/meta-svc/src/soul.ts` — export `readSoul(): Promise<string>` that reads `config/soul.md` relative to `process.cwd()`, throws if file missing [owner:api-engineer]
- [x] [3.2] [P-1] Implement `writeSoul(content: string): Promise<void>` — write full content to `config/soul.md`, create parent dirs if needed, log write via pino [owner:api-engineer]

## Batch 4 -- Self-Assessment

- [x] [4.1] [P-1] Create `packages/tools/meta-svc/src/self-assess.ts` — implement `runSelfAssessment(): Promise<SelfAssessmentResult>` [owner:api-engineer]
- [x] [4.2] [P-1] Gather memory topics via `GET http://localhost:4001/api/memory` (3s timeout), extract topic count [owner:api-engineer]
- [x] [4.3] [P-1] Gather recent messages via `GET http://localhost:4002/api/messages?per_page=20` (3s timeout), extract message count and channel distribution [owner:api-engineer]
- [x] [4.4] [P-1] Call `probeFleet()` for fleet health summary (healthy/unhealthy/unreachable counts) [owner:api-engineer]
- [x] [4.5] [P-2] Generate `observations[]` — plain-text observations from gathered data (e.g. "12 memory topics", "7/8 services healthy", "20 recent messages across 2 channels") [owner:api-engineer]
- [x] [4.6] [P-2] Generate `suggestions[]` — actionable suggestions based on observations (e.g. "memory-svc unreachable, check systemd status", "no recent messages in last hour") [owner:api-engineer]
- [x] [4.7] [P-2] Wrap entire assessment in 10s timeout; on partial failure include error note in `observations` and return partial results [owner:api-engineer]

## Batch 5 -- HTTP Server

- [x] [5.1] [P-1] Create `packages/tools/meta-svc/src/server.ts` — Hono app with `logger()`, `cors({ origin: '*' })`, `secureHeaders()`, global JSON error handler [owner:api-engineer]
- [x] [5.2] [P-1] Implement `GET /health` — return `{ status: "ok", uptime_secs, version }` (read version from package.json at module load) [owner:api-engineer]
- [x] [5.3] [P-1] Implement `GET /services` — call `probeFleet()`, return `{ services: [...], summary: { total, healthy, unhealthy, unreachable } }` [owner:api-engineer]
- [x] [5.4] [P-1] Implement `POST /self-assess` — call `runSelfAssessment()`, return the result [owner:api-engineer]
- [x] [5.5] [P-1] Implement `GET /soul` — call `readSoul()`, return `{ content }` [owner:api-engineer]
- [x] [5.6] [P-1] Implement `POST /soul` — parse `{ content }` from body, validate non-empty (400 if missing), call `writeSoul(content)`, return `{ ok: true, bytes }` [owner:api-engineer]
- [x] [5.7] [P-1] Export `startServer(port: number): Promise<void>` — create `@hono/node-server` serve instance, log startup [owner:api-engineer]

## Batch 6 -- MCP Server

- [x] [6.1] [P-1] Create `packages/tools/meta-svc/src/mcp.ts` — MCP stdio server with 3 tool definitions: `check_services` (no params), `self_assessment_run` (no params), `update_soul` (params: `{ changes: string }`) [owner:api-engineer]
- [x] [6.2] [P-1] Wire `check_services` handler to `probeFleet()`, return JSON string of services + summary [owner:api-engineer]
- [x] [6.3] [P-1] Wire `self_assessment_run` handler to `runSelfAssessment()`, return JSON string [owner:api-engineer]
- [x] [6.4] [P-1] Wire `update_soul` handler to `writeSoul(changes)`, return confirmation string [owner:api-engineer]

## Batch 7 -- Entry Point

- [x] [7.1] [P-1] Create `packages/tools/meta-svc/src/index.ts` — import `startServer`, read `META_SVC_PORT` env (default 4008), call `await startServer(port)` [owner:api-engineer]

## Batch 8 -- Verify

- [x] [8.1] [P-1] `pnpm install` in workspace root succeeds, `@nova/meta-svc` resolves [owner:api-engineer]
- [x] [8.2] [P-1] `cd packages/tools/meta-svc && npx tsc --noEmit` passes with zero errors [owner:api-engineer]
- [x] [8.3] [P-1] `cd packages/tools/meta-svc && npx tsc` produces `dist/` with all .js files [owner:api-engineer]
- [x] [8.4] [P-2] `GET /health` returns `{ status: "ok", uptime_secs: number, version: "0.1.0" }` [owner:api-engineer]
- [x] [8.5] [P-2] `GET /services` returns 8 entries (mix of healthy/unreachable depending on which services are running) [owner:api-engineer]
- [x] [8.6] [P-2] `GET /soul` returns the content of `config/soul.md` [owner:api-engineer]
- [x] [8.7] [P-2] `POST /soul` with `{ "content": "test" }` writes the file and returns `{ ok: true, bytes: 4 }` [owner:api-engineer]
- [x] [8.8] [P-2] `POST /self-assess` returns a `SelfAssessmentResult` with partial or full data [owner:api-engineer]
- [x] [8.9] [P-3] `POST /soul` with empty body returns 400 [owner:api-engineer]
- [ ] [8.10] [user] Manual smoke: start meta-svc alongside at least memory-svc and messages-svc, verify `GET /services` shows them as healthy and `POST /self-assess` returns real data [owner:api-engineer]
