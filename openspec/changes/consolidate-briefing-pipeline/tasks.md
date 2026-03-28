# Implementation Tasks

<!-- beads:epic:nv-1wq6 -->

## Rust Retirement Batch

- [x] [0.1] [P-1] Remove `send_morning_briefing()` from `crates/nv-daemon/src/orchestrator.rs` -- replace the MorningBriefing handler in `TriggerClass::Digest` arm with a log-and-skip: `tracing::info!("morning briefing: delegated to TS daemon"); return;` [owner:api-engineer] [beads:nv-nfzx]
- [x] [0.2] [P-1] Remove `crates/nv-daemon/src/briefing_store.rs` module and all `use crate::briefing_store::*` imports across the crate (worker.rs, orchestrator.rs, http.rs, main.rs) [owner:api-engineer] [beads:nv-mypj]
- [x] [0.3] [P-1] Remove `briefing_store` field from `HttpState` in `crates/nv-daemon/src/http.rs` and from `WorkerDeps` in `crates/nv-daemon/src/worker.rs`, remove all plumbing that initializes and passes `BriefingStore` through `main.rs` [owner:api-engineer] [beads:nv-4314]
- [x] [0.4] [P-1] Remove `get_briefing_handler` and `get_briefing_history_handler` from `crates/nv-daemon/src/http.rs`, remove `.route("/api/briefing", ...)` and `.route("/api/briefing/history", ...)` from `build_router()`, remove `BriefingQuery` struct [owner:api-engineer] [beads:nv-g210]
- [x] [0.5] [P-1] Remove briefing-related test helpers (`setup_with_briefing`) and test functions from `crates/nv-daemon/src/http.rs` that reference `BriefingStore` [owner:api-engineer] [beads:nv-dys3]
- [x] [0.6] [P-1] Gate: `cargo check -p nv-daemon` passes with zero errors [owner:api-engineer] [beads:nv-jb3c]

## DB Batch

- [x] [1.1] [P-1] Add `blocks` JSONB column (nullable) to `briefings` table in `packages/db/src/schema/briefings.ts` -- `blocks: jsonb("blocks")` with no default (null when omitted). Update Briefing/NewBriefing inferred types [owner:db-engineer] [beads:nv-cwb1]
- [x] [1.2] [P-1] Run `pnpm drizzle-kit generate` in `packages/db/` to produce the migration SQL for the new column [owner:db-engineer] [beads:nv-4n85]
- [x] [1.3] [P-1] Gate: `tsc --noEmit` passes for @nova/db [owner:db-engineer] [beads:nv-heb7]

## API Batch 1: Block Types

- [x] [2.1] [P-1] Create `packages/db/src/blocks.ts` -- Zod discriminated union on `type` field for 10 block types: section, status_table, metric_card, timeline, action_group, kv_list, alert, source_pills, pr_list, pipeline_table. Each block has `type` (required), `title` (optional), and type-specific `data` object. Export BriefingBlockSchema, BriefingBlocksSchema, inferred types BriefingBlock and BriefingBlocks [owner:api-engineer] [beads:nv-jaud]
- [x] [2.2] [P-2] Add barrel export for blocks.ts in `packages/db/src/index.ts` -- export { BriefingBlockSchema, BriefingBlocksSchema, type BriefingBlock, type BriefingBlocks } [owner:api-engineer] [beads:nv-uahf]
- [x] [2.3] [P-1] Gate: `tsc --noEmit` passes for @nova/db [owner:api-engineer] [beads:nv-6mwx]

## API Batch 2: Synthesizer Update

- [x] [3.1] [P-1] Update BRIEFING_SYSTEM_PROMPT in `packages/daemon/src/features/briefing/synthesizer.ts` -- replace markdown instructions with JSON block schema definition, include examples for each of the 10 block types, instruct Claude to output a raw JSON array (no code fences) [owner:api-engineer] [beads:nv-ff2j]
- [x] [3.2] [P-1] Add JSON parsing + Zod validation to `synthesizeBriefing()` -- parse Claude response as JSON, validate with BriefingBlocksSchema from @nova/db. On parse/validation failure, log warning and fall back to `buildStaticSummary()` [owner:api-engineer] [beads:nv-mzt8]
- [x] [3.3] [P-1] Add `blocksToMarkdown()` helper in synthesizer.ts -- convert validated BriefingBlock[] to readable markdown string for Telegram delivery. Section blocks become ### headers, status_table becomes ASCII table, metric_card becomes "Label: Value (delta)", etc. [owner:api-engineer] [beads:nv-pzci]
- [x] [3.4] [P-1] Extract suggestedActions from action_group blocks -- find blocks with type "action_group", map their actions to SuggestedAction[]. Remove the old `parseSuggestedActions()` JSON code fence parser [owner:api-engineer] [beads:nv-1twf]
- [x] [3.5] [P-1] Update SynthesisResult type to include `blocks: BriefingBlock[] | null` -- set to validated block array on success, null on fallback to markdown [owner:api-engineer] [beads:nv-ga54]
- [x] [3.6] [P-1] Gate: `tsc --noEmit` passes for daemon [owner:api-engineer] [beads:nv-zlpw]

## API Batch 3: Runner + SSE + Missed Detection

- [x] [4.1] [P-1] Update `runMorningBriefing()` in `packages/daemon/src/features/briefing/runner.ts` -- add `$4` parameter for `blocks` (JSON.stringify(synthesis.blocks) or null) in the INSERT query. Update SQL to `INSERT INTO briefings (content, sources_status, suggested_actions, blocks) VALUES ($1, $2, $3, $4)` [owner:api-engineer] [beads:nv-unws]
- [x] [4.2] [P-1] Add `GET /api/briefing/stream` SSE endpoint in `packages/daemon/src/http.ts` -- use streamSSE pattern from Hono (same as POST /chat). Call gatherContext(), then stream blocks as they are synthesized. Emit `{ type: "block", index, block }` per block, `{ type: "done", blocks }` on completion, `{ type: "error", message }` on failure. After streaming, persist briefing to DB and send Telegram notification [owner:api-engineer] [beads:nv-3qs9]
- [x] [4.3] [P-2] Return 503 if briefingDeps is not configured on the stream endpoint [owner:api-engineer] [beads:nv-1mcz]
- [x] [4.4] [P-1] Add missed-briefing detection to `packages/daemon/src/features/briefing/scheduler.ts` -- after briefingHour + 1 (e.g., 08:00 if briefing is at 07:00), query DB for today's briefing count. If zero, send Telegram alert and set in-memory guard to prevent duplicate alerts. Only check once per day [owner:api-engineer] [beads:nv-oztt]
- [x] [4.5] [P-1] Gate: `tsc --noEmit` passes for daemon [owner:api-engineer] [beads:nv-l3ai]

## API Batch 4: tRPC Router Update

- [x] [5.1] [P-1] Update `mapBriefingRow()` in `packages/api/src/routers/briefing.ts` to include `blocks` field -- pass through row.blocks (JSONB parsed by Drizzle, or null) [owner:api-engineer] [beads:nv-7qda]
- [x] [5.2] [P-1] Add `missedToday` to the `latest` procedure response -- read `briefing_hour` from settings table (default 7), compute whether current time is past briefingHour + 1 and latest briefing is not from today [owner:api-engineer] [beads:nv-0tea]
- [x] [5.3] [P-1] Gate: `tsc --noEmit` passes for @nova/api [owner:api-engineer] [beads:nv-urx0]

## UI Batch 1: Block Components

- [x] [6.1] [P-1] Create `apps/dashboard/components/blocks/SectionBlock.tsx` -- render body as prose text using Geist tokens (surface-card, text-copy-14, text-ds-gray-1000). Similar layout to existing BriefingSectionCard [owner:ui-engineer] [beads:nv-qvgw]
- [x] [6.2] [P-1] Create `apps/dashboard/components/blocks/StatusTable.tsx` -- render columns as th headers, rows as tr/td cells. Use surface-card container, text-label-12 for headers, text-copy-13 for cells, ds-gray-400 borders [owner:ui-engineer] [beads:nv-kxzp]
- [x] [6.3] [P-1] Create `apps/dashboard/components/blocks/MetricCard.tsx` -- render label, large value with optional unit, trend arrow (up=ds-green-700, down=ds-red-700, flat=ds-gray-700), delta text. Use surface-card container [owner:ui-engineer] [beads:nv-f457]
- [x] [6.4] [P-1] Create `apps/dashboard/components/blocks/Timeline.tsx` -- render events as vertical timeline with time labels, severity-colored dots (info=ds-blue-700, warning=ds-amber-700, error=ds-red-700), label and optional detail text [owner:ui-engineer] [beads:nv-6v82]
- [x] [6.5] [P-1] Create `apps/dashboard/components/blocks/ActionGroup.tsx` -- render actions as chips with status-based styling (pending=ds-gray-alpha-100 border, completed=green-700/10, dismissed=ds-gray-100 line-through). Match existing suggested actions chip pattern [owner:ui-engineer] [beads:nv-6j4n]
- [x] [6.6] [P-2] Create `apps/dashboard/components/blocks/KVList.tsx` -- render items as two-column layout with key in text-label-13 text-ds-gray-700 and value in text-copy-13 text-ds-gray-1000 [owner:ui-engineer] [beads:nv-monl]
- [x] [6.7] [P-2] Create `apps/dashboard/components/blocks/AlertBlock.tsx` -- render message with severity-colored left border (info=ds-blue-700, warning=ds-amber-700, error=ds-red-700) and matching icon. Use surface-card container [owner:ui-engineer] [beads:nv-nl35]
- [x] [6.8] [P-2] Create `apps/dashboard/components/blocks/SourcePills.tsx` -- render sources as pill badges with status dot (ok=green-700, unavailable=red-700, empty=ds-gray-500). Match existing source status pill pattern from briefing page [owner:ui-engineer] [beads:nv-ibfr]
- [x] [6.9] [P-2] Create `apps/dashboard/components/blocks/PRList.tsx` -- render prs as list items with repo name badge, title text, and status indicator (open=ds-green-700, merged=ds-blue-700, closed=ds-red-700) [owner:ui-engineer] [beads:nv-781h]
- [x] [6.10] [P-2] Create `apps/dashboard/components/blocks/PipelineTable.tsx` -- render pipelines as table rows with name, status badge (success=green, failed=red, running=amber, pending=gray), and optional duration [owner:ui-engineer] [beads:nv-zm5f]
- [x] [6.11] [P-1] Create `apps/dashboard/components/blocks/BlockRegistry.tsx` -- export BlockRegistry (Record<string, ComponentType>) mapping each block type to its component. Export BriefingRenderer component that maps block array through registry, renders null for unknown types. Accept optional className prop [owner:ui-engineer] [beads:nv-wg6x]
- [x] [6.12] [P-1] Gate: `pnpm build` passes for dashboard [owner:ui-engineer] [beads:nv-czhr]

## UI Batch 2: Briefing Page Update

- [x] [7.1] [P-1] Add `blocks` field (BriefingBlock[] | null) and update BriefingGetResponse to include `missedToday` in `apps/dashboard/types/api.ts` [owner:ui-engineer] [beads:nv-g86r]
- [x] [7.2] [P-1] Update `apps/dashboard/app/briefing/page.tsx` rendering logic -- if displayEntry.blocks is non-null and non-empty, render via BriefingRenderer. Otherwise, fall back to current parseBriefingSections + ReactMarkdown path. Keep all existing UX (history rail, update banner, error handling) [owner:ui-engineer] [beads:nv-ejkm]
- [x] [7.3] [P-1] Implement streaming in `handleGenerate()` -- on "Generate Now" click, connect to `GET /api/briefing/stream` via EventSource. As "block" events arrive, append to a local blocks array and render progressively with skeleton placeholders for remaining blocks. On "done" event, close EventSource and refresh from DB. On "error" event, show error banner [owner:ui-engineer] [beads:nv-42hd]
- [x] [7.4] [P-2] Add streaming skeleton component -- show animated placeholder cards while blocks are being generated, remove each as the corresponding block arrives [owner:ui-engineer] [beads:nv-fhoj]
- [x] [7.5] [P-1] Add missed-briefing banner -- when `missedToday` is true, show a banner "No briefing generated today. Generate one now?" with a button that triggers handleGenerate(). Dismiss when briefing is created [owner:ui-engineer] [beads:nv-3ni2]
- [x] [7.6] [P-1] Gate: `pnpm build` passes for dashboard [owner:ui-engineer] [beads:nv-hv0i]

## E2E Verification

- [ ] [8.1] `cargo check -p nv-daemon` passes (Rust briefing retirement) [owner:api-engineer] [beads:nv-7y3p]
- [ ] [8.2] `tsc --noEmit` passes for @nova/db (blocks schema + migration) [owner:api-engineer] [beads:nv-y0rv]
- [ ] [8.3] `tsc --noEmit` passes for daemon (synthesizer, runner, SSE, scheduler) [owner:api-engineer] [beads:nv-vyak]
- [ ] [8.4] `pnpm build` passes for dashboard (block components, briefing page) [owner:ui-engineer] [beads:nv-kjov]
- [ ] [8.5] [user] Manual: trigger "Generate Now" from dashboard, verify blocks render progressively via SSE stream [beads:nv-sqnc]
- [ ] [8.6] [user] Manual: view an old briefing from history rail, verify markdown fallback renders correctly [beads:nv-exnb]
- [ ] [8.7] [user] Manual: verify Telegram receives markdown content (not JSON) when briefing is generated [beads:nv-z5qe]
- [ ] [8.8] [user] Manual: stop daemon before briefing hour, verify missed-briefing alert appears on Telegram and dashboard after briefingHour + 1 [beads:nv-sd06]
- [ ] [8.9] [user] Manual: verify Rust daemon no longer writes to JSONL or exposes /api/briefing endpoints [beads:nv-a95t]
- [ ] [8.10] [deferred] Archive `generative-ui-briefings` spec (subsumed by this spec) [beads:nv-becu]
