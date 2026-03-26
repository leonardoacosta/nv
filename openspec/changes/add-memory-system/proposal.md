# Proposal: Add Memory System (TypeScript)

## Change ID
`add-memory-system`

## Summary

Port the Nova memory system to TypeScript. Topic-based memory stored in Postgres via Drizzle
(introduced by `setup-postgres-drizzle`) with optional pgvector semantic search. Dual storage:
filesystem (`~/.nv/memory/*.md`) for Claude Code's Read/Write tool access; Postgres for
structured search and API. Exposes `GET /api/memory` and `PUT /api/memory` on the TS daemon.

## Context

- Depends on: `setup-postgres-drizzle` (provides `packages/db/` with `memory` table schema and
  Drizzle client), `scaffold-ts-daemon` (provides `packages/daemon/` structure)
- Related: `crates/nv-daemon/src/memory.rs` (Rust implementation — filesystem only, no
  Postgres, no vector search; this spec is not a drop-in replacement but a parallel TS port)
- Dashboard already has `apps/dashboard/app/api/memory/route.ts` which proxies to the daemon —
  once the TS daemon is live this route needs no changes (same URL shape)

## Motivation

The Rust memory system stores all data as markdown files in `~/.nv/memory/`. This works for
Claude Code access but has no search capability beyond substring grep. Moving to Postgres
provides:

1. **Keyword search** — SQL `LIKE` or full-text `tsvector` search across all topics in one query
2. **Vector similarity search** — optional pgvector `<->` distance on the `embedding` column
   (introduced by `setup-postgres-drizzle`) for semantic recall
3. **Typed API** — Drizzle schema in `packages/db/src/schema/memory.ts` gives the TS daemon
   type-safe access without string-based SQL
4. **Dashboard integration** — the existing memory dashboard page continues to work via the same
   `GET /api/memory` and `PUT /api/memory` endpoints

Filesystem write is retained so Claude Code (which uses `Read`/`Write` tools on
`~/.nv/memory/*.md`) can still access memory without any daemon running.

## Requirements

### Req-1: Drizzle CRUD Store

Create `packages/daemon/src/features/memory/store.ts`:

- Import the `memory` table from `@nova/db` (from `setup-postgres-drizzle`)
- Export `MemoryStore` class with:
  - `upsert(topic: string, content: string): Promise<MemoryRecord>` — inserts or updates by
    `topic` (unique); sets `updated_at` to `now()`
  - `get(topic: string): Promise<MemoryRecord | null>` — returns row or null
  - `list(): Promise<{ topic: string; updated_at: Date }[]>` — all topics (no content)
  - `delete(topic: string): Promise<void>` — removes a row (admin use only)
- `MemoryRecord` type: `{ id: string; topic: string; content: string; updated_at: Date }`

### Req-2: Filesystem Sync

Create `packages/daemon/src/features/memory/fs-sync.ts`:

- Export `MemoryFsSync` class with:
  - `write(topic: string, content: string): Promise<void>` — writes
    `~/.nv/memory/<sanitized-topic>.md`; creates the directory if absent
  - `read(topic: string): Promise<string | null>` — reads file; returns null if missing
  - `listTopics(): Promise<string[]>` — lists `*.md` filenames (stem only) in the memory dir
- `sanitizeTopic(topic: string): string` — strips non-alphanumeric chars except `-_`, lowercased

Both store and fs-sync are called together from the service layer, not independently from
handlers.

### Req-3: Search

Create `packages/daemon/src/features/memory/search.ts`:

- Export `MemorySearch` class constructed with the Drizzle `db` client
- `byKeyword(query: string, limit?: number): Promise<SearchResult[]>` — `WHERE content ILIKE
  '%<query>%' OR topic ILIKE '%<query>%'` with `LIMIT` (default 10)
- `bySimilarity(embedding: number[], limit?: number): Promise<SearchResult[]>` — `ORDER BY
  embedding <-> $1 LIMIT $2`; throws `Error("embeddings not configured")` if the `embedding`
  column is null on all rows (graceful degradation)
- `SearchResult` type: `{ topic: string; content: string; score?: number }`

Vector similarity requires the caller to supply a pre-computed embedding vector. The search
module does not call the embedding API itself.

### Req-4: Memory Service

Create `packages/daemon/src/features/memory/service.ts`:

- Composes `MemoryStore`, `MemoryFsSync`, and `MemorySearch`
- Export `MemoryService` class:
  - `get(topic: string): Promise<MemoryRecord | null>`
  - `upsert(topic: string, content: string): Promise<MemoryRecord>` — writes to Postgres then
    syncs to filesystem (Postgres is source of truth; fs write is best-effort, logged on error
    but does not throw)
  - `list(): Promise<{ topic: string; updated_at: Date }[]>`
  - `search(query: string, embedding?: number[]): Promise<SearchResult[]>` — if `embedding`
    provided, calls `bySimilarity`; otherwise calls `byKeyword`

### Req-5: HTTP Handlers

Create `packages/daemon/src/features/memory/handlers.ts`:

Two handlers matching the existing Rust API shape:

**GET /api/memory**
- No query params: returns `{ topics: string[] }` (topic names only — from `list()`)
- `?topic=<name>`: returns `{ topic: string; content: string }` or 404 if not found
- `?search=<query>`: returns `{ results: SearchResult[] }` (calls `service.search(query)`)

**PUT /api/memory**
- Body: `{ topic: string; content: string }`
- Calls `service.upsert(topic, content)`
- Returns `{ topic: string; written: number }` (`written` = `content.length`)
- 400 if `topic` or `content` missing

### Req-6: Feature Barrel

Create `packages/daemon/src/features/memory/index.ts`:
- Re-exports `MemoryService`, `MemoryStore`, `MemoryFsSync`, `MemorySearch`, handler functions
- Exports `createMemoryService(db: DrizzleClient): MemoryService` factory

### Req-7: Dashboard Proxy Compatibility

The existing `apps/dashboard/app/api/memory/route.ts` proxies to the daemon's `/api/memory`
with `topic` query param. No changes needed — the TS daemon must match this contract exactly:
- `GET /api/memory` → `{ topics: string[] }`
- `GET /api/memory?topic=<name>` → `{ topic: string; content: string }`
- `PUT /api/memory` body `{ topic, content }` → `{ topic, written }`

The `types/api.ts` interfaces `MemoryListResponse`, `MemoryTopicResponse`, `PutMemoryRequest`,
`PutMemoryResponse` already model this shape correctly — no dashboard changes needed.

## Scope

- **IN**: `packages/daemon/src/features/memory/` (store, fs-sync, search, service, handlers,
  index), dual Postgres + filesystem write path, GET/PUT /api/memory handlers
- **OUT**: Embedding generation (caller provides vector), dashboard UI changes, migration
  changes (schema already defined in `setup-postgres-drizzle`), Rust daemon changes, seeding
  existing `~/.nv/memory/*.md` files into Postgres (separate migration script)

## Impact

| Area | Change |
|------|--------|
| `packages/daemon/src/features/memory/store.ts` | New: Drizzle CRUD for memory table |
| `packages/daemon/src/features/memory/fs-sync.ts` | New: filesystem read/write sync |
| `packages/daemon/src/features/memory/search.ts` | New: keyword + vector search |
| `packages/daemon/src/features/memory/service.ts` | New: service composing store + sync + search |
| `packages/daemon/src/features/memory/handlers.ts` | New: HTTP handlers (GET, PUT) |
| `packages/daemon/src/features/memory/index.ts` | New: barrel re-exports + factory |

No changes to `apps/dashboard/`, `crates/`, `packages/db/`, or `docker-compose.yml`.

## Risks

| Risk | Mitigation |
|------|-----------|
| `packages/db` not yet applied when this runs | `setup-postgres-drizzle` is a hard prerequisite; CI will fail to typecheck otherwise |
| Filesystem write fails (permissions, disk full) | Best-effort: log error, do not throw — Postgres is source of truth |
| Vector search on empty embedding column | `bySimilarity` catches null embeddings and throws a descriptive error; caller falls back to keyword |
| `topic` values contain path traversal chars | `sanitizeTopic` strips all non-alphanumeric chars before writing to filesystem |
| TS daemon not yet running | Handlers are wired into the daemon router in a downstream spec; this spec is feature logic only |
