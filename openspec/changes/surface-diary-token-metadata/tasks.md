# Implementation Tasks

<!-- beads:epic:nv-75rh -->

## DB Batch

- [ ] [1.1] [P-1] Add `model` (text, nullable) and `cost_usd` (real, nullable) columns to diary schema in `packages/db/src/schema/diary.ts` [owner:db-engineer] [beads:nv-iw6w]
- [ ] [1.2] [P-2] Generate Drizzle migration for the two new diary columns via `pnpm drizzle-kit generate` [owner:db-engineer] [beads:nv-j1o0]

## API Batch

- [ ] [2.1] [P-1] Create `packages/daemon/src/features/diary/pricing.ts` with `estimateCost(model, tokensIn, tokensOut)` returning `number | null` using static pricing table for opus-4-6, sonnet-4-5, haiku-3-5 [owner:api-engineer] [beads:nv-srcp]
- [ ] [2.2] [P-1] Extend `DiaryWriteInput` in `packages/daemon/src/features/diary/writer.ts` with `model?: string`, `costUsd?: number`, and change `toolsUsed` from `string[]` to `Array<{name: string; input_summary: string; duration_ms: number | null}> | string[]` [owner:api-engineer] [beads:nv-oa14]
- [ ] [2.3] [P-1] Update `writeEntry()` in `packages/daemon/src/features/diary/writer.ts` to persist `model` and `costUsd` columns [owner:api-engineer] [beads:nv-2pjc]
- [ ] [2.4] [P-1] Update Zod schemas in `packages/validators/src/diary.ts` to accept both legacy `string[]` and new `ToolCallDetail[]` shapes for `toolsUsed`, plus `model` and `costUsd` [owner:api-engineer] [beads:nv-8amp]
- [ ] [2.5] [P-2] Update `processMessage()` in `packages/daemon/src/brain/agent.ts` (~line 165) to pass structured `toolCalls` with `{name, input_summary, duration_ms}`, `model`, and `costUsd` to `writeEntry()` -- add per-tool timing to non-streaming path [owner:api-engineer] [beads:nv-6c8z]
- [ ] [2.6] [P-2] Update `processMessageStream()` in `packages/daemon/src/brain/agent.ts` (~line 304) to pass structured `toolCalls` with `{name, input_summary, duration_ms}` from `inflightTools`, `model`, and `costUsd` to `writeEntry()` [owner:api-engineer] [beads:nv-hjth]
- [ ] [2.7] [P-2] Update keyword router diary write in `packages/daemon/src/index.ts` (~line 593) to pass `model` and structured tool details [owner:api-engineer] [beads:nv-nypw]
- [ ] [2.8] [P-2] Add normalizer in tRPC `diary.list` procedure (`packages/api/src/routers/diary.ts`) to handle both legacy `string[]` and new `ToolCallDetail[]` rows, emitting a unified shape in the response [owner:api-engineer] [beads:nv-2h2n]
- [ ] [2.9] [P-2] Add aggregate computation to tRPC `diary.list` procedure: `total_tokens_in`, `total_tokens_out`, `total_cost_usd`, `avg_latency_ms`, `tool_frequency` (top 10) [owner:api-engineer] [beads:nv-w7a6]
- [ ] [2.10] [P-2] Extend `DiaryEntryItem` in `apps/dashboard/types/api.ts` with `model`, `cost_usd`, and `tools_detail` fields; add `DiaryAggregates` interface; extend `DiaryGetResponse` with `aggregates` [owner:api-engineer] [beads:nv-4uct]

## UI Batch

- [ ] [3.1] [P-1] Add compact token badge (`1.2k+340`), latency badge, cost badge, and up to 3 tool pills with `+N` overflow to collapsed row in `DiaryEntry.tsx` [owner:ui-engineer] [beads:nv-hjch]
- [ ] [3.2] [P-1] Add responsive hiding of metadata badges below `sm` breakpoint in `DiaryEntry.tsx` [owner:ui-engineer] [beads:nv-ay89]
- [ ] [3.3] [P-2] Update expanded view in `DiaryEntry.tsx` to show structured tool details (name, input summary, duration) instead of plain tool name pills [owner:ui-engineer] [beads:nv-s5fi]
- [ ] [3.4] [P-2] Add aggregate summary stats (total tokens, estimated cost, avg latency, top tool) to diary page summary bar in `page.tsx` [owner:ui-engineer] [beads:nv-gvus]

## E2E Batch

- [ ] [4.1] Verify diary page renders collapsed row metadata (token badge, latency, tool pills) for an entry with tools and tokens [owner:e2e-engineer] [beads:nv-khka]
- [ ] [4.2] Verify expanded view shows structured tool detail (input summary, duration) when new-format entry exists [owner:e2e-engineer] [beads:nv-1tq5]
- [ ] [4.3] Verify aggregate stats bar displays total tokens and estimated cost for the selected date [owner:e2e-engineer] [beads:nv-6dzo]
