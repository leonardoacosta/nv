# Implementation Tasks

<!-- beads:epic:nv-oe4p -->

## DB Batch

- [x] [1.1] [P-1] Verify `packages/db/src/schema/diary.ts` exists and exports `diary` table — gate: `setup-postgres-drizzle` must be applied; confirm columns: id, trigger_type, trigger_source, channel, slug, content, tools_used, tokens_in, tokens_out, response_latency_ms, created_at [owner:db-engineer]

## API Batch

- [x] [2.1] [P-1] Create `packages/daemon/src/features/diary/writer.ts` — export `writeEntry(input: DiaryWriteInput): Promise<void>` that inserts into `diary` table via Drizzle db client; catch and log all errors; never re-throw [owner:api-engineer]
- [x] [2.2] [P-1] Create `packages/daemon/src/features/diary/reader.ts` — export `getEntriesByDate(date: string, limit?: number): Promise<DiaryEntryItem[]>` and `getEntriesByDateRange(from: string, to: string): Promise<DiaryEntryItem[]>`; map Drizzle rows to DiaryEntryItem shape matching `apps/dashboard/types/api.ts` [owner:api-engineer]
- [x] [2.3] [P-2] Create `packages/daemon/src/features/diary/index.ts` — barrel re-export of writer and reader [owner:api-engineer]
- [x] [2.4] [P-2] Integrate `writeEntry()` into Agent SDK response handler in `packages/daemon/src/agent.ts` — call after each response cycle with trigger_type, channel, slug, tools_used, token counts, and latency_ms [owner:api-engineer]
- [x] [2.5] [P-1] Create `packages/daemon/src/http/routes/diary.ts` — `GET /api/diary` handler; query params: `date` (YYYY-MM-DD, default today), `limit` (integer, default 50); validate date format with 400 on bad input; respond with `{ date, entries, total }` [owner:api-engineer]
- [x] [2.6] [P-2] Register `GET /api/diary` route in the HTTP server (wired to `add-http-api` server setup) [owner:api-engineer]
- [x] [2.7] [P-1] Create `packages/daemon/src/telegram/commands/diary.ts` — `/diary` command handler; parse optional YYYY-MM-DD argument; fall back to yesterday if today empty; format entries as compact text; truncate at 4000 chars; reply via Telegram bot [owner:api-engineer]
- [x] [2.8] [P-2] Register `/diary` command in the Telegram command router (wired to `add-telegram-adapter` command registry) [owner:api-engineer]
- [x] [2.9] [P-2] Run `pnpm typecheck` in `packages/daemon/` — zero TypeScript errors [owner:api-engineer]
