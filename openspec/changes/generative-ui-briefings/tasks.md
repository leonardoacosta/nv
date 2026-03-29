# Implementation Tasks

<!-- beads:epic:nv-ckdj -->

## DB Batch

- [x] [1.1] [P-1] Add `blocks` JSONB column (nullable) to `briefings` table in packages/db/src/schema/briefings.ts -- `blocks: jsonb("blocks")` with no default (null when omitted). Update Briefing/NewBriefing inferred types [owner:db-engineer] [beads:nv-ya2l]
- [x] [1.2] [P-1] Run `pnpm drizzle-kit generate` in packages/db/ to produce the migration SQL for the new column [owner:db-engineer] [beads:nv-py5q]
- [x] [1.3] [P-1] Gate: `tsc --noEmit` passes for @nova/db [owner:db-engineer] [beads:nv-fx3u]

## API Batch 1: Block Types

- [x] [2.1] [P-1] Create packages/db/src/blocks.ts -- Zod discriminated union on `type` field for 10 block types: section, status_table, metric_card, timeline, action_group, kv_list, alert, source_pills, pr_list, pipeline_table. Each block has `type` (required), `title` (optional), and type-specific `data` object. Export BriefingBlockSchema (single block), BriefingBlocksSchema (z.array), inferred types BriefingBlock and BriefingBlocks [owner:api-engineer] [beads:nv-gkh1]
- [x] [2.2] [P-2] Add barrel export for blocks.ts in packages/db/src/index.ts -- export { BriefingBlockSchema, BriefingBlocksSchema, type BriefingBlock, type BriefingBlocks } [owner:api-engineer] [beads:nv-8g0d]
- [x] [2.3] [P-1] Gate: `tsc --noEmit` passes for @nova/db [owner:api-engineer] [beads:nv-ktvi]

## API Batch 2: Synthesizer Update

- [x] [3.1] [P-1] Update BRIEFING_SYSTEM_PROMPT in packages/daemon/src/features/briefing/synthesizer.ts -- replace markdown instructions with JSON block schema definition, include examples for each of the 10 block types, instruct Claude to output a raw JSON array (no code fences) [owner:api-engineer] [beads:nv-q85s]
- [x] [3.2] [P-1] Add JSON parsing + Zod validation to synthesizeBriefing() -- parse Claude response as JSON, validate with BriefingBlocksSchema from @nova/db. On parse/validation failure, log warning and fall back to buildStaticSummary() [owner:api-engineer] [beads:nv-csjk]
- [x] [3.3] [P-1] Add blocksToMarkdown() helper in synthesizer.ts -- convert validated BriefingBlock[] to readable markdown string for Telegram delivery. Section blocks become ### headers, status_table becomes ASCII table, metric_card becomes "Label: Value (delta)", etc. [owner:api-engineer] [beads:nv-qxsc]
- [x] [3.4] [P-1] Extract suggestedActions from action_group blocks -- find blocks with type "action_group", map their actions to SuggestedAction[]. Remove the old parseSuggestedActions() JSON code fence parser [owner:api-engineer] [beads:nv-b43m]
- [x] [3.5] [P-1] Update SynthesisResult type to include `blocks: BriefingBlock[] | null` -- set to validated block array on success, null on fallback to markdown [owner:api-engineer] [beads:nv-ivg7]
- [x] [3.6] [P-1] Gate: `tsc --noEmit` passes for daemon [owner:api-engineer] [beads:nv-wpn3]

## API Batch 3: Runner + SSE Streaming

- [x] [4.1] [P-1] Update runMorningBriefing() in packages/daemon/src/features/briefing/runner.ts -- add `$4` parameter for `blocks` (JSON.stringify(synthesis.blocks) or null) in the INSERT query. Update SQL to `INSERT INTO briefings (content, sources_status, suggested_actions, blocks) VALUES ($1, $2, $3, $4)` [owner:api-engineer] [beads:nv-yd5b]
- [x] [4.2] [P-1] Add GET /api/briefing/stream SSE endpoint in packages/daemon/src/http.ts -- use streamSSE pattern from Hono (same as POST /chat). Call gatherContext(), then stream blocks as they are synthesized. Emit `{ type: "block", index, block }` per block, `{ type: "done", blocks }` on completion, `{ type: "error", message }` on failure. After streaming, persist briefing to DB and send Telegram notification [owner:api-engineer] [beads:nv-b576]
- [x] [4.3] [P-2] Return 503 if briefingDeps is not configured on the stream endpoint [owner:api-engineer] [beads:nv-qbzc]
- [x] [4.4] [P-1] Gate: `tsc --noEmit` passes for daemon [owner:api-engineer] [beads:nv-a1ut]

## UI Batch 1: Block Components

- [x] [5.1] [P-1] Create apps/dashboard/components/blocks/SectionBlock.tsx -- render body as prose text using Geist tokens (surface-card, text-copy-14, text-ds-gray-1000). Similar layout to existing BriefingSectionCard [owner:ui-engineer] [beads:nv-d8ls]
- [x] [5.2] [P-1] Create apps/dashboard/components/blocks/StatusTable.tsx -- render columns as th headers, rows as tr/td cells. Use surface-card container, text-label-12 for headers, text-copy-13 for cells, ds-gray-400 borders [owner:ui-engineer] [beads:nv-kylc]
- [x] [5.3] [P-1] Create apps/dashboard/components/blocks/MetricCard.tsx -- render label, large value with optional unit, trend arrow (up=ds-green-700, down=ds-red-700, flat=ds-gray-700), delta text. Use surface-card container [owner:ui-engineer] [beads:nv-410a]
- [x] [5.4] [P-1] Create apps/dashboard/components/blocks/Timeline.tsx -- render events as vertical timeline with time labels, severity-colored dots (info=ds-blue-700, warning=ds-amber-700, error=ds-red-700), label and optional detail text [owner:ui-engineer] [beads:nv-szij]
- [x] [5.5] [P-1] Create apps/dashboard/components/blocks/ActionGroup.tsx -- render actions as chips with status-based styling (pending=ds-gray-alpha-100 border, completed=green-700/10, dismissed=ds-gray-100 line-through). Match existing suggested actions chip pattern [owner:ui-engineer] [beads:nv-p6v0]
- [x] [5.6] [P-2] Create apps/dashboard/components/blocks/KVList.tsx -- render items as two-column layout with key in text-label-13 text-ds-gray-700 and value in text-copy-13 text-ds-gray-1000 [owner:ui-engineer] [beads:nv-ls6x]
- [x] [5.7] [P-2] Create apps/dashboard/components/blocks/AlertBlock.tsx -- render message with severity-colored left border (info=ds-blue-700, warning=ds-amber-700, error=ds-red-700) and matching icon. Use surface-card container [owner:ui-engineer] [beads:nv-v8dq]
- [x] [5.8] [P-2] Create apps/dashboard/components/blocks/SourcePills.tsx -- render sources as pill badges with status dot (ok=green-700, unavailable=red-700, empty=ds-gray-500). Match existing source status pill pattern from briefing page [owner:ui-engineer] [beads:nv-vlnv]
- [x] [5.9] [P-2] Create apps/dashboard/components/blocks/PRList.tsx -- render prs as list items with repo name badge, title text, and status indicator (open=ds-green-700, merged=ds-blue-700, closed=ds-red-700) [owner:ui-engineer] [beads:nv-uk5t]
- [x] [5.10] [P-2] Create apps/dashboard/components/blocks/PipelineTable.tsx -- render pipelines as table rows with name, status badge (success=green, failed=red, running=amber, pending=gray), and optional duration [owner:ui-engineer] [beads:nv-v531]
- [x] [5.11] [P-1] Create apps/dashboard/components/blocks/BlockRegistry.tsx -- export BlockRegistry (Record<string, ComponentType>) mapping each block type to its component. Export BriefingRenderer component that maps block array through registry, renders null for unknown types. Accept optional className prop [owner:ui-engineer] [beads:nv-6wec]
- [x] [5.12] [P-1] Gate: `pnpm build` passes for dashboard [owner:ui-engineer] [beads:nv-6qxy]

## UI Batch 2: Briefing Page Update

- [x] [6.1] [P-1] Add `blocks` field to BriefingEntry in apps/dashboard/types/api.ts -- `blocks: BriefingBlock[] | null` (import BriefingBlock from @nova/db or re-declare matching type) [owner:ui-engineer] [beads:nv-9336]
- [x] [6.2] [P-1] Update apps/dashboard/app/briefing/page.tsx rendering logic -- if displayEntry.blocks is non-null and non-empty, render via BriefingRenderer. Otherwise, fall back to current parseBriefingSections + ReactMarkdown path. Keep all existing UX (history rail, update banner, error handling) [owner:ui-engineer] [beads:nv-yscp]
- [x] [6.3] [P-1] Implement streaming in handleGenerate() -- on "Generate Now" click, connect to GET /api/briefing/stream via EventSource. As "block" events arrive, append to a local blocks array and render progressively with skeleton placeholders for remaining blocks. On "done" event, close EventSource and refresh from DB. On "error" event, show error banner [owner:ui-engineer] [beads:nv-1f81]
- [x] [6.4] [P-2] Add streaming skeleton component -- show animated placeholder cards while blocks are being generated, remove each as the corresponding block arrives [owner:ui-engineer] [beads:nv-5olf]
- [x] [6.5] [P-1] Gate: `pnpm build` passes for dashboard [owner:ui-engineer] [beads:nv-dc50]

## E2E Verification

- [x] [7.1] `tsc --noEmit` passes for @nova/db (blocks schema) [owner:api-engineer] [beads:nv-llt0]
- [x] [7.2] `tsc --noEmit` passes for daemon (synthesizer, runner, SSE endpoint) [owner:api-engineer] [beads:nv-77tk]
- [x] [7.3] `pnpm build` passes for dashboard (block components, briefing page) — TypeScript compiled successfully; `next build` fails only at page-data collection due to pre-existing missing DATABASE_URL env var in CI environment (not a TS error) [owner:ui-engineer] [beads:nv-rjek]
- [ ] [7.4] [user] Manual: trigger "Generate Now" from dashboard, verify blocks render progressively via SSE stream [beads:nv-zr15]
- [ ] [7.5] [user] Manual: view an old briefing from history rail, verify markdown fallback renders correctly [beads:nv-4gwy]
- [ ] [7.6] [user] Manual: verify Telegram receives markdown content (not JSON) when briefing is generated [beads:nv-dmu8]
- [ ] [7.7] [user] Manual: kill daemon mid-stream, verify dashboard shows error banner and recovers on retry [beads:nv-y6wg]
