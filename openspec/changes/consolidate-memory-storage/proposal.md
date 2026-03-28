# Proposal: Consolidate Memory Storage

## Change ID
`consolidate-memory-storage`

## Summary

Eliminate the filesystem fallback in memory-svc. Currently, memory is stored in both PostgreSQL
(with pgvector embeddings) and the filesystem. The `handleRead` function falls back to filesystem
when the DB returns empty. `handleWrite` writes to both, but if either fails, they silently diverge
with no reconciliation.

## Context
- Service: `packages/tools/memory-svc/`
- `handleRead`: queries Postgres first, falls back to `readMemoryFile(config.memoryDir, topic)`
- `handleWrite`: generates embedding via OpenAI, upserts to Postgres, AND syncs to filesystem
- `handleSearch`: uses pgvector similarity search, falls back to ILIKE substring search
- The dual-write pattern means failures in either storage path silently create divergence
- No reconciliation mechanism exists
- Original filesystem storage predates the Postgres migration; kept as fallback during transition

## Motivation

Dual-storage adds complexity with no benefit now that Postgres with pgvector is stable. The
filesystem was the original storage before the DB migration. Keeping it as a fallback creates a
false sense of reliability -- users may read stale filesystem data without knowing the DB has newer
content. Removing it simplifies the write path, eliminates a class of silent divergence bugs, and
reduces the surface area of memory-svc.

## Requirements

### Req-1: Remove Filesystem from Write Path

In `packages/tools/memory-svc/src/handlers/write.ts`:
- Remove the `syncToFilesystem()` call
- Write exclusively to PostgreSQL
- Keep embedding generation (OpenAI) as best-effort -- write succeeds even if embedding fails

### Req-2: Remove Filesystem Fallback from Read Path

In `packages/tools/memory-svc/src/handlers/read.ts`:
- Remove the `readMemoryFile()` fallback
- Return empty/null when topic not found in DB
- Log at DEBUG level when topic not found (not WARN)

### Req-3: One-Time Migration Script

Create a migration script that ensures all filesystem memory data exists in Postgres before the
filesystem path is removed:
- Read all memory files from `memoryDir`
- For each file: upsert into Postgres (skip if topic already exists with a newer `updatedAt`)
- Generate embeddings for any migrated topics missing embeddings
- Rate-limit embedding generation to 10 topics/min to avoid OpenAI quota issues
- Script runs once, then the filesystem memory directory can be archived

### Req-4: Config Cleanup

- Remove `memoryDir` from config (no longer needed after migration)
- Remove filesystem-related imports and utilities (e.g., `readMemoryFile`, `syncToFilesystem`)

## Scope
- **IN**: `packages/tools/memory-svc/src/handlers/` (read.ts, write.ts), config, migration script,
  filesystem utility removal
- **OUT**: Embedding generation logic (kept as-is), search logic (kept -- already DB-only for
  vector search, ILIKE fallback is DB-side), MCP server interface (unchanged)

## Impact

| Area | Change |
|------|--------|
| `packages/tools/memory-svc/src/handlers/write.ts` | Simplified -- remove filesystem sync |
| `packages/tools/memory-svc/src/handlers/read.ts` | Simplified -- remove filesystem fallback |
| `packages/tools/memory-svc/src/config.ts` | Remove `memoryDir` config |
| `scripts/migrate-memory-to-db.ts` | New -- one-time migration script |

## Risks

| Risk | Mitigation |
|------|-----------|
| Data loss if DB has gaps filesystem does not | Migration script (Req-3) runs first, upserts all missing topics before removal |
| Embedding generation quota limits during migration | Rate-limit migration to 10 topics/min |
| `memoryDir` still referenced elsewhere in the codebase | Grep for `memoryDir` across the full codebase before removing |
