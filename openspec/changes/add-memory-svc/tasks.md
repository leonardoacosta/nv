# Implementation Tasks

<!-- beads:epic:TBD -->

## Schema Migration

- [x] [1.1] [P-1] Add vector customType to packages/db/src/schema/memory.ts — reuse pattern from messages.ts (customType with toDriver/fromDriver) [owner:db-engineer]
- [x] [1.2] [P-1] Add nullable embedding column `vector("embedding", { dimensions: 1536 })` to memory table [owner:db-engineer]
- [x] [1.3] [P-1] Run `pnpm drizzle-kit generate` to produce migration SQL for the new column [owner:db-engineer]

## Service Scaffold

- [x] [2.1] [P-1] Create packages/tools/memory-svc/package.json — @nova/memory-svc, type:module, deps: hono, @hono/node-server, @nova/db, pino, openai; devDeps: typescript, @types/node, pino-pretty [owner:api-engineer]
- [x] [2.2] [P-1] Create packages/tools/memory-svc/tsconfig.json — strict, ES2022, NodeNext, outDir:dist, rootDir:src [owner:api-engineer]
- [x] [2.3] [P-1] Create packages/tools/memory-svc/src/logger.ts — pino logger named "memory-svc", NV_LOG_LEVEL env [owner:api-engineer]
- [x] [2.4] [P-1] Create packages/tools/memory-svc/src/config.ts — typed config from env vars (PORT, DATABASE_URL, OPENAI_API_KEY, MEMORY_DIR, NV_LOG_LEVEL) with defaults [owner:api-engineer]
- [x] [2.5] [P-1] Create packages/tools/memory-svc/src/index.ts — Hono app with /health, /read, /write, /search routes, serve on PORT, graceful shutdown on SIGTERM/SIGINT [owner:api-engineer]

## Tool Implementation

- [x] [3.1] [P-1] Create packages/tools/memory-svc/src/tools/read.ts — POST /read handler: query memory table by topic, fallback to filesystem read, truncate at 20K chars [owner:api-engineer]
- [x] [3.2] [P-1] Create packages/tools/memory-svc/src/tools/write.ts — POST /write handler: upsert memory row (ON CONFLICT topic), generate embedding (best-effort), sync to filesystem [owner:api-engineer]
- [x] [3.3] [P-1] Create packages/tools/memory-svc/src/tools/search.ts — POST /search handler: embed query, pgvector cosine similarity, fallback to substring search if no embeddings/key [owner:api-engineer]
- [x] [3.4] [P-2] Create packages/tools/memory-svc/src/embedding.ts — OpenAI text-embedding-3-small wrapper, returns number[] or null on failure, logs warning if API key missing [owner:api-engineer]
- [x] [3.5] [P-2] Create packages/tools/memory-svc/src/filesystem.ts — sanitizeTopic(), writeMemoryFile(topic, content), readMemoryFile(topic) with YAML frontmatter matching Rust format [owner:api-engineer]

## Health Endpoint

- [x] [4.1] [P-2] Implement GET /health — return service name, status ok/degraded, uptime seconds, test Postgres connectivity [owner:api-engineer]

## MCP Server

- [x] [5.1] [P-2] Create packages/tools/memory-svc/src/mcp.ts — MCP stdio server exposing read_memory, write_memory, search_memory tools with JSON schema input validation [owner:api-engineer]
- [x] [5.2] [P-2] Add "mcp" script to package.json — `node dist/mcp.js` entrypoint [owner:api-engineer]

## Verify

- [x] [6.1] pnpm install from workspace root succeeds [owner:api-engineer]
- [x] [6.2] tsc --noEmit passes for @nova/memory-svc [owner:api-engineer]
- [x] [6.3] tsc --noEmit passes for @nova/db (schema change) [owner:db-engineer]
- [x] [6.4] Service starts on port 4001 and GET /health returns 200 [owner:api-engineer]
- [x] [6.5] POST /write creates memory row + filesystem file [owner:api-engineer]
- [x] [6.6] POST /read returns written content [owner:api-engineer]
- [x] [6.7] POST /search returns results (substring fallback if no OPENAI_API_KEY) [owner:api-engineer]
- [ ] [6.8] [user] Manual test: run service, write memory via curl, read it back, search for it [owner:api-engineer]
- [ ] [6.9] [user] Manual test: verify ~/.nv/memory/{topic}.md file created with correct frontmatter [owner:api-engineer]
