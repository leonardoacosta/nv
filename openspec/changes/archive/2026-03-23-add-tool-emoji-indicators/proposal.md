# Proposal: Tool Emoji Indicators

## Change ID
`add-tool-emoji-indicators`

## Summary

Replace the invisible typing indicator with real-time emoji tool status messages in Telegram, and
harden `strip_tool_call_artifacts()` to catch leaked `[Called tool: ...]` single-line patterns.
When Claude calls a tool, the existing "thinking" message is edited to show an emoji + description
(e.g., `🔍 Searching Jira...`), then replaced by the final response. This gives the user live
feedback about what Nova is doing without spamming extra messages.

## Context
- Extends: `crates/nv-daemon/src/worker.rs` (strip_tool_call_artifacts, WorkerEvent), `crates/nv-daemon/src/orchestrator.rs` (humanize_tool, handle_worker_event, check_inactivity), `crates/nv-daemon/src/agent.rs` (thinking message lifecycle), `crates/nv-daemon/src/telegram/client.rs` (edit_message)
- Related: `WorkerEvent::ToolCalled` is already emitted in the tool loop and received by the orchestrator, but only trace-logged. `humanize_tool()` already maps tool names to descriptions. The thinking message (`"..."`) is already sent and later edited with the final response.
- Depends on: nothing — standalone improvement

## Motivation

When Nova processes a request, the user sees only `...` and a typing indicator. There's no
visibility into which tool is running or why a response is taking time. Additionally,
`[Called tool: jira_search with {...}]` text sometimes leaks into the final Telegram message
because `strip_tool_call_artifacts()` only handles fenced code blocks, not the single-line format
emitted by `claude.rs` line 836. These two issues together make the chat feel opaque and unpolished.

## Requirements

### Req-1: Strip `[Called tool: ...]` Patterns

Extend `strip_tool_call_artifacts()` in `worker.rs` to also remove single-line patterns matching
`[Called tool: <name> with <json>]`. These are emitted by `claude.rs` when building conversation
history summaries and sometimes leak into the response text extracted by `extract_text()`.

- Match the pattern: `[Called tool: <any> with <any>]` (single line, may appear multiple times)
- Remove the entire line (including any preceding preamble sentence on the same line)
- Preserve all other text unchanged
- Add tests for the new pattern

### Req-2: Emoji Mapping in `humanize_tool()`

Extend `humanize_tool()` in `orchestrator.rs` to return `(emoji, description)` pairs instead of
plain strings. Every existing mapping gets an emoji prefix. The fallback for unknown tools uses a
generic gear emoji.

Example mappings:
| Tool | Emoji | Description |
|------|-------|-------------|
| `jira_search` / `jira_get` | `🔍` | Searching Jira... |
| `jira_create` / `jira_transition` | `✏️` | Updating Jira... |
| `read_memory` / `search_memory` | `🧠` | Reading memory... |
| `write_memory` | `💾` | Saving to memory... |
| `query_nexus` / `query_session` | `🔗` | Checking Nexus sessions... |
| `gh_pr_list` / `gh_run_status` | `🐙` | Checking GitHub... |
| `neon_query` | `🗄️` | Querying database... |
| `search_messages` | `💬` | Searching conversation history... |
| (unknown) | `⚙️` | Running {tool_name}... |

Return type changes from `String` to `(String, String)` — `(emoji, description)`.

### Req-3: Edit Thinking Message with Tool Status

When `WorkerEvent::ToolCalled` fires, the orchestrator should edit the existing thinking message
(`"..."`) to show the emoji + description. This reuses the message already sent by the agent —
no new messages are created.

Flow:
1. Agent sends thinking message (`"..."`) — returns `thinking_msg_id` (existing behavior)
2. Orchestrator receives `ToolCalled` event → calls `edit_message(chat_id, thinking_msg_id, "🔍 Searching Jira...")`
3. If another `ToolCalled` fires, edit again with the new tool status
4. When the worker completes, the agent edits the thinking message with the final response (existing behavior) — this naturally replaces the last tool indicator
5. If the worker errors, the agent's existing error handling deletes the thinking message (existing behavior)

Implementation detail:
- The orchestrator needs access to the `thinking_msg_id` for the active worker. Thread this through `WorkerEvent::StageStarted` or add it to the `WorkerTask`/`SharedDeps` worker-to-orchestrator channel.
- The simplest approach: add `thinking_msg_id: Option<i64>` to `WorkerEvent::StageStarted` on the first stage event, and have the orchestrator track it alongside the existing `worker_stage_started` map.
- Only edit if the thinking message exists (Telegram channel is active) — skip silently for CLI-only sessions.

### Req-4: Typing Indicator Refresh Continues

The existing typing indicator refresh (every 5s in `check_inactivity`) should continue running
alongside the emoji status edits. The typing indicator shows "Nova is typing..." in the chat
header, while the emoji message shows specific tool status in the chat body. Both are valuable.

- No changes needed to the typing refresh loop
- The emoji edit and typing refresh are independent — no coordination required

## Scope
- **IN**: strip `[Called tool: ...]` patterns, emoji+description return from `humanize_tool()`, edit thinking message on ToolCalled events, track thinking_msg_id in orchestrator
- **OUT**: separate status messages (reuse thinking message only), tool completion events (ToolCalled is fire-and-forget, final response replaces it), per-tool progress bars, message chains

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/worker.rs` | Add `[Called tool: ...]` stripping to `strip_tool_call_artifacts()` |
| `crates/nv-daemon/src/orchestrator.rs` | Change `humanize_tool()` return type to `(String, String)`, edit thinking message on ToolCalled, track thinking_msg_id per worker |
| `crates/nv-daemon/src/agent.rs` | Emit thinking_msg_id to orchestrator (via WorkerEvent or shared state) |

## Risks
| Risk | Mitigation |
|------|-----------|
| Editing thinking message races with final response edit | The agent owns the final edit — orchestrator only edits between tool calls. The agent cancels the ticker before editing, so there's a natural ordering. Add a small debounce (skip edit if last edit was <500ms ago) to avoid Telegram rate limits. |
| `humanize_tool()` return type change breaks callers | Only one call site in `handle_worker_event()` — update it in the same change |
| Telegram rate limit on rapid tool-call edits | Debounce: only edit if >500ms since last edit for this message. Tools called in rapid succession show only the last one. |
| `[Called tool: ...]` regex too greedy | Pattern is specific: `\[Called tool: .+ with .+\]` — unlikely to match user content. Add negative tests for similar-looking text. |
