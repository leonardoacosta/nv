# Proposal: Add Morning Briefing — TypeScript Port

## Change ID
`add-morning-briefing`

## Summary

Port the morning briefing / digest system from the Rust daemon to the TypeScript daemon. A
cron-style scheduler fires at 07:00 daily, a synthesizer gathers context from obligations,
memory, and recent messages, Claude generates a structured summary via the Anthropic Agent SDK
`query()` call, the result is stored in a `briefings` Postgres table, and the dashboard gains a
`GET /api/briefing` endpoint to expose it.

## Context

- Phase: 3 — Features | Wave: 8
- Stack: TypeScript (packages/daemon)
- Depends on: `add-proactive-watcher` (obligation scan infrastructure), `add-memory-system`
  (memory read API)
- Prior art: Rust implementation in `crates/nv-daemon/src/scheduler.rs`,
  `crates/nv-daemon/src/briefing_store.rs`, and `crates/nv-daemon/src/orchestrator.rs`
  (`send_morning_briefing()`); archived spec `2026-03-25-add-morning-briefing-page`
- DB layer: `packages/db` (from `setup-postgres-drizzle`)
- Agent SDK: Anthropic Agent SDK `query()` (matches the pattern established by
  `replace-anthropic-with-agent-sdk`)

## Motivation

The Rust daemon already delivers a morning briefing via Telegram at 07:00, but its synthesizer
is a static template (`format_morning_briefing`) that only counts obligations by priority. The
TypeScript port uses the full Agent SDK `query()` path so Claude actually reads the context and
writes a natural-language digest — not a formatted table. The Postgres `briefings` table makes
the briefing queryable by the dashboard and future specs without relying on the JSONL file at
`~/.nv/state/briefing-log.jsonl`.

## Proposed Changes

### 1. Briefing Schema (`packages/db/src/schema/briefings.ts`)

New Drizzle schema file:

```typescript
export const briefings = pgTable("briefings", {
  id:               uuid("id").primaryKey().defaultRandom(),
  generated_at:     timestamp("generated_at", { withTimezone: true }).notNull().defaultNow(),
  content:          text("content").notNull(),
  sources_status:   jsonb("sources_status").notNull().default({}),
  suggested_actions: jsonb("suggested_actions").notNull().default([]),
});
```

`sources_status`: `Record<string, "ok" | "unavailable" | "empty">` — one key per data source
(obligations, memory, messages).
`suggested_actions`: `Array<{ label: string; url?: string }>` — action items extracted by
Claude from the briefing content.

### 2. Scheduler (`src/features/briefing/scheduler.ts`)

Cron-like scheduler implemented with `setInterval` polling every 60 seconds:

- On each tick, reads `Date` from `chrono` (or `new Date()`) — checks `local hour === 7`.
- Tracks `lastBriefingDate: string | null` (YYYY-MM-DD) in memory; skips if already fired today.
- On fire: calls `runMorningBriefing(deps)` asynchronously (fire-and-forget with error logging).
- Exports `startBriefingScheduler(deps: BriefingDeps): () => void` — returns a cleanup
  function that clears the interval.

Implementation note: `setInterval` at 60-second cadence mirrors the Rust scheduler's 60-second
`MORNING_BRIEFING_POLL_SECS`. No external cron library needed.

### 3. Synthesizer (`src/features/briefing/synthesizer.ts`)

`gatherContext(deps)` — parallel fetch with `Promise.allSettled`:
1. **Obligations**: query `db.select().from(obligations).where(eq(status, "pending" | "in_progress"))` — top 20 by priority, then created_at.
2. **Memory**: query `db.select().from(memory).orderBy(desc(updated_at)).limit(10)` — most recently updated memory entries.
3. **Recent messages**: query `db.select().from(messages).orderBy(desc(created_at)).limit(20)` — last 20 inbound messages across all channels.

Each fetch has an independent 10-second timeout via `Promise.race([fetch, timeout])`. Partial
results are accepted — a failed source records `"unavailable"` in `sources_status`.

`synthesizeBriefing(context, agentClient)` — builds a system prompt and calls `query()`:
- System prompt instructs Claude to produce a morning briefing with sections: Obligations,
  Memory Highlights, Recent Activity, and Suggested Actions.
- Passes gathered context as user message.
- Returns `{ content: string, suggestedActions: Array<{ label: string; url?: string }> }`.
- On Agent SDK error, falls back to a static template summarising obligation counts.

### 4. Briefing Runner (`src/features/briefing/runner.ts`)

`runMorningBriefing(deps: BriefingDeps): Promise<void>`:
1. Calls `gatherContext(deps)`.
2. Calls `synthesizeBriefing(context, deps.agentClient)`.
3. Inserts result into `briefings` table via `db.insert(briefings).values(...)`.
4. Logs success with `generated_at` timestamp.
5. On error: logs the error and re-throws (caller handles).

`BriefingDeps`:
```typescript
export interface BriefingDeps {
  db: DrizzleClient;
  agentClient: AgentClient;  // Agent SDK query() wrapper
  logger: Logger;
}
```

### 5. Dashboard API Endpoint (`src/http/routes/briefing.ts`)

`GET /api/briefing`:
- Queries `db.select().from(briefings).orderBy(desc(generated_at)).limit(1)`.
- Returns `200 { id, generated_at, content, sources_status, suggested_actions }` on success.
- Returns `404 { error: "no briefing available" }` if table is empty.

`GET /api/briefing/history`:
- Accepts `?limit=N` (default 10, max 30).
- Returns `200 { entries: [...] }` — newest first.
- Empty array is a valid response (no 404).

Router registration in `src/http/router.ts`.

### 6. Wiring (`src/index.ts`)

Import `startBriefingScheduler` and call it after DB and Agent SDK client are initialised.
Pass the cleanup function to the shutdown handler alongside other interval cleanups.

## Scope

**IN**: `packages/db/src/schema/briefings.ts`, Drizzle migration, `src/features/briefing/`
(scheduler.ts, synthesizer.ts, runner.ts), `src/http/routes/briefing.ts`, router registration,
wiring in `src/index.ts`.

**OUT**: Telegram delivery of briefing (remains in Rust daemon until full TS migration),
dashboard frontend page (covered by archived `add-morning-briefing-page` spec which targets the
existing React dashboard), action dismissal/completion via API, configurable briefing hour via
API, per-section source attribution beyond `sources_status`.

## Impact

| Area | Change |
|------|--------|
| `packages/db/src/schema/briefings.ts` | New: briefings table schema |
| `packages/db/drizzle/` | New: migration adding briefings table |
| `packages/db/src/index.ts` | Re-export briefings table |
| `packages/daemon/src/features/briefing/scheduler.ts` | New: 60s poll scheduler, fires at 07:00 |
| `packages/daemon/src/features/briefing/synthesizer.ts` | New: context gather + Agent SDK synthesis |
| `packages/daemon/src/features/briefing/runner.ts` | New: orchestrate gather → synthesize → persist |
| `packages/daemon/src/http/routes/briefing.ts` | New: GET /api/briefing + GET /api/briefing/history |
| `packages/daemon/src/http/router.ts` | Register briefing routes |
| `packages/daemon/src/index.ts` | Start scheduler, wire BriefingDeps, register shutdown cleanup |

## Risks

| Risk | Mitigation |
|------|-----------|
| Agent SDK `query()` timeout on slow mornings | 30-second overall timeout on `synthesizeBriefing`; static fallback template on timeout/error |
| `add-proactive-watcher` not yet applied | Synthesizer's obligation query is a direct DB select — no runtime dependency on the watcher module. The `add-proactive-watcher` dependency is for the obligation data being present, not the module API. |
| `add-memory-system` not yet applied | Memory query is a direct DB select — same as above. If the memory table is empty, `sources_status.memory = "empty"` and the briefing continues. |
| Briefings table grows unbounded | Add a nightly cleanup task (separate spec) or a cap in `runMorningBriefing` that deletes rows older than 90 days after each insert. |
| Briefing fires at 07:00 local time vs UTC | Use `new Date()` with `getHours()` — Node.js `getHours()` returns local hours when the system timezone is set correctly. Document that the server timezone must match the user's locale. |
