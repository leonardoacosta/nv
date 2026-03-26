# Proposal: Add Memory Service

## Change ID
`add-memory-svc`

## Summary

Hono microservice at port 4001 exposing read_memory, write_memory, and search_memory tools. Ports
the Rust daemon's Memory system to TypeScript with Postgres-backed storage (Drizzle), filesystem
sync to `~/.nv/memory/`, and pgvector embedding search. Dual HTTP (Hono) + MCP (stdio) transport.

## Context
- Depends on: `scaffold-tool-service` (service template), `migrate-to-shared-postgres` (shared DB)
- Wave: 3 (Phase 2 — Core Tools)
- Schema: `packages/db/src/schema/memory.ts` — uuid id, text topic (unique), text content, timestamp updatedAt
- Rust reference: `crates/nv-daemon/src/memory.rs` — Memory struct with read/write/search on markdown files
- Existing vector pattern: `packages/db/src/schema/messages.ts` — customType vector(1536) for pgvector
- Port: 4001 per architecture (nova-tools.target)

## Motivation

Nova's memory system currently lives inside the Rust daemon monolith. V10 architecture decomposes
tools into independent services. Memory is critical path — the agent reads/writes memory on nearly
every interaction. Making it an independent service allows:

1. Independent restart without affecting Telegram polling or agent dispatch
2. Direct HTTP access from the dashboard (via Traefik)
3. MCP native tool discovery by Claude Agent SDK
4. pgvector similarity search (new — Rust version used substring matching only)

## Requirements

### Req-1: Schema Migration — Add embedding Column

The existing `memory` table lacks an embedding column needed for vector search. Add a
`vector(1536)` column to `packages/db/src/schema/memory.ts` using the same customType pattern from
`messages.ts`. Generate a Drizzle migration. The column is nullable — existing rows get embeddings
backfilled lazily on next write or via a one-time script.

### Req-2: Service Scaffold

New package `packages/tools/memory-svc/` following the scaffold-tool-service template:
- `package.json` with `@nova/memory-svc` name, Hono + pino + `@nova/db` workspace dep
- `tsconfig.json` matching project conventions (ES2022, NodeNext)
- `src/index.ts` — Hono app on port 4001 with graceful shutdown
- `src/logger.ts` — pino logger named `memory-svc`

### Req-3: read_memory Tool

`POST /read` — Read a memory topic.

- Input: `{ topic: string }`
- Behavior: Query Postgres `memory` table by topic. If found, return content. Also read the
  corresponding file from `~/.nv/memory/{topic}.md` as a fallback if DB row is missing.
- Output: `{ topic, content, updatedAt }` or `{ error: "not_found" }` with 404
- Truncate content at 20,000 chars (matching Rust `MAX_MEMORY_READ_CHARS`)

### Req-4: write_memory Tool

`POST /write` — Upsert a memory entry.

- Input: `{ topic: string, content: string }`
- Behavior:
  1. Upsert into Postgres `memory` table (ON CONFLICT topic DO UPDATE content + updatedAt)
  2. Generate OpenAI embedding (text-embedding-3-small, 1536 dims) and store in embedding column
  3. Sync to filesystem: write `~/.nv/memory/{topic}.md` with frontmatter + content (matching
     Rust format: YAML frontmatter with topic, created, updated, entries count)
- Output: `{ topic, action: "created" | "updated" }`
- Embedding generation is best-effort — if OPENAI_API_KEY is missing or API fails, write succeeds
  without embedding (log warning)

### Req-5: search_memory Tool

`POST /search` — Search memory by semantic similarity.

- Input: `{ query: string, limit?: number }`
- Behavior:
  1. Generate embedding for the query string (text-embedding-3-small)
  2. pgvector cosine similarity search: `1 - (embedding <=> query_embedding)` ordered DESC
  3. Filter out rows with null embeddings
  4. Default limit: 10, max: 50
- Output: `{ results: Array<{ topic, content, similarity, updatedAt }> }`
- Fallback: If no embeddings exist or OPENAI_API_KEY is missing, fall back to case-insensitive
  substring search on topic + content (matching Rust behavior)

### Req-6: Health Endpoint

`GET /health` — Standard health check.

- Returns `{ status: "ok", service: "memory-svc", uptime: <seconds> }` with 200
- If Postgres connection fails, returns `{ status: "degraded", error: "..." }` with 503

### Req-7: Filesystem Sync

On every write, mirror the memory entry to `~/.nv/memory/{sanitized_topic}.md`. Topic sanitization:
lowercase, replace spaces/special chars with hyphens, strip leading/trailing hyphens.

The filesystem copy is the source of truth for the Rust daemon's legacy reads (backward compat
during migration). Once the Rust daemon is fully replaced, fs sync becomes optional.

### Req-8: MCP Server (stdio)

Expose the same 3 tools via MCP protocol on stdio transport for Agent SDK native discovery:
- `read_memory` — maps to POST /read
- `write_memory` — maps to POST /write
- `search_memory` — maps to POST /search

MCP server runs as a separate entrypoint (`src/mcp.ts`) invoked by Claude's `mcp.json` config
with `command: "node"` and `args: ["dist/mcp.js"]`.

### Req-9: Configuration

| Env Var | Default | Source |
|---------|---------|--------|
| `PORT` | `4001` | Service port |
| `DATABASE_URL` | (required) | Postgres connection string |
| `OPENAI_API_KEY` | (optional) | For embedding generation |
| `MEMORY_DIR` | `~/.nv/memory` | Filesystem sync directory |
| `NV_LOG_LEVEL` | `info` | Pino log level |

## Scope
- **IN**: Hono HTTP server, 3 tool endpoints + health, Postgres queries via Drizzle, filesystem sync,
  pgvector embedding search, MCP stdio server, schema migration for embedding column, pino logging
- **OUT**: Authentication/rate limiting (homelab only), MEMORY.md index file management,
  background summarization (Rust Memory feature — defer), topic listing endpoint, context summary
  builder

## Impact

| Area | Change |
|------|--------|
| `packages/db/src/schema/memory.ts` | Add embedding vector(1536) column |
| `packages/db/drizzle/` | New migration for embedding column |
| `packages/tools/memory-svc/` | New package: Hono service + MCP server |
| `pnpm-workspace.yaml` | Already includes `packages/*` — no change needed |

## Risks

| Risk | Mitigation |
|------|-----------|
| OpenAI API latency on writes | Embedding generation is best-effort; write succeeds without it |
| Stale filesystem copies | Postgres is source of truth; fs is write-through cache |
| Missing OPENAI_API_KEY | Graceful fallback to substring search; log warning once at startup |
| Schema migration on shared DB | Additive change (nullable column) — no data loss risk |
| Large memory files (>20K chars) | Truncation on read, matching Rust behavior |
