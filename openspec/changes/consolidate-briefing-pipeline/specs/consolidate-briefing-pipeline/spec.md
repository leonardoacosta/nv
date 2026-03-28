# Capability: Consolidate Briefing Pipeline

## REMOVED Requirements

### Requirement: Rust daemon briefing JSONL writer is retired
The Rust daemon SHALL NOT write briefing entries to JSONL files. The `BriefingStore` module (`crates/nv-daemon/src/briefing_store.rs`) SHALL be removed. The `send_morning_briefing()` method in the orchestrator SHALL be removed. The `MorningBriefing` cron event handler in the `TriggerClass::Digest` arm SHALL skip briefing generation (log and return, do not delete the event type). The axum HTTP endpoints `GET /api/briefing` and `GET /api/briefing/history` that read from JSONL SHALL be removed. The `briefing_store` field SHALL be removed from `HttpState` and `WorkerDeps`.

#### Scenario: MorningBriefing event is received by Rust daemon

Given the Rust scheduler emits a `MorningBriefing` cron event
When the orchestrator receives the event
Then it logs "morning briefing: delegated to TS daemon" and returns without writing any data
And no JSONL file is created or appended

#### Scenario: Rust HTTP briefing endpoints are removed

Given the axum router is built
When a client sends GET `/api/briefing` to the Rust daemon
Then the route does not exist (404)

#### Scenario: BriefingStore module is removed

Given the crate is compiled
When `crates/nv-daemon/src/briefing_store.rs` is absent
Then the crate compiles without errors (no remaining references to BriefingStore)

---

## ADDED Requirements

### Requirement: briefings table has a nullable blocks JSONB column
The `briefings` table in `packages/db/src/schema/briefings.ts` SHALL have a `blocks` column of type `jsonb`, nullable, with no default value. The existing `content`, `sources_status`, and `suggested_actions` columns MUST remain unchanged. A Drizzle migration MUST be generated via `pnpm drizzle-kit generate`.

#### Scenario: New briefing with blocks

Given a briefing is inserted with a valid blocks JSON array
When the row is queried
Then the `blocks` column contains the JSON array and `content` contains a markdown string

#### Scenario: Old briefing without blocks

Given a briefing was inserted before the migration (no blocks column value)
When the row is queried
Then the `blocks` column is null and `content` contains the original markdown

#### Scenario: Briefing with failed block generation

Given the synthesizer falls back to markdown
When the briefing is inserted
Then `blocks` is null and `content` contains the fallback markdown

---

### Requirement: block types are defined as a Zod discriminated union in @nova/db
`packages/db/src/blocks.ts` SHALL export a `BriefingBlockSchema` Zod discriminated union on the `type` field covering 10 block types: `section`, `status_table`, `metric_card`, `timeline`, `action_group`, `kv_list`, `alert`, `source_pills`, `pr_list`, `pipeline_table`. Each block MUST have a required `type` string, an optional `title` string, and a type-specific `data` object. A `BriefingBlocksSchema` (z.array of BriefingBlockSchema) MUST also be exported. Inferred TypeScript types `BriefingBlock` and `BriefingBlocks` MUST be exported.

#### Scenario: Valid section block passes validation

Given `{ type: "section", title: "Messages", data: { body: "No new messages." } }`
When validated against BriefingBlockSchema
Then validation succeeds and the inferred type narrows to the section variant

#### Scenario: Valid metric_card block passes validation

Given `{ type: "metric_card", data: { label: "Open Obligations", value: 5, trend: "down", delta: "-2 from yesterday" } }`
When validated against BriefingBlockSchema
Then validation succeeds with `title` defaulting to undefined

#### Scenario: Unknown block type fails validation

Given `{ type: "sparkline_chart", data: { points: [1, 2, 3] } }`
When validated against BriefingBlockSchema
Then validation fails with a Zod discriminated union error

#### Scenario: Missing required data field fails validation

Given `{ type: "alert", data: { message: "Server down" } }` (missing `severity`)
When validated against BriefingBlockSchema
Then validation fails because `severity` is required in alert data

#### Scenario: Full block array validates

Given an array of 6 blocks with types section, status_table, metric_card, timeline, alert, action_group
When validated against BriefingBlocksSchema
Then all 6 blocks pass and the inferred type is BriefingBlock[]

---

### Requirement: synthesizer outputs JSON blocks instead of markdown
The `synthesizeBriefing()` function in `packages/daemon/src/features/briefing/synthesizer.ts` SHALL instruct Claude to output a JSON array of typed blocks conforming to the BriefingBlockSchema. The Claude response MUST be parsed as JSON and validated with `BriefingBlocksSchema`. The `SynthesisResult` type MUST include a `blocks: BriefingBlock[] | null` field. A `content` markdown string MUST be derived from the blocks for Telegram delivery. Suggested actions MUST be extracted from `action_group` blocks.

#### Scenario: Successful JSON block synthesis

Given gatherContext returns obligations, messages, calendar, and memory data
When synthesizeBriefing is called
Then the result contains `blocks` as a non-null validated BriefingBlock array
And `content` contains a readable markdown string derived from the blocks
And `suggestedActions` contains actions extracted from action_group blocks

#### Scenario: Claude returns invalid JSON

Given Claude outputs a response that is not valid JSON (e.g., markdown text)
When the response is parsed
Then JSON.parse throws and the synthesizer falls back to buildStaticSummary
And `blocks` is null in the returned SynthesisResult
And a warning is logged

#### Scenario: Claude returns valid JSON but invalid block types

Given Claude outputs `[{ "type": "unknown_block", "data": {} }]`
When validated against BriefingBlocksSchema
Then Zod validation fails and the synthesizer falls back to buildStaticSummary
And `blocks` is null

#### Scenario: Markdown content generated from blocks

Given a validated block array containing a section block with body "3 pending obligations" and an alert block with severity "warning" and message "Calendar source unavailable"
When blocksToMarkdown is called
Then the output contains `### <section title>` followed by the body text
And the output contains a warning line for the alert

#### Scenario: Suggested actions extracted from action_group blocks

Given a validated block array containing an action_group block with actions `[{label: "Review PR #42"}, {label: "Check Sentry errors"}]`
When suggestedActions are extracted
Then the result contains 2 SuggestedAction items with labels "Review PR #42" and "Check Sentry errors"

---

### Requirement: runner persists blocks to the briefings table
The `runMorningBriefing()` function in `packages/daemon/src/features/briefing/runner.ts` SHALL include `synthesis.blocks` (as JSON.stringify or null) in the INSERT INTO briefings query. The `blocks` value MUST be persisted alongside `content`, `sources_status`, and `suggested_actions`.

#### Scenario: Blocks persisted on successful synthesis

Given synthesizeBriefing returns blocks as a 6-element BriefingBlock array
When runMorningBriefing inserts the briefing row
Then the `blocks` column contains the JSON-stringified array

#### Scenario: Null blocks persisted on fallback

Given synthesizeBriefing returns blocks as null (fallback to markdown)
When runMorningBriefing inserts the briefing row
Then the `blocks` column is null

---

### Requirement: SSE streaming endpoint delivers blocks progressively
A `GET /api/briefing/stream` endpoint SHALL be added to `packages/daemon/src/http.ts` using Hono `streamSSE`. The endpoint MUST stream individual blocks as `{ type: "block", index: number, block: BriefingBlock }` SSE events as they are generated. A final `{ type: "done", blocks: BriefingBlock[] }` event MUST be sent on completion. An `{ type: "error", message: string }` event MUST be sent on failure. The completed briefing MUST be persisted to the database after the stream finishes. The endpoint MUST return 503 if `briefingDeps` is not configured.

#### Scenario: Progressive block streaming

Given briefingDeps is configured and context is gathered
When GET /api/briefing/stream is opened as an EventSource
Then the client receives multiple SSE events with type "block" and incrementing index values
And each event contains a valid BriefingBlock in the block field
And the final event has type "done" with the complete blocks array

#### Scenario: Briefing persisted after stream completion

Given the stream completes with 8 blocks
When the "done" event is sent
Then a new row is inserted into the briefings table with blocks and content
And the latest briefing can be fetched via GET /api/briefing

#### Scenario: Stream error handling

Given context gathering fails with a timeout
When GET /api/briefing/stream is opened
Then the client receives an SSE event with type "error" and a descriptive message

#### Scenario: Briefing deps not configured

Given briefingDeps is null
When GET /api/briefing/stream is called
Then HTTP 503 is returned with `{ error: "Briefing system not configured" }`

---

### Requirement: missed-briefing detection alerts when no briefing exists for today
The TS daemon scheduler SHALL check the `briefings` table at `briefingHour + 1` (one hour after the configured briefing hour). If no briefing row exists with `generated_at` on today's date, it MUST send a Telegram alert and set a `missedToday` flag on the tRPC response. The check MUST fire at most once per day.

#### Scenario: No briefing generated by briefingHour + 1

Given the configured briefing hour is 7 and the current time is 08:00
And no briefing row exists with generated_at on today's date
When the missed-briefing check runs
Then a Telegram message is sent: "No morning briefing was generated today. The daemon may have been offline at 7:00. Use the dashboard 'Generate Now' button to create one."
And the tRPC briefing.latest response includes `missedToday: true`

#### Scenario: Briefing was generated on time

Given the configured briefing hour is 7 and the current time is 08:00
And a briefing row exists with generated_at at 07:01 today
When the missed-briefing check runs
Then no Telegram alert is sent
And the tRPC briefing.latest response includes `missedToday: false`

#### Scenario: Missed-briefing check fires at most once per day

Given the missed-briefing check already fired today (alert was sent)
When the scheduler polls again at 08:01
Then no duplicate alert is sent

#### Scenario: User generates briefing after missed alert

Given a missed-briefing alert was sent at 08:00
When the user clicks "Generate Now" and a briefing is created
Then subsequent tRPC briefing.latest calls return `missedToday: false`

---

### Requirement: block components render each block type with Geist tokens
`apps/dashboard/components/blocks/` SHALL contain one React component per block type (StatusTable, MetricCard, Timeline, ActionGroup, SectionBlock, KVList, AlertBlock, SourcePills, PRList, PipelineTable). All components MUST use Geist dark theme tokens (surface-card, ds-gray-*, text-heading-*, text-copy-*, text-label-*) and MUST NOT use cosmic/purple theme colors. Each component MUST accept `{ block: BriefingBlock }` as props.

#### Scenario: StatusTable renders columns and rows

Given a status_table block with columns ["Service", "Status", "Latency"] and 3 rows
When StatusTable renders
Then the output contains a table with 3 header cells and 3 body rows
And the table uses surface-card background and ds-gray-400 border styling

#### Scenario: MetricCard renders value with trend

Given a metric_card block with label "Open PRs", value 12, trend "up", delta "+3"
When MetricCard renders
Then the output shows "Open PRs" as label, "12" as value, an upward trend indicator in ds-green-700, and "+3" as delta text

#### Scenario: Timeline renders events with severity colors

Given a timeline block with 3 events: one info, one warning, one error
When Timeline renders
Then the output shows 3 timeline entries with dots colored ds-blue-700, ds-amber-700, and ds-red-700 respectively

#### Scenario: AlertBlock renders with severity border

Given an alert block with severity "error" and message "2 pipelines failed"
When AlertBlock renders
Then the output shows a card with a ds-red-700 left border and the message text

#### Scenario: PRList renders status indicators

Given a pr_list block with prs: [{title: "Add auth", repo: "nova", status: "open"}, {title: "Fix lint", repo: "nova", status: "merged"}]
When PRList renders
Then the output shows 2 list items with status indicators: open in ds-green-700, merged in ds-blue-700

---

### Requirement: BlockRegistry maps types to components and renders unknown types as null
`apps/dashboard/components/blocks/BlockRegistry.tsx` SHALL export a `BlockRegistry` record mapping each of the 10 block type strings to their corresponding React component. A `BriefingRenderer` component SHALL take a `blocks: BriefingBlock[]` prop and render each block through the registry. Unknown block types (not in the registry) MUST render `null`.

#### Scenario: Known block type renders its component

Given a blocks array with one status_table block
When BriefingRenderer renders
Then StatusTable component is rendered for that block

#### Scenario: Unknown block type renders null

Given a blocks array with one block of type "sparkline_chart" (not in registry)
When BriefingRenderer renders
Then no DOM element is produced for that block and no error is thrown

#### Scenario: Mixed known and unknown blocks

Given a blocks array with [section, unknown_type, metric_card]
When BriefingRenderer renders
Then SectionBlock and MetricCard render in order, with nothing between them for the unknown type

---

### Requirement: briefing page renders blocks or falls back to markdown
`apps/dashboard/app/briefing/page.tsx` SHALL render via `BriefingRenderer` when `displayEntry.blocks` is a non-null, non-empty array. When `displayEntry.blocks` is null or empty, it MUST fall back to the current `parseBriefingSections` + `ReactMarkdown` rendering path. All existing UX (history navigation, "Generate Now" button, polling, update banner, error handling, source status pills) MUST be preserved.

#### Scenario: New briefing with blocks renders via BriefingRenderer

Given displayEntry has blocks = [section, status_table, metric_card, action_group]
When the briefing page renders
Then BriefingRenderer is used and 4 block components are visible
And parseBriefingSections is not called

#### Scenario: Old briefing without blocks falls back to markdown

Given displayEntry has blocks = null and content = "### Messages\nNo new messages."
When the briefing page renders
Then parseBriefingSections is called and BriefingSectionCard components render the markdown sections

#### Scenario: History navigation between old and new briefings

Given history contains 2 entries: one with blocks (new) and one without (old)
When the user clicks the old entry in the history rail
Then the display switches to markdown rendering
And when the user clicks the new entry
Then the display switches to block rendering

---

### Requirement: Generate Now streams blocks progressively
When the "Generate Now" button is clicked, the briefing page SHALL connect to `GET /api/briefing/stream` via `EventSource`. As `block` events arrive, they MUST be appended to a local array and rendered progressively. Skeleton placeholders MUST be shown for blocks not yet received. On the `done` event, the EventSource MUST be closed and the page MUST refresh from the database. On an `error` event, an error banner MUST be shown.

#### Scenario: Progressive rendering during generation

Given the user clicks "Generate Now"
When the EventSource receives 3 "block" events
Then 3 block components are rendered and skeleton placeholders show below them

#### Scenario: Generation completes

Given the EventSource receives a "done" event with 8 blocks
When the event is processed
Then the EventSource is closed
And the page refreshes from the database showing the persisted briefing
And skeleton placeholders are removed

#### Scenario: Generation fails mid-stream

Given the EventSource receives 2 "block" events then an "error" event with message "Synthesis timed out"
When the error event is processed
Then an error banner shows "Synthesis timed out"
And the 2 already-rendered blocks remain visible
And the user can retry with "Generate Now"

#### Scenario: Streaming indicator replaces spinner

Given the user clicks "Generate Now" and SSE streaming starts
When the first block event arrives
Then the loading spinner is replaced by the first rendered block with remaining skeleton placeholders

---

### Requirement: missed-briefing banner shown on dashboard
When the tRPC `briefing.latest` response includes `missedToday: true`, the briefing page SHALL display a banner: "No briefing generated today. Generate one now?" with a button that triggers the "Generate Now" flow. The banner MUST be dismissed when a briefing is generated.

#### Scenario: Missed briefing banner appears

Given the tRPC response has `missedToday: true` and `entry: null`
When the briefing page renders
Then a missed-briefing banner is visible with "Generate one now?" action

#### Scenario: Banner dismissed after generation

Given the missed-briefing banner is shown
When the user clicks "Generate one now?" and the briefing completes
Then the banner is hidden and the new briefing is displayed

---

## MODIFIED Requirements

### Requirement: BriefingEntry type includes blocks field
The `BriefingEntry` interface in `apps/dashboard/types/api.ts` SHALL include `blocks: BriefingBlock[] | null`. The `BriefingBlock` type MUST be imported from `@nova/db` or re-declared as a matching type. Existing fields (`id`, `generated_at`, `content`, `suggested_actions`, `sources_status`) MUST remain unchanged.

#### Scenario: API response includes blocks

Given a briefing row has a non-null blocks column
When the tRPC briefing.latest returns the entry
Then the response JSON includes `blocks` as an array of typed objects

#### Scenario: API response with null blocks

Given a briefing row has blocks = null (old data)
When the tRPC briefing.latest returns the entry
Then the response JSON includes `blocks: null`

### Requirement: tRPC briefing router exposes blocks and missed status
The `briefingRouter` in `packages/api/src/routers/briefing.ts` SHALL include the `blocks` field in `mapBriefingRow()` output. The `latest` procedure SHALL include a `missedToday: boolean` field, computed by checking if the latest briefing's `generated_at` is today and if current time is past `briefingHour + 1`. The `briefing_hour` setting SHALL be read from the `settings` table with a default of 7.

#### Scenario: Latest briefing includes blocks

Given the newest briefing row has a non-null blocks column
When the `latest` procedure is called
Then the response includes the parsed blocks array

#### Scenario: missedToday is true when no briefing today

Given the latest briefing was generated yesterday and current time is 08:05
When the `latest` procedure is called
Then the response includes `missedToday: true`

#### Scenario: missedToday is false when briefing exists today

Given a briefing was generated at 07:02 today
When the `latest` procedure is called
Then the response includes `missedToday: false`

#### Scenario: missedToday is false before briefingHour + 1

Given the configured briefing hour is 7 and current time is 07:30
And no briefing exists for today
When the `latest` procedure is called
Then the response includes `missedToday: false` (too early to declare missed)
