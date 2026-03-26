# Implementation Tasks

<!-- beads:epic:nv-egdp -->

## Infra Batch

- [x] [1.1] [P-1] Add `packages/tools/*` to `pnpm-workspace.yaml` packages array if not already present [owner:api-engineer] [beads:nv-qi8b]
- [x] [1.2] [P-1] Create `packages/tools/tool-router/` package scaffold: `package.json` (name: `@nova/tool-router`, deps: hono, @hono/node-server, pino), `tsconfig.json` (same pattern as daemon), `src/index.ts` entry point with Hono server on `:4000`, pino logger, CORS [owner:api-engineer] [beads:nv-9w63]

## API Batch

- [x] [2.1] [P-1] Implement `src/registry.ts`: typed constant mapping all 30 tool names to `{ serviceUrl, serviceName }` entries across the 8 services (memory :4001, messages :4002, channels :4003, discord :4004, teams :4005, schedule :4006, graph :4007, meta :4008); export `getServiceForTool(name)` and `getAllServices()` helpers [owner:api-engineer] [beads:nv-devu]
- [x] [2.2] [P-1] Implement `POST /dispatch` route: parse `{ tool, input }` body, look up tool in registry, forward as `POST {serviceUrl}/tools/{tool}` with input as body, return downstream response; 404 for unknown tool, 502 for unreachable service; include pino request logging [owner:api-engineer] [beads:nv-qjcr]
- [x] [2.3] [P-1] Implement `GET /health` route: call `GET {serviceUrl}/health` on all 8 services in parallel with 3s timeout per service, aggregate into `{ status, services, healthy_count, total_count }` response; status is healthy/degraded/unhealthy based on response counts [owner:api-engineer] [beads:nv-r4eh]
- [x] [2.4] [P-2] Implement `GET /registry` route: return the full tool-to-service mapping from registry.ts as JSON [owner:api-engineer] [beads:nv-0y01]

## E2E Batch

- [x] [3.1] [P-2] Add unit tests for dispatch (unknown tool returns 404, unreachable service returns 502) and health aggregation (all-healthy, degraded, all-unhealthy cases) using node:test [owner:api-engineer] [beads:nv-s2b5]
