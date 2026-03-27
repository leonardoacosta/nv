# Proposal: Add Response Streaming

## Change ID
`add-response-streaming`

## Summary

Progressive response streaming so Telegram users see real-time progress during long agent operations (1-21 min) instead of just "typing..." indicators. Uses Telegram Bot API `sendMessageDraft` (API 9.5) for native progressive rendering with tool status indicators, falling back to `editMessageText` for older clients.

## Context
- Extends: `packages/daemon/src/brain/agent.ts` (NovaAgent with `processMessage()` and `processMessageStream()`), `packages/daemon/src/brain/types.ts` (ToolCall, AgentResponse), `packages/daemon/src/channels/telegram.ts` (TelegramAdapter), `packages/daemon/src/index.ts` (message routing loop)
- Related: `add-chat-page` (added `processMessageStream()` yielding chunk/done events), `wire-conversation-history` (conversation history in agent loop)
- Agent SDK: `query()` returns `AsyncIterable<SDKMessage>` with `assistant` messages containing `tool_use` and `text` content blocks. The `includePartialMessages` option would yield intermediate `stream_event` messages with `text_delta` and tool progress events.
- MCP tool naming: tools follow `mcp__<server-name>__<tool_name>` pattern (e.g. `mcp__nova-teams__teams_list_chats`, `mcp__nova-calendar__calendar_list_events`). The server name portion maps to a human-readable service name.
- Telegram limits: messages capped at 4096 chars, `sendMessageDraft` uses a `draft_id` (stable non-zero int per response), `editMessageText` has rate limits (~30 calls/sec per chat).
- `processMessageStream()` already exists in `agent.ts` (lines 194-287) yielding `{ type: "chunk", text: string }` and `{ type: "done", response: AgentResponse }`. This spec enriches the stream with tool lifecycle events and wires it to Telegram drafts.

## Motivation

1. **Silence during long operations**: Nova takes 1-21 minutes to respond. Users see only "typing..." (refreshed every 4s) with zero visibility into what Nova is doing. This creates anxiety and uncertainty -- users cannot tell if Nova is stuck, working, or about to finish.
2. **No tool visibility**: When Nova calls MCP tools (Teams, Calendar, Azure, Discord), the user has no idea which services are being queried. Competing assistants (ChatGPT, Claude.ai, Perplexity) show tool/step indicators during processing.
3. **Existing stream infrastructure underused**: `processMessageStream()` already extracts text blocks incrementally from the SDK stream, but the Telegram message handler in `index.ts` (line 273) calls `processMessage()` (blocking), waits for the full response, then sends it. The streaming generator is only used by the dashboard chat page.
4. **Draft API available**: Telegram Bot API 9.5 added `sendMessageDraft` which renders as a native "draft" bubble (similar to how users see their own typing). This is lower-friction than `editMessageText` (no message flash, no notification, 300ms throttle vs 1000ms).

## Requirements

### Req-1: Rich Stream Events from Agent

Refactor `processMessageStream()` in `agent.ts` to yield a richer set of events beyond chunk/done:

- `{ type: "text_delta", text: string }` -- partial text from assistant message text blocks (replaces current `chunk` event)
- `{ type: "tool_start", name: string, callId: string }` -- emitted when a `tool_use` content block is encountered in an assistant message
- `{ type: "tool_done", name: string, callId: string, durationMs: number }` -- emitted when a `tool_result` message arrives for a tracked tool call
- `{ type: "done", response: AgentResponse }` -- final event with accumulated result (unchanged)

The `tool_start`/`tool_done` pairing requires tracking in-flight tool calls by their `id` field from the `tool_use` block. When an `sdkMsg.type === "result"` of subtype `tool_result` arrives (or the next assistant message after a tool_use), the corresponding `tool_done` event fires with elapsed time.

Backward compatibility: `processMessage()` (non-streaming) remains unchanged. The existing dashboard chat SSE endpoint that consumes `processMessageStream()` must continue to work -- the `text_delta` event replaces `chunk` but carries the same payload shape.

### Req-2: StreamEvent Union Type

Add a `StreamEvent` discriminated union to `packages/daemon/src/brain/types.ts`:

```typescript
export type StreamEvent =
  | { type: "text_delta"; text: string }
  | { type: "tool_start"; name: string; callId: string }
  | { type: "tool_done"; name: string; callId: string; durationMs: number }
  | { type: "done"; response: AgentResponse };
```

Update `processMessageStream()` return type to `AsyncGenerator<StreamEvent>`.

### Req-3: Tool Name Humanization

Create `packages/daemon/src/channels/tool-names.ts` with a `humanizeToolName(rawName: string)` function that converts MCP tool names to user-friendly labels:

- Parse the `mcp__<server>__<tool>` pattern to extract the server name
- Map known server prefixes to service names: `nova-teams` -> "Teams", `nova-calendar` -> "Calendar", `nova-discord` -> "Discord", `nova-mail` -> "Mail", `nova-contacts` -> "Contacts", `nova-ado` -> "Azure DevOps", `nova-graph` -> "Microsoft Graph", `nova-memory` -> "Memory", `nova-meta` -> "Meta"
- Map known tool action prefixes to verbs: `list_` -> "Checking", `get_` -> "Reading", `search_` -> "Searching", `send_` -> "Sending", `create_` -> "Creating", `update_` -> "Updating", `delete_` -> "Deleting"
- Combine: `mcp__nova-teams__teams_list_chats` -> "Checking Teams..."
- Built-in tools (Read, Write, Bash, Glob, Grep, WebSearch, WebFetch) map to: "Reading files...", "Writing files...", "Running command...", "Searching files...", "Searching files...", "Searching the web...", "Fetching page..."
- Unknown tools: strip `mcp__` prefix, replace underscores with spaces, title case -> "Working..."

### Req-4: TelegramStreamWriter

Create `packages/daemon/src/channels/stream-writer.ts` with a `TelegramStreamWriter` class that manages the draft message lifecycle for a single response:

**Constructor**: `new TelegramStreamWriter(adapter: TelegramAdapter, chatId: string)` -- stores adapter reference and chat ID, generates a random non-zero `draftId` (integer).

**State management**:
- `currentText: string` -- accumulated display text (what the user sees in the draft)
- `activeTools: Map<string, { name: string; startedAt: number }>` -- in-flight tool calls keyed by callId
- `lastFlushAt: number` -- timestamp of last draft send (for throttling)
- `finalMessageId: number | null` -- ID of the final sent message (for cleanup)

**Methods**:

- `onTextDelta(text: string)`: Append text to `currentText`. Strip incomplete Markdown (unclosed `**`, `` ` ``, `\n````) during streaming to avoid rendering glitches. Schedule a throttled flush.
- `onToolStart(name: string, callId: string)`: Add to `activeTools` map. Update the status line at the top of `currentText` with humanized tool name (Req-3). Schedule a throttled flush.
- `onToolDone(name: string, callId: string, durationMs: number)`: Remove from `activeTools`. Update status line to show completion with elapsed time (e.g. "Checked Teams (2s)"). Schedule a throttled flush.
- `flush()`: If less than 300ms since `lastFlushAt`, skip (throttle). Otherwise, build the draft text: status line (if tools active) + accumulated text, truncated to 4096 chars. Call `adapter.sendDraft(chatId, draftId, draftText)`. Update `lastFlushAt`.
- `finalize(fullText: string)`: Send the complete formatted response via `adapter.sendMessage()` with Markdown parse mode. This replaces the draft with the final message. Handle 4096-char splitting (reuse existing chunking logic from `index.ts`).
- `abort(error: string)`: Send error message, clean up draft state.

**Throttling**: 300ms minimum between `sendDraft` calls. If `sendDraft` is unavailable (method not found, API error), fall back to `editMessageText` with 1000ms throttle on a single placeholder message.

### Req-5: sendDraft() on TelegramAdapter

Add a `sendDraft(chatId: string | number, draftId: number, text: string)` method to `TelegramAdapter` that makes a raw HTTP POST to the Telegram Bot API:

```
POST https://api.telegram.org/bot<token>/sendMessageDraft
Content-Type: application/json
{ "chat_id": chatId, "draft_id": draftId, "text": text }
```

Since `node-telegram-bot-api` (v0.67) does not expose `sendMessageDraft`, this method calls the API directly using `fetch()`. The method should:
- Return `true` on success, `false` on failure (non-200 response or method not found)
- Log failures at `debug` level (expected when API version does not support drafts)
- Cache the availability result: if the first call fails with "method not found" or 404, set a flag so subsequent calls skip the HTTP request and return `false` immediately

The bot token is needed for the URL. Since `TelegramAdapter` already receives the token in the constructor, store it as a private field for reuse.

### Req-6: Wire Streaming into Message Handler

Replace the blocking `processMessage()` call in the Telegram message handler (`index.ts` lines 257-350) with the streaming `processMessageStream()`:

1. Create a `TelegramStreamWriter` instance for each incoming message.
2. Iterate `processMessageStream()` events:
   - `text_delta` -> call `writer.onTextDelta(event.text)`
   - `tool_start` -> call `writer.onToolStart(event.name, event.callId)`
   - `tool_done` -> call `writer.onToolDone(event.name, event.callId, event.durationMs)`
   - `done` -> call `writer.finalize(event.response.text)`, proceed to diary entry and conversation save
3. Remove the typing indicator interval (`setInterval` every 4s) -- the draft messages replace it.
4. Keep the error handling: on exception, call `writer.abort("Sorry, something went wrong.")`.
5. The conversation save, diary entry, and dream scheduler increment remain unchanged -- they fire after `done`.

### Req-7: Dashboard Chat SSE Compatibility

The dashboard chat page consumes `processMessageStream()` via an SSE endpoint. The event type rename (`chunk` -> `text_delta`) must be reflected in the SSE serialization. Update the HTTP SSE handler (in `http.ts` or wherever the chat SSE route is defined) to emit `text_delta` events instead of `chunk`. The dashboard client must also be updated to listen for `text_delta`.

Alternatively, if the SSE handler maps events to its own wire format, the change is contained in the handler and the dashboard client needs no update.

## Scope
- **IN**: Rich stream events from agent, StreamEvent type, tool name humanization, TelegramStreamWriter class, sendDraft() raw API call, wiring streaming into Telegram handler, dashboard SSE compatibility
- **OUT**: Telegram Bot API upgrade (we call the raw HTTP endpoint), node-telegram-bot-api version bump, new Telegram message types (stickers, documents), voice message streaming, multi-chat concurrent streaming limits, message reaction streaming, dashboard UI changes for tool indicators

## Impact

| Area | Change |
|------|--------|
| `packages/daemon/src/brain/types.ts` | MODIFY -- add `StreamEvent` union type |
| `packages/daemon/src/brain/agent.ts` | MODIFY -- refactor `processMessageStream()` to yield rich events (`text_delta`, `tool_start`, `tool_done`, `done`), track in-flight tool calls |
| `packages/daemon/src/channels/tool-names.ts` | NEW -- `humanizeToolName()` function with server-to-name and action-to-verb mappings |
| `packages/daemon/src/channels/stream-writer.ts` | NEW -- `TelegramStreamWriter` class managing draft lifecycle, throttling, tool status, finalization |
| `packages/daemon/src/channels/telegram.ts` | MODIFY -- add `sendDraft()` raw HTTP method, store bot token as private field |
| `packages/daemon/src/index.ts` | MODIFY -- replace `processMessage()` with `processMessageStream()` loop, create `TelegramStreamWriter` per message, remove typing interval |

## Risks

| Risk | Mitigation |
|------|-----------|
| `sendMessageDraft` may not exist in Telegram Bot API 9.5 (unverified) | Fallback to `editMessageText` with 1000ms throttle is built into `TelegramStreamWriter`. The `sendDraft()` method caches availability on first call -- if unsupported, all subsequent responses use the edit fallback with zero overhead. |
| Agent SDK `includePartialMessages` option may not exist or behave differently | The current `AsyncIterable<SDKMessage>` already yields `assistant` messages incrementally. The rich events are derived from existing `tool_use` blocks and `text` blocks in assistant messages. `tool_done` timing uses the delta between `tool_start` and the next assistant message (not a dedicated SDK event). No new SDK features are strictly required. |
| 300ms draft throttle may still hit Telegram rate limits under heavy tool use | The throttle ensures max ~3.3 calls/sec, well under Telegram's ~30 calls/sec per chat limit. Tool status updates batch into a single flush. |
| Partial Markdown stripping may corrupt text | Strip only unclosed delimiters at the end of the accumulated buffer. The final `finalize()` call sends the complete, unstripped text from `AgentResponse.text`. |
| `editMessageText` fallback creates message flash (notification + visual jump) | Expected degradation. The edit fallback uses a single placeholder message created once via `sendMessage("Thinking...")`, then edits it. Users see content replace "Thinking..." progressively rather than a new message per update. |
| Dashboard SSE event rename (`chunk` -> `text_delta`) breaks existing dashboard | The SSE handler maps internal events to wire format. If the wire format stays `chunk`, no dashboard change needed. If renamed, the dashboard chat component needs a one-line event name update. |
