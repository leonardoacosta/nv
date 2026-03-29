# Proposal: Unify Conversation Streaming

## Change ID
`unify-conversation-streaming`

## Summary

Unify Nova's conversation system so every message exchange is persisted regardless of routing tier, the agent sees cross-channel history, a WebSocket event system streams all activity to the dashboard in real time, and messages flow bidirectionally between Telegram and dashboard.

## Context
- Extends: `packages/daemon/src/` (HTTP server, brain, channels), `apps/dashboard/` (chat page, event context)
- Depends on: `add-agent-sdk-integration` (completed), `fix-websocket-integration` (completed -- custom server proxies `/ws/events` to daemon)
- Related: `cross-channel-routing` (archived -- Rust-era cross-channel tools; this supersedes the TS daemon equivalent)
- Daemon: TypeScript, Hono HTTP on port 7700, `@hono/node-server`
- Dashboard: Next.js App Router with custom `server.ts` proxying WebSocket upgrades to daemon
- ConversationManager: raw `pg.Pool`, not Drizzle (daemon side); dashboard reads via Drizzle `@nova/db`
- DaemonEventContext: already implements full WebSocket client with reconnect backoff, subscriber registry, and `useDaemonEvents` hook -- currently connecting to `/ws/events` which does not exist on the daemon yet

## Motivation

Five gaps exist in the current conversation architecture:

1. **Tier 1/2 messages never persisted.** When the keyword or embedding router handles a message (lines 510-588 of `index.ts`), the response is sent to Telegram and logged to the diary, but `conversationManager.saveExchange()` is never called. These exchanges vanish from conversation history.

2. **Channel-scoped agent context.** `conversationManager.loadHistory()` queries `WHERE channel = $1`, so the agent only sees messages from the originating channel. A Telegram conversation and a dashboard conversation are invisible to each other -- Nova has amnesia across channels.

3. **No cross-channel streaming.** When Nova streams a response on Telegram via `TelegramStreamWriter`, the dashboard has no awareness. When Nova responds via the `/chat` SSE endpoint, Telegram sees nothing. Each channel is a dead-end pipeline.

4. **No WebSocket endpoint.** `DaemonEventContext.tsx` already connects to `/ws/events` with full reconnection logic, subscriber dispatch, and status indicators. But the daemon's Hono server has no WebSocket upgrade handler -- the connection always fails. The dashboard's real-time event system is wired but has no server.

5. **History vs context mismatch.** The `/api/chat/history` endpoint returns ALL messages (no channel filter), so the dashboard shows cross-channel history. But the agent only loads channel-scoped history. The user sees conversations Nova cannot remember.

## Requirements

### Req-1: Persist ALL Message Exchanges

Extend the Telegram message handler in `packages/daemon/src/index.ts` to call `conversationManager.saveExchange()` for Tier 1 (keyword) and Tier 2 (embedding) routed messages.

Currently (lines 510-588), when a Tier 1/2 route matches and `responseText` is non-empty, the response is sent to Telegram and a diary entry is written, but no conversation exchange is saved. After the `writeEntry()` call and before the early `return`, insert a `saveExchange()` call with `channelKey = "telegram:{chatId}"`.

The assistant message must be constructed identically to the Tier 3 pattern (lines 645-651): `senderId: "nova"`, `senderName: "nova"`, `content: responseText`, `text: responseText`.

#### Scenario: Tier 1 keyword response persisted

Given a Telegram message "docker status" that matches the keyword router,
when the fleet tool responds successfully,
then `conversationManager.saveExchange("telegram:{chatId}", userMsg, assistantMsg)` is called with the user's original message and Nova's tool response,
and a subsequent `loadHistory("telegram:{chatId}", 10)` includes both messages.

#### Scenario: Tier 2 embedding response persisted

Given a Telegram message that matches no keyword but scores above the embedding threshold,
when the fleet tool responds successfully,
then `saveExchange` is called identically to the Tier 1 case.

#### Scenario: Fleet tool failure does not persist

Given a Tier 1/2 match where the fleet tool call throws,
when the handler falls through to Tier 3,
then no `saveExchange` is called at the Tier 1/2 level (the Tier 3 path handles its own persistence).

### Req-2: Unified Conversation Context

Modify `ConversationManager.loadHistory()` in `packages/daemon/src/brain/conversation.ts` to load messages across ALL channels, not scoped to a single channel.

Current query: `WHERE channel = $1 ORDER BY created_at DESC LIMIT $2`

New query: `ORDER BY created_at DESC LIMIT $1` (remove channel filter entirely). The method signature changes from `loadHistory(channelId: string, limit: number)` to `loadHistory(limit: number)`.

Each message row already stores its `channel` value, and `rowToMessage()` maps it into `Message.channel`. The agent's `formatHistoryBlock()` in `agent.ts` already includes sender info -- it should be extended to include the channel so the agent knows message provenance:

Current format: `[sender] (timestamp): content`
New format: `[sender via channel] (timestamp): content`

Update all call sites:
- `packages/daemon/src/index.ts` line 609: `loadHistory(channelKey, depth)` -> `loadHistory(depth)`
- `packages/daemon/src/http.ts` line 111: `loadHistory("dashboard:web", depth)` -> `loadHistory(depth)`

#### Scenario: Agent sees cross-channel messages

Given 3 messages in `telegram:123` and 2 in `dashboard:web`,
when `loadHistory(10)` is called,
then all 5 messages are returned in chronological order,
and each retains its original `channel` value.

#### Scenario: History block includes channel provenance

Given a history containing messages from both Telegram and dashboard,
when `formatHistoryBlock()` formats the messages,
then each line reads `[user via telegram] (2026-03-27 14:00): message text` or `[nova via dashboard] (2026-03-27 14:01): response text`.

### Req-3: WebSocket Event System

Add a Hono WebSocket upgrade handler at `/ws/events` in `packages/daemon/src/http.ts`.

**Dependencies:** Use `hono/ws` for the WebSocket upgrade within Hono, with `@hono/node-ws` as the Node.js WebSocket adapter. This requires modifying the server setup in `index.ts` to use `createNodeWebSocket()` and inject the WebSocket middleware.

**Authentication:** Validate a `token` query parameter against `config.dashboardToken` (new config field sourced from `DASHBOARD_TOKEN` env var). If the token is missing or invalid, close the connection with code 4001 and reason "unauthorized".

**Connection management:**
- Maintain a `Set<WSContext>` of active connections in the HTTP module
- On open: add to set, log connection count
- On close: remove from set, log connection count
- Heartbeat: send `{"type":"ping"}` every 30 seconds; client is expected to handle (DaemonEventContext already parses all JSON messages)

**Broadcast helper:** `broadcast(event: WsEvent): void` -- serializes the event as JSON and sends to all active connections. Exported so other modules (index.ts) can call it.

**Event types:**

```typescript
interface WsEvent {
  type: "message.user" | "message.chunk" | "message.complete" | "message.typing" | "ping";
  channel: string;        // originating channel: "telegram", "dashboard"
  sender?: string;        // user identifier or "nova"
  messageId?: string;     // UUID for correlation
  content?: string;       // full content (message.user, message.complete)
  chunk?: string;         // text delta (message.chunk)
  timestamp: number;      // Unix ms
}
```

#### Scenario: Client connects with valid token

Given a WebSocket connection to `/ws/events?token=valid-token`,
when the connection opens,
then it is added to the active connections set,
and it begins receiving ping events every 30 seconds.

#### Scenario: Client connects without token

Given a WebSocket connection to `/ws/events` with no token parameter,
when the upgrade is attempted,
then the connection is closed with code 4001 and reason "unauthorized".

#### Scenario: Broadcast delivery

Given 3 active WebSocket connections,
when `broadcast({ type: "message.complete", ... })` is called,
then all 3 connections receive the serialized JSON event.

### Req-4: Bidirectional Streaming

Wire the WebSocket broadcast into both the Telegram and dashboard response paths so that activity on either channel is visible on both.

**Telegram -> Dashboard (Tier 3 streaming):**

In the Tier 3 job handler (`index.ts` lines 603-720), alongside the `TelegramStreamWriter` events:
- On job start: broadcast `{ type: "message.user", channel: "telegram", sender: msg.senderId, content: msg.content, messageId, timestamp }`
- On `text_delta`: broadcast `{ type: "message.chunk", channel: "telegram", sender: "nova", chunk: event.text, messageId, timestamp }`
- On `done`: broadcast `{ type: "message.complete", channel: "telegram", sender: "nova", content: finalResponse.text, messageId, timestamp }`

**Telegram -> Dashboard (Tier 1/2):**

After the Tier 1/2 response is sent to Telegram and persisted (Req-1):
- Broadcast `{ type: "message.user", channel: "telegram", sender: msg.senderId, content: msg.content, messageId, timestamp }`
- Broadcast `{ type: "message.complete", channel: "telegram", sender: "nova", content: responseText, messageId, timestamp }`

**Dashboard -> Telegram:**

In the `/chat` POST handler (`http.ts`):
- After the user message is constructed: broadcast `{ type: "message.user", channel: "dashboard", sender: "dashboard-user", content: userMessage, messageId, timestamp }`
- On each `text_delta` SSE event: broadcast `{ type: "message.chunk", channel: "dashboard", sender: "nova", chunk: event.text, messageId, timestamp }`
- On `done`: broadcast `{ type: "message.complete", channel: "dashboard", sender: "nova", content: fullText, messageId, timestamp }`
- Additionally on `done`: relay the complete response to Telegram via `telegram.sendMessage(config.telegramChatId, fullText)` with Markdown parsing (fallback to plain text). This requires passing the `telegram` adapter and `config.telegramChatId` into `createHttpApp` deps.

**Dashboard user messages -> Telegram:**

When a user sends a message via the dashboard, after the message is constructed in `http.ts`, send it to Telegram as well: `telegram.sendMessage(config.telegramChatId, "via Dashboard: {message}")`. This provides a chronological log in Telegram of dashboard activity.

#### Scenario: Telegram agent stream appears on dashboard

Given a Telegram user sends "check my calendar",
when the agent streams a response via TelegramStreamWriter,
then the dashboard receives `message.user` (the question), a series of `message.chunk` events (streaming text), and `message.complete` (full response),
and a connected dashboard chat page shows the exchange in real time.

#### Scenario: Dashboard response appears in Telegram

Given a dashboard user sends "what's the weather?",
when the agent completes its response via SSE,
then the complete response text is sent to Telegram via `sendMessage`,
and Telegram receives a message like the agent's response text with Markdown formatting.

#### Scenario: Dashboard user message appears in Telegram

Given a dashboard user sends "hello Nova",
then Telegram receives a message "via Dashboard: hello Nova".

### Req-5: Dashboard Chat Unified Stream

Update `apps/dashboard/app/chat/page.tsx` to subscribe to WebSocket message events so that ALL channels' activity appears in the chat view in real time.

**WebSocket subscription:**
- Import `useDaemonEvents` from `DaemonEventContext`
- Subscribe with filter `"message"` to receive `message.user`, `message.chunk`, `message.complete`, `message.typing`
- On `message.user`: append a new inbound `StoredMessage` to the messages array (with the event's `channel` and `sender`)
- On `message.chunk`: if no streaming bubble is active for this `messageId`, create one; append `chunk` to its accumulated text
- On `message.complete`: replace the streaming bubble with a finalized `StoredMessage`; clear streaming state
- Avoid duplicates: if the event's `channel` is `"dashboard"` and the message was sent by the current user, skip `message.user` (already added optimistically)

**Channel badges:**
- The `ChannelBadge` component already exists and is rendered per message
- Messages from Telegram will naturally show a blue "telegram" badge; dashboard messages show a green "dashboard" badge
- No component changes needed -- the channel field from WebSocket events flows through to `StoredMessage.channel`

**Streaming from other channels:**
- Extend the `StreamingBubble` component to accept an optional `channel` prop (defaults to "dashboard")
- When streaming text arrives from Telegram (via WebSocket), render a `StreamingBubble` with `channel="telegram"`

**Fallback on disconnect:**
- If `useDaemonEvents` returns status `"disconnected"` or `"reconnecting"`, show a subtle banner: "Live updates paused -- reconnecting..."
- On reconnection, call `loadHistory()` to catch up on messages missed during disconnection

#### Scenario: Telegram conversation appears live in dashboard

Given the dashboard chat page is open and WebSocket is connected,
when a Telegram user sends a message and Nova responds,
then the dashboard shows the user's message (with Telegram badge) and Nova's streaming response in real time,
without any page refresh or manual polling.

#### Scenario: Reconnection catch-up

Given the WebSocket disconnects for 30 seconds and then reconnects,
when reconnection succeeds,
then `loadHistory()` is called to fetch any messages exchanged during the outage,
and the messages array is updated without duplicating existing messages.

### Req-6: Chat History Unification

The `/api/chat/history` endpoint (`apps/dashboard/app/api/chat/history/route.ts`) already returns messages from all channels (no `WHERE channel = ...` filter). No changes needed to this endpoint.

Add a `type` field to `StoredMessage` in `apps/dashboard/types/api.ts` if not already present, to distinguish `"conversation"` messages from system events.

#### Scenario: History includes all channels

Given messages exist from both Telegram and dashboard,
when the chat page loads and calls `/api/chat/history`,
then all messages are returned sorted by time,
and each message has its original `channel` value for badge rendering.

## Scope
- **IN**: Tier 1/2 persistence fix, unified conversation context (remove channel filter), WebSocket `/ws/events` endpoint with auth and broadcast, bidirectional streaming (Telegram <-> dashboard), dashboard chat WebSocket subscription, streaming bubble for cross-channel responses
- **OUT**: Discord/Teams channel sync (future), message editing/deletion sync, read receipts, user-side typing indicators, end-to-end encryption, WebSocket event persistence (events are ephemeral -- missed events are caught up via history poll)

## Impact

| Area | Change |
|------|--------|
| `packages/daemon/src/index.ts` | MODIFY -- add saveExchange for Tier 1/2, add WS broadcast calls in Telegram message handler, pass telegram adapter to HTTP deps |
| `packages/daemon/src/http.ts` | MODIFY -- add `/ws/events` WebSocket upgrade handler, connection management, broadcast helper, add WS broadcasts in /chat handler, relay dashboard responses to Telegram |
| `packages/daemon/src/brain/conversation.ts` | MODIFY -- remove channel filter from loadHistory, update method signature |
| `packages/daemon/src/brain/agent.ts` | MODIFY -- update formatHistoryBlock to include channel provenance, update loadHistory call site |
| `packages/daemon/src/config.ts` | MODIFY -- add dashboardToken config field |
| `packages/daemon/package.json` | MODIFY -- add @hono/node-ws dependency |
| `apps/dashboard/app/chat/page.tsx` | MODIFY -- subscribe to WS message events, handle cross-channel streaming, reconnection catch-up |
| `apps/dashboard/app/api/chat/history/route.ts` | NO CHANGE -- already returns all channels |
| `apps/dashboard/components/providers/DaemonEventContext.tsx` | NO CHANGE -- already fully implemented |

## Risks

| Risk | Mitigation |
|------|-----------|
| Unified history includes noise from all channels | The agent sees full context, which is the goal; the system prompt already instructs Nova on conversational context usage. If context grows too large, `conversationHistoryDepth` config caps the message count. |
| WebSocket broadcast to many clients causes backpressure | Dashboard is single-user (homelab); even with 2-3 tabs, broadcast fan-out is negligible. Add a `try/catch` around each `ws.send()` to handle closed connections gracefully. |
| Telegram relay of dashboard responses causes double-notification | The relay message is Nova's response, not a notification. The user sees it as part of the conversation log. If intrusive, a config flag `relayDashboardToTelegram: boolean` (default true) can disable it. |
| `@hono/node-ws` compatibility with existing `@hono/node-server` | Both are official Hono packages designed to work together. The `injectWebSocket` pattern is documented in Hono's WebSocket guide. |
| Dashboard receives its own messages back via WebSocket | The deduplication logic in Req-5 skips `message.user` events where `channel === "dashboard"` and the sender matches the current user. For `message.complete`, the SSE stream already handles dashboard responses -- the WebSocket `message.complete` for `channel === "dashboard"` should also be skipped. |
| Race between SSE completion and WebSocket broadcast | Both originate from the same code path in `http.ts`. The SSE `done` event and WS broadcast happen sequentially in the same async function, so order is deterministic. |
