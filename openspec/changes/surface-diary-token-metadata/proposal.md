# Proposal: Surface Diary Token Metadata

## Change ID
`surface-diary-token-metadata`

## Summary

Promote hidden diary metadata (tokens, latency, tools, cost) into the collapsed row view, enhance
`toolsUsed` storage from flat string array to structured objects with input summaries and duration,
add per-entry cost estimation, and introduce daily/weekly aggregate summaries.

## Context
- Extends: `apps/dashboard/components/DiaryEntry.tsx` (collapsed row, expanded view)
- Extends: `apps/dashboard/app/diary/page.tsx` (summary bar, aggregate stats)
- Extends: `packages/db/src/schema/diary.ts` (diary table columns)
- Extends: `packages/api/src/routers/diary.ts` (tRPC diary.list procedure)
- Extends: `packages/daemon/src/features/diary/writer.ts` (DiaryWriteInput, writeEntry)
- Extends: `packages/daemon/src/brain/agent.ts` (processMessage, processMessageStream)
- Extends: `packages/validators/src/diary.ts` (Zod schemas for toolsUsed)
- Extends: `apps/dashboard/types/api.ts` (DiaryEntryItem, DiaryGetResponse)
- Related: `cleanup-diary-page` (active) -- established the compact row + expand/collapse layout this spec builds on
- Related: `add-diary-system` (archived) -- created the Postgres diary writer and tRPC reader

## Motivation

The diary page hides its most valuable data behind a click-to-expand interaction. Token usage
(tokensIn, tokensOut), response latency, and tools called are already stored in every diary row
but only visible in the expanded detail view. Users scanning the activity log cannot quickly
identify expensive interactions, slow responses, or tool-heavy sessions without clicking each entry.

Additionally, `toolsUsed` stores only tool names (`string[]`), discarding the input parameters
and per-tool duration that the Agent SDK already provides in memory during processing. This loses
context about *what* each tool did and *how long* it took.

There is no cost estimation despite the daemon knowing the model name and token counts at write
time. Users have no visibility into per-interaction or aggregate API spend from the diary page.

## Requirements

### Req-1: Surface Metadata in Collapsed Row

Show token count, response latency, and top-3 tool pills directly in the collapsed diary entry
row, without requiring expand. The collapsed row currently shows: timestamp, channel badge,
trigger type badge, truncated summary, and chevron. Add after the summary (before chevron):

- **Token badge**: compact `{in}+{out}` display with abbreviated numbers (e.g. "1.2k+340")
- **Latency badge**: `{ms}ms` or `{s}s` for values over 1000ms
- **Tool pills**: up to 3 tool name pills (ToolPill component already exists), with a `+N` overflow indicator when more than 3 tools were called
- **Cost badge**: estimated cost in USD (e.g. "$0.03") when cost data is available

These should be compact, muted, and not compete visually with the summary text. Hide on narrow
viewports (below `sm` breakpoint) to avoid horizontal overflow.

### Req-2: Enhance toolsUsed Storage Format

Change `toolsUsed` from `string[]` to a structured array of objects:

```typescript
interface ToolCallDetail {
  name: string;
  input_summary: string;   // first 120 chars of JSON.stringify(input), truncated
  duration_ms: number | null;
}
```

The diary schema column (`jsonb("tools_used")`) already supports arbitrary JSON, so no Postgres
migration is needed -- only the shape of the data written changes. The reader must handle both
the legacy `string[]` format (for existing rows) and the new `ToolCallDetail[]` format.

Capture `input_summary` by stringifying and truncating the tool input object. Capture `duration_ms`
from the existing `inflightTools` timing in the streaming path, and by adding timing to the
non-streaming path.

### Req-3: Add Model and Cost to Diary Entry

Add two new columns to the diary schema:

- `model` (`text`, nullable) -- the Claude model used (e.g. "claude-opus-4-6")
- `costUsd` (`real`, nullable) -- estimated cost in USD

The daemon writer already has access to `this.config.agent.model` at each call site. For cost
estimation, use a static pricing lookup table mapping model names to per-token rates (input/output),
computed at write time. This avoids depending on the Claude API's `total_cost_usd` field which is
not available in the Agent SDK path.

### Req-4: Per-Entry Cost Estimation

Implement a `estimateCost(model, tokensIn, tokensOut)` utility in `packages/daemon/src/features/diary/`
that returns `number | null`. Use a hardcoded pricing table:

| Model | Input ($/1M tokens) | Output ($/1M tokens) |
|-------|---------------------|----------------------|
| claude-opus-4-6 | 15.00 | 75.00 |
| claude-sonnet-4-5 | 3.00 | 15.00 |
| claude-haiku-3-5 | 0.80 | 4.00 |

Return `null` for unknown models. The table is intentionally simple and static -- it can be updated
when pricing changes without a schema migration.

### Req-5: Daily Aggregate Summary

Add an `aggregates` field to `DiaryGetResponse` computed server-side in the tRPC diary.list
procedure. The aggregates cover all entries for the requested date:

```typescript
interface DiaryAggregates {
  total_tokens_in: number;
  total_tokens_out: number;
  total_cost_usd: number | null;
  avg_latency_ms: number;
  tool_frequency: Array<{ name: string; count: number }>; // top 10, descending
}
```

Display these aggregates in the summary bar on the diary page, replacing or augmenting the existing
StatCard row with: total tokens, estimated cost, average latency, and most-used tool.

### Req-6: Capture Structured Tool Details in Daemon

Modify the three diary `writeEntry()` call sites in the TS daemon:

1. `packages/daemon/src/brain/agent.ts` (processMessage, line ~171) -- pass full `toolCalls` array
   with `input` and add timing measurement
2. `packages/daemon/src/brain/agent.ts` (processMessageStream, line ~310) -- pass `toolCalls` with
   `input` and `durationMs` from `inflightTools`
3. `packages/daemon/src/index.ts` (keyword router path, line ~593) -- pass single tool with input
   if available

Also pass `model` from `this.config.agent.model` at each call site.

## Scope
- **IN**: Collapsed row metadata display, structured toolsUsed format, model/cost columns, cost
  estimation utility, daily aggregates in tRPC response, summary bar enhancement, three daemon
  writeEntry call site updates, Zod validator updates, API type updates
- **OUT**: Weekly aggregate view (daily only for now), cost alerting/budgeting (exists separately
  in api_usage), Rust daemon diary changes (file-based diary is deprecated), tool result storage
  (only input summary, not full output), historical data backfill, per-tool token breakdown
  (not available from Agent SDK)

## Impact
| Area | Change |
|------|--------|
| `packages/db/src/schema/diary.ts` | Add `model` and `costUsd` columns |
| `packages/validators/src/diary.ts` | Update Zod schemas for new toolsUsed shape and new columns |
| `packages/daemon/src/features/diary/writer.ts` | Extend DiaryWriteInput with model, costUsd, structured toolsUsed |
| `packages/daemon/src/features/diary/pricing.ts` | New: estimateCost() utility with static pricing table |
| `packages/daemon/src/brain/agent.ts` | Pass structured tool details + model to writeEntry at both call sites |
| `packages/daemon/src/index.ts` | Pass model to writeEntry at keyword router call site |
| `packages/api/src/routers/diary.ts` | Add aggregates computation, pass model/cost in entry response |
| `apps/dashboard/types/api.ts` | Extend DiaryEntryItem with model, cost_usd; add DiaryAggregates; extend DiaryGetResponse |
| `apps/dashboard/components/DiaryEntry.tsx` | Add token/latency/tool/cost badges to collapsed row |
| `apps/dashboard/app/diary/page.tsx` | Display aggregate stats in summary bar |

## Risks
| Risk | Mitigation |
|------|-----------|
| Backward compatibility with existing `string[]` toolsUsed rows | Reader normalizes both formats: if element is string, wrap as `{name, input_summary: "", duration_ms: null}` |
| Collapsed row horizontal overflow with many badges | Hide badges below `sm` breakpoint; cap at 3 tool pills + overflow count |
| Hardcoded pricing table becomes stale | Single-file utility, easy to update; returns null for unknown models rather than wrong values |
| Cost estimation slightly inaccurate vs actual billing | Labeled as "estimated" in UI; actual billing tracked in separate api_usage table |
| Schema migration for new columns | Both columns are nullable with no default, so migration is additive-only -- zero downtime |
