# Implementation Tasks

## DB Batch
(No DB schema changes)

## API Batch
- [ ] [2.1] [P-1] Remove filesystem fallback from `packages/tools/memory-svc/src/tools/read.ts` — delete `readMemoryFile` import and fallback block, return 404 directly when DB row not found, log at DEBUG level [owner:api-engineer]
- [ ] [2.2] [P-1] Remove filesystem sync from `packages/tools/memory-svc/src/tools/write.ts` — delete `writeMemoryFile` import and the try/catch filesystem sync block [owner:api-engineer]
- [ ] [2.3] [P-1] Remove filesystem fallback from MCP read_memory tool in `packages/tools/memory-svc/src/mcp.ts` — delete `readMemoryFile` import usage and fallback block (lines 58-73) [owner:api-engineer]
- [ ] [2.4] [P-1] Remove filesystem sync from MCP write_memory tool in `packages/tools/memory-svc/src/mcp.ts` — delete `writeMemoryFile` usage and try/catch block (lines 123-127) [owner:api-engineer]
- [ ] [2.5] [P-2] Remove `memoryDir` from `MemorySvcConfig` interface and `loadConfig()` in `packages/tools/memory-svc/src/config.ts` — delete field, env var read, and `homedir`/`join` imports if unused [owner:api-engineer]
- [ ] [2.6] [P-2] Delete `packages/tools/memory-svc/src/filesystem.ts` — entire file is dead code after 2.1-2.4 [owner:api-engineer]
- [ ] [2.7] [P-2] Remove `filesystem.js` import from `packages/tools/memory-svc/src/mcp.ts` — clean up the remaining import line after 2.3 and 2.4 [owner:api-engineer]
- [ ] [2.8] [P-3] Create one-time migration script `packages/tools/memory-svc/scripts/migrate-fs-to-db.ts` — read all `.md` files from memoryDir, upsert into Postgres (skip if DB topic has newer updatedAt), generate embeddings for migrated topics missing them, rate-limit embedding calls to 10/min [owner:api-engineer]

## UI Batch
(No UI changes)

## E2E Batch
- [ ] [4.1] [P-2] Test: verify handleRead returns 404 JSON when topic not found in DB (no filesystem fallback) [owner:e2e-engineer]
- [ ] [4.2] [P-2] Test: verify handleWrite succeeds without filesystem side-effects (mock fs to confirm no writes) [owner:e2e-engineer]
- [ ] [4.3] [P-3] Test: verify migration script upserts filesystem topics into DB and skips topics with newer DB entries [owner:e2e-engineer]
