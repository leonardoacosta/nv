# Proposal: Generative UI Briefings

## Change ID
`generative-ui-briefings`

## Summary

Replace raw markdown briefings with a typed block schema and component registry. The daemon synthesizer outputs structured JSON blocks (validated by Zod) stored in a new `blocks` JSONB column alongside the existing `content` text. The dashboard renders blocks through a registry of purpose-built components, with SSE streaming for progressive block-by-block display. Backward compatible: old briefings without `blocks` fall back to the current markdown parser.

## Context
- Extends: `packages/db/src/schema/briefings.ts` (briefings table), `packages/daemon/src/features/briefing/synthesizer.ts` (Claude synthesis), `packages/daemon/src/features/briefing/runner.ts` (orchestration), `packages/daemon/src/http.ts` (Hono SSE server), `apps/dashboard/app/briefing/page.tsx` (rendering), `apps/dashboard/lib/briefing.ts` (section parser)
- Related: `add-morning-briefing` (original briefing system -- this spec upgrades its output format)
- Depends on: nothing new -- all existing packages already in place

## Motivation

The current briefing pipeline synthesizes markdown text via Claude, stores it as a `content` text column, and renders it with `ReactMarkdown` after parsing `###` headers into section cards. This has several limitations:

1. **No structured data** -- obligations, calendar events, and PR statuses are flattened into prose. The dashboard cannot highlight, filter, or style them differently from surrounding text.
2. **Inconsistent layout** -- Claude's markdown output varies between runs. Section ordering, heading levels, and list formatting are nondeterministic, making the briefing page visually unpredictable.
3. **No progressive rendering** -- the dashboard waits for the full synthesis to complete before displaying anything. Generation takes 10-30 seconds with no feedback beyond a spinner.
4. **No semantic types** -- the dashboard treats all content identically. A critical alert looks the same as a memory highlight. There is no way to render a status table, metric card, or timeline without custom markdown heuristics.

A typed block schema solves all four: each block has a known `type`, a validated `data` shape, and a dedicated React component. SSE streaming delivers blocks incrementally so the user sees content appear progressively.

## Requirements

### Req-1: Schema Migration -- Add `blocks` JSONB Column

Add a nullable `blocks` JSONB column to the `briefings` table in `packages/db/src/schema/briefings.ts`. The existing `content` text column remains unchanged for Telegram plain-text delivery. Generate the Drizzle migration via `pnpm drizzle-kit generate`.

### Req-2: Block Type Definitions in @nova/db

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

### Req-3: Synthesizer -- JSON Block Output

Update `packages/daemon/src/features/briefing/synthesizer.ts`:

- Replace `BRIEFING_SYSTEM_PROMPT` to instruct Claude to output a JSON array of typed blocks instead of markdown
- The prompt must define the block schema and provide examples for each block type
- Parse the Claude response as JSON and validate with `BriefingBlocksSchema` from `@nova/db`
- If JSON parsing or Zod validation fails, fall back to the current markdown synthesis path (call `buildStaticSummary` or re-run with the old prompt)
- Generate a markdown `content` string from the validated blocks for Telegram delivery (convert blocks to readable markdown)
- Update `SynthesisResult` to include `blocks: BriefingBlock[] | null` alongside existing `content` and `suggestedActions`
- Extract `suggestedActions` from `action_group` blocks instead of parsing a JSON code fence

### Req-4: Runner -- Persist Blocks

Update `packages/daemon/src/features/briefing/runner.ts`:

- Pass `synthesis.blocks` to the `INSERT INTO briefings` query as the `blocks` column value (JSON.stringify or null)
- No other changes needed -- Telegram delivery continues to use `synthesis.content` (markdown)

### Req-5: SSE Streaming Endpoint

Add `GET /api/briefing/stream` to `packages/daemon/src/http.ts`:

- Use the same `streamSSE` pattern from Hono as the existing `POST /chat` endpoint
- Stream individual blocks as they are generated: `{ type: "block", index: number, block: BriefingBlock }` events
- Send a final `{ type: "done", blocks: BriefingBlock[] }` event with the complete array
- Persist the completed briefing to the database after streaming finishes
- Send Telegram notification after persistence (same as current runner)
- Error events: `{ type: "error", message: string }` on failure
- If `briefingDeps` is not configured, return 503

### Req-6: Block Components

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

All components:
- Use Geist dark theme tokens (`surface-card`, `ds-gray-*`, `text-heading-*`, `text-copy-*`, `text-label-*`)
- Do NOT use cosmic/purple theme colors
- Accept `{ block: BriefingBlock }` as props (narrowed to the specific block type)

Create `apps/dashboard/components/blocks/BlockRegistry.tsx`:
- Export `BlockRegistry`: a `Record<string, React.ComponentType<{ block: BriefingBlock }>>` mapping block types to components
- Export `BriefingRenderer`: a component that takes `blocks: BriefingBlock[]` and renders each through the registry
- Unknown block types render `null` (forward-compatible)

### Req-7: Briefing Page -- Block Rendering + Streaming

Update `apps/dashboard/app/briefing/page.tsx`:

- If `displayEntry` has a `blocks` array (non-null, non-empty), render via `BriefingRenderer` instead of `parseBriefingSections` + `ReactMarkdown`
- If `displayEntry` only has `content` (old data, `blocks` is null), fall back to the current rendering path
- "Generate Now" button: when clicked, connect to `GET /api/briefing/stream` via `EventSource` and render blocks progressively as they arrive, with skeleton placeholders for remaining blocks
- Keep all existing UX: history navigation, polling, update banner, error handling, source status pills
- Add a `BriefingEntry.blocks` field to the dashboard type (`apps/dashboard/types/api.ts`)

## Scope
- **IN**: Schema migration (`blocks` column), block types in `@nova/db` (Zod + TS), synthesizer prompt update (JSON output), Zod validation with markdown fallback, runner persistence, SSE streaming endpoint, 10 block components, block registry + renderer, briefing page update (block rendering, streaming, backward compat), dashboard type update
- **OUT**: Telegram rich rendering (stays markdown via `content` column), block editor UI, user-customizable block ordering, block-level caching, block versioning, interactive block actions (e.g., dismissing actions from the dashboard), adding new block types beyond the initial 10

## Impact

| Area | Change |
|------|--------|
| `packages/db/src/schema/briefings.ts` | MODIFY -- add `blocks` JSONB column (nullable) |
| `packages/db/src/blocks.ts` | NEW -- Zod schemas + TypeScript types for 10 block types |
| `packages/db/src/index.ts` | MODIFY -- barrel export blocks |
| `packages/db/drizzle/` | NEW -- migration SQL (generated by drizzle-kit) |
| `packages/daemon/src/features/briefing/synthesizer.ts` | MODIFY -- JSON prompt, Zod validation, markdown fallback, blocks in SynthesisResult |
| `packages/daemon/src/features/briefing/runner.ts` | MODIFY -- persist blocks column |
| `packages/daemon/src/http.ts` | MODIFY -- add `GET /api/briefing/stream` SSE endpoint |
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
| `apps/dashboard/app/briefing/page.tsx` | MODIFY -- block rendering path, SSE streaming |
| `apps/dashboard/types/api.ts` | MODIFY -- add `blocks` field to `BriefingEntry` |
| `apps/dashboard/lib/briefing.ts` | UNCHANGED -- kept for backward compat with old briefings |

## Risks

| Risk | Mitigation |
|------|-----------|
| Claude outputs malformed JSON or wrong block types | Zod validation catches all errors. On failure, fall back to current markdown synthesis. The synthesizer never crashes -- it degrades gracefully. |
| Block schema changes break old stored data | `blocks` column is nullable. Old briefings have `blocks: null` and render via the markdown path. New schema changes are additive (new block types render null via the registry). |
| SSE streaming endpoint adds complexity to the HTTP server | The pattern already exists for `POST /chat`. The briefing stream is simpler (no conversation history, no tool calls). Reuses Hono `streamSSE` helper. |
| Large block arrays increase DB storage | Briefings are generated at most a few times per day. A 10-block array is ~2-5KB of JSON -- negligible compared to the existing `content` text column. |
| Dashboard renders incorrectly with mixed old/new briefings | Explicit branching: if `blocks` exists and is non-empty, use block renderer. Otherwise, fall back to markdown. History navigation works the same way per-entry. |
| Claude prompt engineering for consistent JSON output | The prompt includes explicit schema definition and examples. `maxTurns: 1` prevents multi-turn confusion. Zod validation is the safety net -- if Claude drifts, we fall back. |
