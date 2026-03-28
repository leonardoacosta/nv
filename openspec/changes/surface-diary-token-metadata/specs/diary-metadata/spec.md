# Capability: Diary Metadata Surface

## MODIFIED Requirements

### Requirement: diary schema stores model and cost per entry

The `diary` Postgres table SHALL gain two nullable columns: `model` (text) for the Claude model name,
and `cost_usd` (real) for the estimated interaction cost. Both MUST be nullable to preserve backward
compatibility with existing rows.

#### Scenario: New diary entry with model and cost
Given the daemon processes a message using `claude-opus-4-6` with 1200 input tokens and 340 output tokens
When `writeEntry()` is called
Then a diary row is inserted with `model = "claude-opus-4-6"` and `cost_usd = 0.0435`

#### Scenario: Legacy entry without model
Given an existing diary row was written before this change
When the tRPC diary.list procedure reads that row
Then `model` is `null` and `cost_usd` is `null` in the response

### Requirement: diary writer stores structured tool call details

The `toolsUsed` JSONB column SHALL transition from `string[]` to `Array<ToolCallDetail>` where
`ToolCallDetail = { name: string, input_summary: string, duration_ms: number | null }`.
The `input_summary` MUST be the first 120 characters of `JSON.stringify(toolInput)`. The `duration_ms`
comes from tool execution timing already tracked in the streaming path.

#### Scenario: Streaming response writes structured tool details
Given `processMessageStream()` completes with 2 tool calls: `read_memory` (45ms) and `jira_search` (320ms)
When `writeEntry()` is called
Then `tools_used` contains `[{name: "read_memory", input_summary: "{\"topic\":\"decisions\"}", duration_ms: 45}, {name: "jira_search", input_summary: "{\"query\":\"sprint backlog\",\"limit\":10}", duration_ms: 320}]`

#### Scenario: Non-streaming response writes structured tool details with timing
Given `processMessage()` completes with 1 tool call: `send_telegram` (120ms)
When `writeEntry()` is called
Then `tools_used` contains `[{name: "send_telegram", input_summary: "{\"chat_id\":\"123\",\"text\":\"Hello\"}", duration_ms: 120}]`

#### Scenario: Input summary truncation
Given a tool call with input JSON longer than 120 characters
When the diary entry is written
Then `input_summary` is the first 120 characters followed by "..."

### Requirement: diary reader normalizes both legacy and structured toolsUsed

The tRPC diary.list procedure SHALL normalize `tools_used` from both legacy `string[]` format and
new `ToolCallDetail[]` format into a unified response shape.

#### Scenario: Legacy string array row
Given a diary row has `tools_used = ["read_memory", "jira_search"]`
When diary.list returns the entry
Then `tools_called` contains `["read_memory", "jira_search"]` and `tools_detail` contains `[{name: "read_memory", input_summary: "", duration_ms: null}, {name: "jira_search", input_summary: "", duration_ms: null}]`

#### Scenario: New structured row
Given a diary row has `tools_used = [{name: "read_memory", input_summary: "...", duration_ms: 45}]`
When diary.list returns the entry
Then `tools_called` contains `["read_memory"]` and `tools_detail` contains the structured array as-is

### Requirement: cost estimation returns USD from model and token counts

A static pricing lookup function `estimateCost(model, tokensIn, tokensOut)` SHALL estimate USD cost.
The function MUST return `null` for unknown models.

#### Scenario: Known model cost calculation
Given model `claude-opus-4-6`, tokensIn `1000`, tokensOut `500`
When `estimateCost()` is called
Then it returns `0.0525` (1000 * 15/1M + 500 * 75/1M)

#### Scenario: Unknown model returns null
Given model `claude-unknown-99`
When `estimateCost()` is called
Then it returns `null`

### Requirement: collapsed diary row displays token, latency, cost, and tool badges

The collapsed diary entry row SHALL display compact metadata badges inline after the summary text:
token count, latency, estimated cost, and up to 3 tool pills with overflow indicator. Badges MUST be hidden below the `sm` breakpoint.

#### Scenario: Entry with 2 tools and token data
Given a diary entry with tokensIn=1200, tokensOut=340, latency=890ms, tools=["read_memory", "jira_search"], cost=0.04
When the collapsed row renders
Then it shows "1.2k+340" token badge, "890ms" latency badge, "$0.04" cost badge, and 2 tool pills

#### Scenario: Entry with 5 tools shows overflow
Given a diary entry with 5 tools called
When the collapsed row renders
Then it shows 3 tool pills and a "+2" overflow indicator

#### Scenario: Narrow viewport hides badges
Given the viewport width is below the `sm` breakpoint (640px)
When the collapsed row renders
Then metadata badges (tokens, latency, cost, tools) are hidden

### Requirement: diary.list returns daily aggregate summary

The diary.list tRPC response SHALL include an `aggregates` object with totals and averages for the
requested date.

#### Scenario: Date with multiple entries
Given 10 diary entries for 2026-03-27 with varying tokens and costs
When diary.list is called with date=2026-03-27
Then `aggregates.total_tokens_in` is the sum of all tokensIn, `aggregates.total_cost_usd` is the sum of all costUsd (excluding nulls), and `aggregates.tool_frequency` lists the top 10 tools by count

#### Scenario: Date with no entries
Given no diary entries for 2026-03-28
When diary.list is called with date=2026-03-28
Then `aggregates` has all-zero values and empty `tool_frequency`

## ADDED Requirements

### Requirement: DiaryEntryItem includes model, cost, and structured tool detail

`DiaryEntryItem` SHALL include `model: string | null`, `cost_usd: number | null`, `tools_detail: ToolCallDetail[]`.
A `DiaryAggregates` interface MUST be added with `total_tokens_in`, `total_tokens_out`, `total_cost_usd`, `avg_latency_ms`, `tool_frequency`.
`DiaryGetResponse` SHALL include `aggregates: DiaryAggregates`.

#### Scenario: API response includes new fields
Given a diary entry with model="claude-opus-4-6", cost_usd=0.04, and structured tools_used
When the dashboard fetches diary.list
Then each entry in the response has `model`, `cost_usd`, and `tools_detail` fields alongside existing `tools_called`
