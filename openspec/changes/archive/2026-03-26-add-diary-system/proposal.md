# Proposal: Add Diary System (TypeScript)

## Change ID
`add-diary-system`

## Summary

Port the interaction diary to TypeScript. Every processed message produces a Postgres-backed diary entry written after each Agent SDK response, with a reader module, a `/diary` Telegram command, and a `GET /api/diary` REST endpoint.

## Context
- Extends: `packages/db/src/schema/diary.ts` (table provided by `setup-postgres-drizzle`), `packages/db/src/client.ts` (db client), `packages/daemon/src/` (TS daemon from `scaffold-ts-daemon`)
- Related: `2026-03-22-add-interaction-diary` (Rust file-based diary — this spec replaces it with Postgres), `setup-postgres-drizzle` (hard dependency — must apply first), `scaffold-ts-daemon` (hard dependency — must apply first)

## Motivation

The Rust diary writes markdown files to `~/.nv/diary/YYYY-MM-DD.md`. The TS daemon needs a structured, queryable alternative in Postgres so that:

1. **Dashboard integration** — the existing `/diary` dashboard page (`apps/dashboard/app/diary/`) already has a `DiaryGetResponse` / `DiaryEntryItem` type contract and a proxy route. The TS daemon must satisfy that contract.
2. **Queryability** — Postgres rows can be filtered by date, channel, trigger type, and tool usage without parsing markdown.
3. **Telegram accountability** — `/diary` command gives a quick mobile-friendly summary of what Nova did.
4. **Agent SDK fit** — inserting a row post-response requires no additional Claude API call and adds <5 ms to the response path.

## Requirements

### Req-1: Diary Writer (`src/features/diary/writer.ts`)

After each Agent SDK response cycle, insert one row into the `diary` Postgres table. The writer must:

- Accept: `trigger_type`, `channel`, `slug`, `tools_used` (string array), `tokens_in`, `tokens_out`, `response_latency_ms`, `content` (summary string)
- Derive `trigger_source` from channel context available in the caller
- Use the `db` client from `packages/db/src/client.ts`
- Insert via Drizzle — no raw SQL
- Be called from the Agent SDK response handler, not from within the SDK callback chain
- Never throw — catch and log errors so a diary failure never disrupts the main response path

### Req-2: Diary Reader (`src/features/diary/reader.ts`)

Query diary entries for display purposes:

- `getEntriesByDate(date: string, limit?: number): Promise<DiaryEntryItem[]>` — filter by `created_at::date`, ordered descending, default limit 50
- `getEntriesByDateRange(from: string, to: string): Promise<DiaryEntryItem[]>` — inclusive date range query
- Return shape must be compatible with `DiaryEntryItem` from `apps/dashboard/types/api.ts` so the dashboard proxy route works unchanged
- Use the `db` client from `packages/db/src/client.ts`

### Req-3: `/diary` Telegram Command

Add a `/diary` Telegram bot command that:

- With no argument: shows last 10 entries for today (falls back to yesterday if today is empty)
- With `YYYY-MM-DD` argument: shows last 10 entries for that date
- Formats each entry as a compact text block: time, trigger type, channel, tools used (comma-joined or "none"), token cost, latency
- Sends the reply as a single Telegram message (truncate at 4 000 chars if needed)

### Req-4: `GET /api/diary` Endpoint

Add a `GET /api/diary` handler to the TS daemon's HTTP server (the server provided by `add-http-api` spec):

- Query params: `date` (YYYY-MM-DD, default today), `limit` (integer, default 50)
- Response: `{ date, entries: DiaryEntryItem[], total }` — matching `DiaryGetResponse` from `apps/dashboard/types/api.ts`
- Returns 200 with empty `entries: []` when no entries exist for the given date
- Returns 400 for a malformed date

## Scope
- **IN**: `src/features/diary/writer.ts`, `src/features/diary/reader.ts`, `/diary` Telegram command handler, `GET /api/diary` HTTP handler, integration into the Agent SDK response path
- **OUT**: Dashboard UI changes (existing page already works against the Rust daemon proxy; no UI changes needed), diary schema migration (owned by `setup-postgres-drizzle`), diary purge/retention policy, vector search over diary entries, diary-to-memory summarization

## Impact
| Area | Change |
|------|--------|
| `packages/daemon/src/features/diary/writer.ts` | New: DiaryWriter with `writeEntry()` |
| `packages/daemon/src/features/diary/reader.ts` | New: `getEntriesByDate()`, `getEntriesByDateRange()` |
| `packages/daemon/src/features/diary/index.ts` | New: barrel re-export |
| `packages/daemon/src/agent.ts` (or equivalent SDK handler) | Modified: call `writeEntry()` after each response |
| `packages/daemon/src/telegram/commands/diary.ts` | New: `/diary` command handler |
| `packages/daemon/src/http/routes/diary.ts` | New: `GET /api/diary` route handler |

## Risks
| Risk | Mitigation |
|------|-----------|
| Diary write fails and breaks message flow | Writer wraps insert in try/catch — logs error, never re-throws |
| `DiaryEntryItem` shape drift between dashboard types and Drizzle schema | Reader maps Drizzle row to `DiaryEntryItem` explicitly; dashboard types are the canonical contract |
| Large entry counts slow Telegram command | Default limit 10 with character cap (4 000) prevents message-too-long errors |
| `setup-postgres-drizzle` not yet applied | Hard dependency — diary schema table won't exist; declare in tasks.md gate |
