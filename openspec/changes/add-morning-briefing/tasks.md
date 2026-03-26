# Implementation Tasks

<!-- beads:epic:nv-kblx -->

## DB Schema Batch

- [ ] [1.1] Create `packages/db/src/schema/briefings.ts` — Drizzle pgTable `briefings` with columns: `id` (uuid pk defaultRandom), `generated_at` (timestamp with tz, notNull, defaultNow), `content` (text, notNull), `sources_status` (jsonb, notNull, default `{}`), `suggested_actions` (jsonb, notNull, default `[]`) [owner:db-engineer]
- [ ] [1.2] Re-export `briefings` table from `packages/db/src/index.ts` [owner:db-engineer]
- [ ] [1.3] Run `pnpm --filter @nova/db db:generate` to produce migration adding briefings table; commit output to `packages/db/drizzle/` [owner:db-engineer]

## Scheduler Batch

- [ ] [2.1] Create `packages/daemon/src/features/briefing/scheduler.ts` — `startBriefingScheduler(deps: BriefingDeps): () => void`; polls every 60s with `setInterval`; on each tick checks `new Date().getHours() === 7`; tracks `lastBriefingDate: string | null` (YYYY-MM-DD) to prevent double-fire; calls `runMorningBriefing(deps)` as fire-and-forget; returns cleanup function [owner:api-engineer]

## Synthesizer Batch

- [ ] [3.1] Create `packages/daemon/src/features/briefing/synthesizer.ts` — export `BriefingDeps` interface (`db`, `agentClient`, `logger`); export `gatherContext(deps)` that runs parallel `Promise.allSettled` fetches of: top-20 open/in-progress obligations ordered by priority + created_at, top-10 memory entries ordered by updated_at desc, last-20 messages ordered by created_at desc; each fetch races against a 10s timeout; returns `GatheredContext` with data + `sourcesStatus` record [owner:api-engineer]
- [ ] [3.2] Implement `synthesizeBriefing(context: GatheredContext, deps: BriefingDeps): Promise<SynthesisResult>` in `synthesizer.ts` — builds system prompt instructing Claude to produce sections: Obligations, Memory Highlights, Recent Activity, Suggested Actions; calls `deps.agentClient.query(prompt, systemPrompt)` with 30s timeout; parses response for `suggestedActions`; on error/timeout falls back to static summary of obligation counts; returns `{ content: string, suggestedActions: Array<{ label: string; url?: string }> }` [owner:api-engineer]

## Runner Batch

- [ ] [4.1] Create `packages/daemon/src/features/briefing/runner.ts` — export `runMorningBriefing(deps: BriefingDeps): Promise<void>`; calls `gatherContext` then `synthesizeBriefing`; inserts row into `briefings` table via `db.insert(briefings).values({ content, sources_status, suggested_actions })`; logs success with `generated_at`; on error logs and re-throws [owner:api-engineer]

## HTTP Route Batch

- [ ] [5.1] Create `packages/daemon/src/http/routes/briefing.ts` — `GET /api/briefing` handler: selects most recent briefing row via `db.select().from(briefings).orderBy(desc(briefings.generated_at)).limit(1)`; returns `200 { id, generated_at, content, sources_status, suggested_actions }` or `404 { error: "no briefing available" }` [owner:api-engineer]
- [ ] [5.2] Add `GET /api/briefing/history` handler to `packages/daemon/src/http/routes/briefing.ts` — accepts `?limit=N` (default 10, max 30); returns `200 { entries: [...] }` newest-first; empty array is valid (no 404) [owner:api-engineer]
- [ ] [5.3] Register briefing routes in `packages/daemon/src/http/router.ts` [owner:api-engineer]

## Wiring Batch

- [ ] [6.1] Import `startBriefingScheduler` in `packages/daemon/src/index.ts`; construct `BriefingDeps` from existing `db` and `agentClient` instances; call scheduler after init; register the returned cleanup function in the shutdown handler [owner:api-engineer]

## Verify

- [ ] [7.1] `pnpm --filter @nova/db build` passes — zero TypeScript errors [owner:api-engineer]
- [ ] [7.2] `pnpm --filter @nova/daemon typecheck` passes — zero TypeScript errors [owner:api-engineer]
- [ ] [7.3] `pnpm --filter @nova/db db:migrate` applies briefings migration against local Postgres — `\d briefings` shows five columns [owner:api-engineer]
- [ ] [7.4] Manual smoke: advance system clock to 07:00, confirm `runMorningBriefing` is called and a row appears in `briefings` table [owner:api-engineer]
- [ ] [7.5] `GET /api/briefing` returns `200` with a briefing row after manual trigger; returns `404` with `{ error: "no briefing available" }` when table is empty [owner:api-engineer]
