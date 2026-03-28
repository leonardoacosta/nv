# Proposal: Consolidate Briefing Pipeline

## Change ID
`consolidate-briefing-pipeline`

## Summary

Unify the dual briefing systems (Rust daemon JSONL + TS daemon Postgres) into a single TS daemon writer, retire the Rust daemon's briefing path, add missed-briefing detection, enhance briefing content with structured sections, and upgrade the dashboard to render briefings as interactive generative-UI blocks instead of parsed markdown cards. Subsumes the existing `generative-ui-briefings` spec.

## Context
- Extends: `packages/daemon/src/features/briefing/` (synthesizer.ts, runner.ts, scheduler.ts)
- Extends: `packages/daemon/src/http.ts` (Hono SSE server)
- Extends: `packages/db/src/schema/briefings.ts` (briefings table)
- Extends: `packages/api/src/routers/briefing.ts` (tRPC router)
- Extends: `apps/dashboard/app/briefing/page.tsx` (rendering)
- Extends: `apps/dashboard/types/api.ts` (BriefingEntry type)
- Removes: `crates/nv-daemon/src/briefing_store.rs` (JSONL storage)
- Removes: `crates/nv-daemon/src/orchestrator.rs` `send_morning_briefing()` (Rust briefing writer)
- Removes: `crates/nv-daemon/src/http.rs` GET `/api/briefing`, GET `/api/briefing/history` (Rust HTTP endpoints)
- Related: `generative-ui-briefings` (0/40 tasks, subsumed by this spec -- to be archived)
- Related: `add-briefing-cron` (completed -- original TS briefing pipeline)
- Related: `fix-briefing-crash` (completed -- error boundary + null guards)

## Motivation

Two independent briefing systems exist and cause three user-visible problems:

1. **Invisible briefings.** The Rust daemon fires at `briefing_hour`, generates a simple obligation-only summary, and writes it to `~/.nv/state/briefing-log.jsonl`. The dashboard reads from Postgres. Only TS daemon briefings appear on the dashboard.

2. **Silent failures.** If neither daemon runs at the configured hour, no briefing is generated and nothing shows for that day. There is no alerting or recovery mechanism.

3. **Flat content.** The TS daemon synthesizes markdown via Claude, stores it as a `content` text column, and the dashboard parses `###` headers into section cards via `parseBriefingSections()`. This means no structured data (obligations, calendar events, PRs are flattened into prose), inconsistent layout between runs, no progressive rendering during generation, and no semantic types (a critical alert looks identical to a memory highlight).

This spec solves all three by: (a) retiring the Rust daemon's briefing path entirely so there is exactly one writer, (b) adding missed-briefing detection with alerting, and (c) upgrading to a typed block schema with generative UI rendering.

## Requirements

### Req-1: Retire Rust Daemon Briefing Path

Remove the Rust daemon's briefing writer so the TS daemon is the sole authority:

1. Remove `send_morning_briefing()` from `crates/nv-daemon/src/orchestrator.rs` and its callsite in the `TriggerClass::Digest` match arm. The `MorningBriefing` cron event should be a no-op in the Rust daemon (log and skip, do not delete the cron event type since the scheduler still emits it and other code references it).
2. Remove `crates/nv-daemon/src/briefing_store.rs` (the JSONL store module).
3. Remove `GET /api/briefing` and `GET /api/briefing/history` from `crates/nv-daemon/src/http.rs` (the axum briefing endpoints that read from JSONL).
4. Remove the `briefing_store` field from `HttpState` and `WorkerDeps` structs, and all plumbing that passes `BriefingStore` through `main.rs` and `worker.rs`.
5. Keep the Rust scheduler's `MorningBriefing` event emission intact -- the TS daemon's own scheduler is the one that fires the actual briefing. The Rust scheduler can continue emitting the event for backward compatibility (it drives the digest gather + Telegram confetti effect) but must not write to JSONL.

### Req-2: Schema Migration -- Add `blocks` JSONB Column

Add a nullable `blocks` JSONB column to the `briefings` table in `packages/db/src/schema/briefings.ts`. The existing `content` text column remains unchanged for Telegram plain-text delivery and backward compatibility with old briefings. Generate the Drizzle migration via `pnpm drizzle-kit generate`.

### Req-3: Block Type Definitions in @nova/db

Create `packages/db/src/blocks.ts` exporting:

- A Zod discriminated union schema (`BriefingBlockSchema`) on the `type` field, covering 10 block types: `section`, `status_table`, `metric_card`, `timeline`, `action_group`, `kv_list`, `alert`, `source_pills`, `pr_list`, `pipeline_table`
- Each block has `type` (required), `title` (optional string), and a type-specific `data` object
- A Zod array schema (`BriefingBlocksSchema`) for validating the full block array
- Inferred TypeScript types: `BriefingBlock`, `BriefingBlocks`
- Barrel export from `packages/db/src/index.ts`

Block type details:

| Type | Data shape |
|------|-----------|
| `section` | `{ body: string }` |
| `status_table` | `{ columns: string[]; rows: Record<string, string>[] }` |
| `metric_card` | `{ label: string; value: string \| number; unit?: string; trend?: "up" \| "down" \| "flat"; delta?: string }` |
| `timeline` | `{ events: { time: string; label: string; detail?: string; severity?: "info" \| "warning" \| "error" }[] }` |
| `action_group` | `{ actions: { label: string; url?: string; status?: "pending" \| "completed" \| "dismissed" }[] }` |
| `kv_list` | `{ items: { key: string; value: string }[] }` |
| `alert` | `{ severity: "info" \| "warning" \| "error"; message: string }` |
| `source_pills` | `{ sources: { name: string; status: "ok" \| "unavailable" \| "empty" }[] }` |
| `pr_list` | `{ prs: { title: string; repo: string; url?: string; status: "open" \| "merged" \| "closed" }[] }` |
| `pipeline_table` | `{ pipelines: { name: string; status: "success" \| "failed" \| "running" \| "pending"; duration?: string }[] }` |

### Req-4: Synthesizer -- JSON Block Output

Update `packages/daemon/src/features/briefing/synthesizer.ts`:

- Replace `BRIEFING_SYSTEM_PROMPT` to instruct Claude to output a JSON array of typed blocks instead of markdown. The prompt must define the block schema and provide examples for each block type.
- Parse the Claude response as JSON and validate with `BriefingBlocksSchema` from `@nova/db`.
- If JSON parsing or Zod validation fails, fall back to the current markdown synthesis path (call `buildStaticSummary`).
- Generate a markdown `content` string from the validated blocks for Telegram delivery via a new `blocksToMarkdown()` helper.
- Update `SynthesisResult` to include `blocks: BriefingBlock[] | null` alongside existing `content` and `suggestedActions`.
- Extract `suggestedActions` from `action_group` blocks instead of parsing a JSON code fence.

### Req-5: Runner -- Persist Blocks

Update `packages/daemon/src/features/briefing/runner.ts`:

- Pass `synthesis.blocks` to the `INSERT INTO briefings` query as the `blocks` column value (`JSON.stringify` or `null`).
- No other changes needed -- Telegram delivery continues to use `synthesis.content` (markdown).

### Req-6: SSE Streaming Endpoint

Add `GET /api/briefing/stream` to `packages/daemon/src/http.ts`:

- Use the same `streamSSE` pattern from Hono as the existing `POST /chat` endpoint.
- Stream individual blocks as they are generated: `{ type: "block", index: number, block: BriefingBlock }` events.
- Send a final `{ type: "done", blocks: BriefingBlock[] }` event with the complete array.
- Persist the completed briefing to the database after streaming finishes.
- Send Telegram notification after persistence (same as current runner).
- Error events: `{ type: "error", message: string }` on failure.
- If `briefingDeps` is not configured, return 503.

### Req-7: Missed-Briefing Detection

Add a missed-briefing checker to the TS daemon scheduler (`packages/daemon/src/features/briefing/scheduler.ts`):

- After `briefingHour + 1` (i.e., at configured hour + 60 minutes), if no briefing row exists in the `briefings` table for today, trigger an alert.
- Alert via Telegram: send a message like "No morning briefing was generated today. The daemon may have been offline at {briefingHour}:00. Use the dashboard 'Generate Now' button to create one."
- Alert via dashboard: the tRPC `briefing.latest` procedure should include a `missedToday: boolean` field when the latest briefing is not from today and current time is past `briefingHour + 1`. The dashboard shows a banner: "No briefing generated today. Generate one now?"
- Only alert once per day (track with an in-memory guard, DB check is the source of truth).

### Req-8: Block Components

Create `apps/dashboard/components/blocks/` with one React component per block type:

- `StatusTable.tsx` -- renders `columns` as `<th>` and `rows` as `<tr>` cells
- `MetricCard.tsx` -- renders `label`, `value` with optional `unit`, `trend` arrow indicator, and `delta` text
- `Timeline.tsx` -- renders `events` as a vertical timeline with severity color indicators
- `ActionGroup.tsx` -- renders `actions` as clickable chips with status-based styling (same chip pattern as current suggested actions)
- `SectionBlock.tsx` -- renders `body` as prose text (replaces `BriefingSectionCard` for block mode)
- `KVList.tsx` -- renders `items` as a two-column key-value layout
- `AlertBlock.tsx` -- renders `message` with a severity-colored left border and icon
- `SourcePills.tsx` -- renders `sources` as pill badges with status dot colors (same pattern as current source status pills)
- `PRList.tsx` -- renders `prs` as a list with repo badge, title, and status indicator
- `PipelineTable.tsx` -- renders `pipelines` as a table with status badge and duration

All components use Geist dark theme tokens (`surface-card`, `ds-gray-*`, `text-heading-*`, `text-copy-*`, `text-label-*`). No cosmic/purple theme colors.

Create `apps/dashboard/components/blocks/BlockRegistry.tsx`:
- Export `BlockRegistry`: a `Record<string, React.ComponentType<{ block: BriefingBlock }>>` mapping block types to components
- Export `BriefingRenderer`: a component that takes `blocks: BriefingBlock[]` and renders each through the registry
- Unknown block types render `null` (forward-compatible)

### Req-9: Briefing Page -- Block Rendering + Streaming

Update `apps/dashboard/app/briefing/page.tsx`:

- If `displayEntry` has a `blocks` array (non-null, non-empty), render via `BriefingRenderer` instead of `parseBriefingSections` + `ReactMarkdown`.
- If `displayEntry` only has `content` (old data, `blocks` is null), fall back to the current rendering path.
- "Generate Now" button: when clicked, connect to `GET /api/briefing/stream` via `EventSource` and render blocks progressively as they arrive, with skeleton placeholders for remaining blocks.
- Show missed-briefing banner when `missedToday` is true from the tRPC response.
- Keep all existing UX: history navigation, polling, update banner, error handling, source status pills.
- Add a `BriefingEntry.blocks` field to the dashboard type (`apps/dashboard/types/api.ts`).

### Req-10: tRPC Router -- Expose Blocks and Missed Status

Update `packages/api/src/routers/briefing.ts`:

- Include `blocks` field in `mapBriefingRow()` -- pass through the JSONB column value (parsed from DB, or null).
- Add `missedToday` boolean to the `latest` procedure response: true when the latest briefing's `generated_at` is not today and current server time is past the configured briefing hour + 1.
- Read `briefing_hour` from the `settings` table (same as the Rust scheduler does) with a default of 7.

## Scope
- **IN**: Rust daemon briefing retirement (JSONL store, orchestrator briefing writer, axum endpoints), schema migration (`blocks` column), block types in `@nova/db`, synthesizer JSON output with Zod validation, runner blocks persistence, SSE streaming endpoint, missed-briefing detection (Telegram + dashboard), 10 block components, block registry + renderer, briefing page block rendering + streaming + missed-briefing banner, tRPC router blocks + missedToday, dashboard type update
- **OUT**: Telegram rich rendering (stays markdown via `content` column), block editor UI, user-customizable block ordering, block-level caching, block versioning, interactive block actions (e.g., dismissing actions from the dashboard), adding new block types beyond the initial 10, Rust daemon `MorningBriefing` cron event removal (kept for digest/confetti), configurable briefing hour UI (exists in add-briefing-cron spec)

## Impact

| Area | Change |
|------|--------|
| `crates/nv-daemon/src/briefing_store.rs` | REMOVE -- JSONL store no longer needed |
| `crates/nv-daemon/src/orchestrator.rs` | MODIFY -- remove `send_morning_briefing()`, skip MorningBriefing handler |
| `crates/nv-daemon/src/http.rs` | MODIFY -- remove GET `/api/briefing` and `/api/briefing/history` endpoints |
| `crates/nv-daemon/src/worker.rs` | MODIFY -- remove `briefing_store` from `WorkerDeps` |
| `crates/nv-daemon/src/main.rs` | MODIFY -- remove `BriefingStore` init and plumbing |
| `packages/db/src/schema/briefings.ts` | MODIFY -- add `blocks` JSONB column (nullable) |
| `packages/db/src/blocks.ts` | NEW -- Zod schemas + TypeScript types for 10 block types |
| `packages/db/src/index.ts` | MODIFY -- barrel export blocks |
| `packages/db/drizzle/` | NEW -- migration SQL (generated by drizzle-kit) |
| `packages/daemon/src/features/briefing/synthesizer.ts` | MODIFY -- JSON prompt, Zod validation, blocksToMarkdown, blocks in SynthesisResult |
| `packages/daemon/src/features/briefing/runner.ts` | MODIFY -- persist blocks column |
| `packages/daemon/src/features/briefing/scheduler.ts` | MODIFY -- add missed-briefing detection after briefingHour + 1 |
| `packages/daemon/src/http.ts` | MODIFY -- add `GET /api/briefing/stream` SSE endpoint |
| `packages/api/src/routers/briefing.ts` | MODIFY -- expose blocks, add missedToday |
| `apps/dashboard/components/blocks/StatusTable.tsx` | NEW |
| `apps/dashboard/components/blocks/MetricCard.tsx` | NEW |
| `apps/dashboard/components/blocks/Timeline.tsx` | NEW |
| `apps/dashboard/components/blocks/ActionGroup.tsx` | NEW |
| `apps/dashboard/components/blocks/SectionBlock.tsx` | NEW |
| `apps/dashboard/components/blocks/KVList.tsx` | NEW |
| `apps/dashboard/components/blocks/AlertBlock.tsx` | NEW |
| `apps/dashboard/components/blocks/SourcePills.tsx` | NEW |
| `apps/dashboard/components/blocks/PRList.tsx` | NEW |
| `apps/dashboard/components/blocks/PipelineTable.tsx` | NEW |
| `apps/dashboard/components/blocks/BlockRegistry.tsx` | NEW -- registry map + BriefingRenderer |
| `apps/dashboard/app/briefing/page.tsx` | MODIFY -- block rendering, SSE streaming, missed-briefing banner |
| `apps/dashboard/types/api.ts` | MODIFY -- add `blocks` and `missedToday` to BriefingEntry/response |
| `apps/dashboard/lib/briefing.ts` | UNCHANGED -- kept for backward compat with old briefings |

## Risks

| Risk | Mitigation |
|------|-----------|
| Rust daemon code removal breaks compilation | Changes are purely subtractive -- remove module, remove references, remove route registration. Compile with `cargo check` after each removal step. |
| Claude outputs malformed JSON or wrong block types | Zod validation catches all errors. On failure, fall back to current markdown synthesis. The synthesizer never crashes -- it degrades gracefully. |
| Block schema changes break old stored data | `blocks` column is nullable. Old briefings have `blocks: null` and render via the markdown path. New schema changes are additive (new block types render null via the registry). |
| SSE streaming adds complexity to the HTTP server | Pattern already exists for `POST /chat`. The briefing stream is simpler (no conversation history, no tool calls). Reuses Hono `streamSSE` helper. |
| Missed-briefing detection fires false alerts | Only fires after `briefingHour + 1`, only once per day, and only if no row exists for today. DB is the source of truth, not in-memory state. |
| Dashboard renders incorrectly with mixed old/new briefings | Explicit branching: if `blocks` exists and is non-empty, use block renderer. Otherwise, fall back to markdown. History navigation works the same way per-entry. |
| Removing Rust HTTP briefing endpoints breaks external consumers | The dashboard reads from tRPC (Postgres), not from the Rust HTTP server. No known external consumer of the Rust briefing API. |
