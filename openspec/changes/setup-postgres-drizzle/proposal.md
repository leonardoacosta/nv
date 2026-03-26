# Proposal: Setup Postgres + Drizzle for Nova TS Daemon

## Change ID
`setup-postgres-drizzle`

## Summary

Add a local Postgres + pgvector service to Docker Compose and create a `packages/db/` TypeScript package with Drizzle ORM schemas for messages, obligations, contacts, diary entries, and memory — providing a shared, typed data layer for the upcoming Nova TS daemon.

## Context
- Extends: `docker-compose.yml` (add postgres service alongside dashboard)
- Related: `2026-03-22-add-message-store` (SQLite messages schema — ported to Postgres), `2026-03-22-add-interaction-diary` (diary schema — ported to Postgres), `scaffold-ts-daemon` (consumer of this package)
- New: `packages/db/` — first TypeScript package outside `apps/`; requires root `package.json` + `pnpm-workspace.yaml`

## Motivation

The Nova TS daemon (scaffold-ts-daemon) needs a shared, strongly-typed data layer. The existing Rust daemon uses SQLite via `rusqlite` with hand-written migrations. A Postgres + Drizzle setup provides:

1. **pgvector support** — `embedding vector(1536)` on the messages table enables semantic search (Claude embeddings)
2. **Shared schema** — both the TS daemon and the dashboard can import types from `packages/db`
3. **Type-safe queries** — Drizzle generates TypeScript types from schema definitions, eliminating raw SQL strings
4. **Migration management** — `drizzle-kit generate` + `drizzle-kit migrate` replaces manual SQL files

## Requirements

### Req-1: Docker Compose Postgres Service

Add a `postgres` service to `docker-compose.yml`:
- Image: `pgvector/pgvector:pg17`
- Port: `5432` (host-bound for local dev)
- Volume: `nova-pg-data` (named, persists across restarts)
- Credentials: user `nova`, password `nova-local`, db `nova`
- Networks: `homelab` (matches existing dashboard service)

### Req-2: Root TS Workspace Setup

Create root `package.json` (private, workspaces: `apps/*`, `packages/*`) and `pnpm-workspace.yaml` to enable cross-package imports. No new scripts needed at root level beyond workspace definition.

### Req-3: `packages/db/` Package

Create a standalone TypeScript package:
- `package.json` with `drizzle-orm`, `postgres` (postgres.js), `@types/pg` devDep
- `tsconfig.json` extending strict settings, outputs to `dist/`
- `src/index.ts` — re-exports schema tables and `db` client
- `src/client.ts` — creates and exports a `postgres.js`-backed `drizzle()` client using `DATABASE_URL` env var

### Req-4: Schema Definitions

Five schema files under `packages/db/src/schema/`:

**messages.ts** — inbound/outbound message log with vector embeddings:
- `id` (uuid, pk), `channel` (text), `sender` (text, nullable), `content` (text), `metadata` (jsonb, nullable), `created_at` (timestamp), `embedding` (vector(1536) via pgvector custom type)

**obligations.ts** — detected actions / commitments tracking:
- `id` (uuid, pk), `detected_action` (text), `owner` (text), `status` (text — pending/in_progress/done/cancelled), `priority` (integer), `project_code` (text, nullable), `source_channel` (text), `source_message` (text, nullable), `deadline` (timestamp, nullable), `last_attempt_at` (timestamp, nullable), `created_at` (timestamp), `updated_at` (timestamp)

**contacts.ts** — known people across channels:
- `id` (uuid, pk), `name` (text), `channel_ids` (jsonb), `relationship_type` (text, nullable), `notes` (text, nullable), `created_at` (timestamp)

**diary.ts** — interaction diary log (replaces file-based diary):
- `id` (uuid, pk), `trigger_type` (text), `trigger_source` (text), `channel` (text), `slug` (text), `content` (text), `tools_used` (jsonb, nullable), `tokens_in` (integer, nullable), `tokens_out` (integer, nullable), `response_latency_ms` (integer, nullable), `created_at` (timestamp)

**memory.ts** — durable key/topic memory store:
- `id` (uuid, pk), `topic` (text, unique), `content` (text), `updated_at` (timestamp)

### Req-5: Drizzle Config

`packages/db/drizzle.config.ts` pointing to `DATABASE_URL` env var, schema glob `src/schema/*.ts`, output dir `drizzle/`.

### Req-6: Migration Scripts

`package.json` scripts in `packages/db/`:
- `db:generate` — runs `drizzle-kit generate`
- `db:migrate` — runs `drizzle-kit migrate`
- `db:studio` — runs `drizzle-kit studio` (local dev introspection)

Initial migration generated and committed to `packages/db/drizzle/`.

## Scope
- **IN**: Docker Compose postgres service, root TS workspace, `packages/db/` package, five schema files, drizzle config, migration scripts, initial migration generation
- **OUT**: Connecting the Rust daemon to Postgres (separate spec), TS daemon implementation (scaffold-ts-daemon), dashboard query integration, vector search tooling, seeding scripts, CI database provisioning

## Impact
| Area | Change |
|------|--------|
| `docker-compose.yml` | Add postgres + pgvector service, nova-pg-data volume |
| `package.json` (root) | New: private workspace root |
| `pnpm-workspace.yaml` | New: defines apps/* + packages/* |
| `packages/db/` | New: full Drizzle package |
| `packages/db/src/schema/*.ts` | New: 5 schema files |
| `packages/db/drizzle/` | New: generated initial migration |

## Risks
| Risk | Mitigation |
|------|-----------|
| pgvector extension not loaded | Add `POSTGRES_INITDB_ARGS` or init SQL to enable extension on first start |
| `packages/` breaks Cargo workspace | Cargo.toml only references `crates/` — no collision |
| `pnpm-workspace.yaml` clashes with dashboard's standalone `package-lock.json` | Dashboard keeps its own lock; workspace adds pnpm lock at root — document in README |
| `DATABASE_URL` conflicts with existing Neon tools | Neon tools use `POSTGRES_URL_{CODE}` pattern — no collision |
