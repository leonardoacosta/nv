# Implementation Tasks
<!-- beads:epic:nv-pezp -->

## DB Batch

- [x] [1.1] [P-1] Scaffold messages-svc package -- create `packages/tools/messages-svc/` with `package.json` (`@nova/messages-svc`), `tsconfig.json`, `build.mjs` (esbuild to `dist/messages-svc.js`); add `@nova/db`, `hono`, `@hono/node-server`, `pino`, `drizzle-orm`, `postgres` as dependencies [owner:api-engineer] [beads:nv-y1fp]

## API Batch

- [x] [2.1] [P-1] Implement get_recent_messages tool -- in `src/tools.ts`, create `getRecentMessages(channel?: string, limit?: number)` that queries `messages` table via Drizzle; filter by `channel` when provided; order by `created_at` desc; default limit 20, max 100; return `Message[]` [owner:api-engineer] [beads:nv-8ve0]
- [x] [2.2] [P-1] Implement search_messages tool -- in `src/tools.ts`, create `searchMessages(query: string, channel?: string, limit?: number)` that runs ILIKE `%query%` on `content` column; filter by `channel` when provided; order by `created_at` desc; default limit 10, max 50; return `Message[]` [owner:api-engineer] [beads:nv-787x]
- [x] [2.3] [P-1] Add Hono HTTP routes -- in `src/index.ts`, create Hono app on port 4002 with: `GET /recent` (query params: channel, limit), `POST /search` (JSON body: query, channel, limit), `GET /health` (returns status, service name, uptime); add CORS, pino request logging [owner:api-engineer] [beads:nv-lie0]
- [x] [2.4] [P-2] Add MCP stdio server -- in `src/mcp.ts`, implement MCP stdio transport exposing `get_recent_messages` and `search_messages` tools with JSON Schema input definitions; entry via `--mcp` CLI flag or separate bin entry [owner:api-engineer] [beads:nv-tsd1]

## Build Batch

- [x] [3.1] [P-2] Add esbuild bundle and verify build -- configure `build.mjs` to bundle `src/index.ts` to `dist/messages-svc.js` (ESM, node20 target, externalize postgres native); verify `node dist/messages-svc.js` starts and responds to `GET /health` [owner:api-engineer] [beads:nv-5iec]
