# Implementation Tasks

<!-- beads:epic:nv-c0bl -->

## Drizzle Store

- [x] [1.1] [P-1] Create packages/daemon/src/features/memory/store.ts — MemoryStore class with upsert(), get(), list(), delete() using @nova/db memory table [owner:api-engineer]
- [x] [1.2] [P-1] Export MemoryRecord type from store.ts: { id: string; topic: string; content: string; updated_at: Date } [owner:api-engineer]

## Filesystem Sync

- [x] [2.1] [P-1] Create packages/daemon/src/features/memory/fs-sync.ts — MemoryFsSync class with write(), read(), listTopics() [owner:api-engineer]
- [x] [2.2] [P-2] Implement sanitizeTopic() — strip non-alphanumeric except -_ + lowercase [owner:api-engineer]
- [x] [2.3] [P-2] write() creates ~/.nv/memory/ directory if absent (fs.mkdir recursive) [owner:api-engineer]

## Search

- [x] [3.1] [P-2] Create packages/daemon/src/features/memory/search.ts — MemorySearch class [owner:api-engineer]
- [x] [3.2] [P-2] Implement byKeyword(query, limit?) — SQL ILIKE on content + topic columns [owner:api-engineer]
- [x] [3.3] [P-3] Implement bySimilarity(embedding, limit?) — pgvector <-> distance ORDER BY; throws graceful error if no embeddings present [owner:api-engineer]
- [x] [3.4] [P-2] Export SearchResult type: { topic: string; content: string; score?: number } [owner:api-engineer]

## Service Layer

- [x] [4.1] [P-1] Create packages/daemon/src/features/memory/service.ts — MemoryService composing MemoryStore + MemoryFsSync + MemorySearch [owner:api-engineer]
- [x] [4.2] [P-1] Implement upsert() — write to Postgres then sync filesystem (fs failure is best-effort: log, do not throw) [owner:api-engineer]
- [x] [4.3] [P-1] Implement get() and list() delegating to MemoryStore [owner:api-engineer]
- [x] [4.4] [P-2] Implement search(query, embedding?) — route to bySimilarity if embedding provided, else byKeyword [owner:api-engineer]

## HTTP Handlers

- [x] [5.1] [P-1] Create packages/daemon/src/features/memory/handlers.ts — getMemory and putMemory handler functions [owner:api-engineer]
- [x] [5.2] [P-1] getMemory: no params → { topics: string[] }; ?topic=<name> → { topic, content } or 404; ?search=<query> → { results: SearchResult[] } [owner:api-engineer]
- [x] [5.3] [P-1] putMemory: validate { topic, content } body → call service.upsert → return { topic, written: content.length }; 400 on missing fields [owner:api-engineer]

## Barrel

- [x] [6.1] [P-2] Create packages/daemon/src/features/memory/index.ts — re-export all classes, types, and handler functions; export createMemoryService() factory [owner:api-engineer]

## Verify

- [x] [7.1] pnpm typecheck passes in packages/daemon/ [owner:api-engineer]
- [x] [7.2] Unit tests: store upsert/get/list, fs-sync write/read/sanitize, search byKeyword, service upsert dual-write (mocked db + fs) [owner:api-engineer]
- [x] [7.3] GET /api/memory returns { topics: [] } with no data [owner:api-engineer]
- [x] [7.4] PUT /api/memory { topic: "test", content: "hello" } then GET /api/memory?topic=test returns { topic: "test", content: "hello" } [owner:api-engineer]
- [x] [7.5] GET /api/memory?search=hello returns { results: [{ topic: "test", content: "hello" }] } [owner:api-engineer]
- [x] [7.6] Filesystem: ~/.nv/memory/test.md exists and matches content after PUT [owner:api-engineer]
