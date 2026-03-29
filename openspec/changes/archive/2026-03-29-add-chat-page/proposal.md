# Proposal: Add Chat Page

## Change ID
`add-chat-page`

## Summary

Add a `/chat` page to the Next.js dashboard for direct two-way conversation with Nova. The
existing `/messages` page remains as the read-only audit log. The chat page uses a direct daemon
HTTP channel as primary transport (SSE streaming), with Telegram relay as automatic fallback when
the daemon is unreachable. A unified timeline shows messages from all channels (Telegram, dashboard,
etc.) with per-bubble channel badges.

## Context

- Phase: Dashboard feature (post-v10 tool fleet)
- Dependencies: None — daemon and dashboard both exist and are deployed
- Key sources:
  - `packages/daemon/src/index.ts` — daemon entrypoint, no HTTP server currently
  - `packages/daemon/src/brain/agent.ts` — `NovaAgent.processMessage()`, streams via `query()` returning `AsyncIterable<SDKMessage>`
  - `packages/daemon/src/brain/conversation.ts` — `ConversationManager` with `loadHistory()` and `saveExchange()`
  - `packages/daemon/src/config.ts` — `Config` interface, `daemonPort: 7700`
  - `packages/daemon/src/types.ts` — `Message`, `Channel` types
  - `apps/dashboard/app/messages/page.tsx` — existing message audit log (688 lines)
  - `apps/dashboard/lib/api-client.ts` — `apiFetch()` with auth header injection
  - `apps/dashboard/types/api.ts` — canonical TS type location
  - `apps/dashboard/components/Sidebar.tsx` — nav registration via `NAV_GROUPS`
  - `packages/tools/channels-svc/src/server.ts` — Hono HTTP pattern used by fleet services

## Motivation

Nova's only interactive channel today is Telegram. The dashboard is read-only: it shows messages,
obligations, briefings, and sessions but cannot send messages. Adding a `/chat` page enables:

- Direct conversation from the dashboard without switching to Telegram
- SSE streaming for real-time response rendering (typing indicator + chunk-by-chunk bubbles)
- A unified timeline showing messages from ALL channels (Telegram, dashboard, CLI, etc.)
- Markdown rendering for richer response display than Telegram's 4096-char limit
- Fallback to Telegram relay when the daemon is down, ensuring the chat page always works

## Design

### Transport Architecture

**Primary: Direct Daemon Channel**

The daemon currently has NO HTTP server (removed in slim-daemon refactor). This spec re-adds a
lightweight Hono server on the configured `daemonPort` (7700) with only two routes:

- `POST /chat` — receives a user message, processes via `agent.processMessage()`, streams the
  response as SSE events
- `GET /health` — basic health check

The Hono server follows the same pattern used by the tool fleet microservices (channels-svc,
messages-svc, etc.) with `cors()` and `secureHeaders()` middleware.

**Fallback: Telegram Relay**

If the daemon's `POST /chat` returns 503, times out (10s connect timeout), or the connection fails:

1. Send the message via channels-svc `/send` endpoint (Telegram channel)
2. Poll `GET /api/messages` every 3s for Nova's response (filter by channel=telegram, sender=nova)
3. Show "Sent via Telegram -- waiting for response..." indicator
4. Auto-detect: try direct first, switch to relay on failure per-request

### SSE Streaming Protocol

`POST /chat` returns `Content-Type: text/event-stream`:

```
data: {"type":"chunk","text":"Here is"}\n\n
data: {"type":"chunk","text":" the first part"}\n\n
data: {"type":"chunk","text":" of my response."}\n\n
data: {"type":"done","full_text":"Here is the first part of my response."}\n\n
```

The daemon iterates the `AsyncIterable<SDKMessage>` from `query()`. On each `sdkMsg.type === "assistant"`
message, it extracts text blocks and emits `chunk` events. On `sdkMsg.type === "result"`, it emits
the `done` event with the full text and persists the exchange via `ConversationManager.saveExchange()`.

### Channel Key

Dashboard messages use the channel key `"dashboard:web"`, matching the existing pattern
`"telegram:<chatId>"`. This key is used for:

- Conversation history lookup via `ConversationManager.loadHistory("dashboard:web", ...)`
- Message persistence via `ConversationManager.saveExchange("dashboard:web", ...)`
- Channel badge rendering in the unified timeline

### Chat Page UI

Bubble layout:
- User messages: right-aligned, blue-tinted background
- Nova messages: left-aligned, dark gray background
- Small channel badge per bubble (Telegram, dashboard, etc.) using existing `channelAccentColor()`
- Markdown rendering for Nova responses (bold, italic, code blocks, lists)
- Typing indicator animation while SSE stream is active
- Auto-scroll to latest message on new messages
- Fixed message input bar at bottom with send button
- Load last 50 messages from `/api/messages` on mount (unified across all channels)

### Dashboard API Routes

Two new Next.js API routes proxy to the daemon:

- `POST /api/chat/send` — proxies request body to `daemon:7700/chat`, returns the SSE stream.
  Includes `Authorization` header if `DASHBOARD_TOKEN` is set. Falls back to channels-svc
  Telegram send on daemon failure.
- `GET /api/chat/history` — alias for `/api/messages?limit=50` with default params for the
  chat page's initial load.

### Sidebar Navigation

Add "Chat" entry to `NAV_GROUPS` in the Activity group, positioned between "Dashboard" (Overview)
and "Obligations" (Activity). Uses `MessageSquareText` icon from lucide-react to differentiate
from the existing "Messages" entry which uses `MessageSquare`.

## Requirements

### Req-1: Daemon HTTP Server

Add a lightweight Hono HTTP server to the daemon on `config.daemonPort` (7700):
- `GET /health` — returns `{ status: "ok", service: "nova-daemon", uptime_secs }`.
- `POST /chat` — accepts `{ message: string }` JSON body, returns SSE stream.
- CORS middleware allowing dashboard origin.
- Secure headers middleware.
- New file: `packages/daemon/src/http.ts`.
- Wired into `index.ts` startup, with graceful shutdown on SIGTERM/SIGINT.

### Req-2: SSE Streaming in POST /chat

The `/chat` endpoint:
- Constructs a `Message` object with `channel: "dashboard"`, `chatId: "dashboard:web"`,
  `senderId: "dashboard-user"`.
- Loads conversation history via `ConversationManager.loadHistory("dashboard:web", depth)`.
- Calls `agent.processMessage(msg, history)` — but instead of awaiting the full result, iterates
  the `query()` `AsyncIterable<SDKMessage>` directly to stream chunks.
- Emits SSE events: `{"type":"chunk","text":"..."}` for each text block from assistant messages.
- Emits `{"type":"done","full_text":"..."}` on result.
- Saves exchange via `ConversationManager.saveExchange()` after completion.
- Sends typing indicator concept: the SSE stream itself serves as the typing indicator (frontend
  shows typing animation while stream is active).
- On error: emits `{"type":"error","message":"..."}` and closes the stream.

### Req-3: NovaAgent Streaming Refactor

Currently `NovaAgent.processMessage()` awaits the full response before returning. To support
SSE streaming, add a new method:

```typescript
async *processMessageStream(
  message: Message,
  history: Message[],
): AsyncGenerator<{ type: "chunk"; text: string } | { type: "done"; response: AgentResponse }>
```

This method yields `chunk` events as text blocks arrive and a final `done` event with the full
`AgentResponse`. The existing `processMessage()` method is kept unchanged for Telegram usage.

### Req-4: Dashboard API Route — POST /api/chat/send

New file: `apps/dashboard/app/api/chat/send/route.ts`:
- Reads `DAEMON_URL` from environment (default `http://localhost:7700`).
- Proxies the request body to `${DAEMON_URL}/chat`.
- Streams the SSE response back to the client.
- On daemon failure (connection refused, 503, timeout): returns `503` with
  `{ error: "daemon_unavailable", fallback: "telegram" }` so the frontend can switch to relay.

### Req-5: Dashboard API Route — GET /api/chat/history

New file: `apps/dashboard/app/api/chat/history/route.ts`:
- Queries the messages table via Drizzle (same as `/api/messages`) with `limit=50`,
  ordered by `createdAt DESC`.
- Returns all channels (not filtered) so the unified timeline shows everything.
- Maps rows to `StoredMessage` shape matching the existing contract.

### Req-6: TypeScript Types

Add to `apps/dashboard/types/api.ts`:

```typescript
export interface ChatSendRequest {
  message: string;
}

export interface ChatSSEChunk {
  type: "chunk";
  text: string;
}

export interface ChatSSEDone {
  type: "done";
  full_text: string;
}

export interface ChatSSEError {
  type: "error";
  message: string;
}

export type ChatSSEEvent = ChatSSEChunk | ChatSSEDone | ChatSSEError;
```

### Req-7: Chat Page Component

New file: `apps/dashboard/app/chat/page.tsx`:
- State: `messages: StoredMessage[]`, `inputValue: string`, `sending: boolean`,
  `streamingText: string`, `error: string | null`, `transportMode: "direct" | "telegram"`.
- On mount: fetch `GET /api/chat/history` and populate `messages`.
- On send: POST to `/api/chat/send`, read SSE stream, append chunks to `streamingText` in
  real-time, on `done` event append the full message to `messages` array.
- Telegram fallback: on 503 from send route, POST to channels-svc via `/api/chat/send-telegram`
  (or inline fallback), poll `/api/messages` every 3s until Nova's response appears.
- Bubble layout: user messages right-aligned (blue-ish tint), Nova messages left-aligned (dark
  gray), channel badge per bubble using `channelAccentColor()`.
- Markdown rendering for Nova responses using a lightweight renderer (inline parsing for bold,
  italic, code, code blocks, lists).
- Typing indicator: pulsing dots animation shown while `sending === true` and SSE stream is active.
- Auto-scroll: `useRef` on the scroll container, scroll to bottom on new messages.
- Input bar: fixed at bottom, textarea with send button, disabled while sending.
- Small toggle/indicator showing current transport mode (direct vs telegram).

### Req-8: Markdown Renderer Utility

New file: `apps/dashboard/lib/markdown.tsx`:
- Export `MarkdownContent` React component that renders a markdown string as styled HTML.
- Supports: `**bold**`, `*italic*`, `` `inline code` ``, ```` ```code blocks``` ````, `- lists`.
- Uses Tailwind classes matching the dashboard design system (ds-gray tokens).
- No external markdown library — lightweight inline parser sufficient for chat responses.

### Req-9: Sidebar Navigation Update

In `apps/dashboard/components/Sidebar.tsx`:
- Add `MessageSquareText` to lucide-react imports.
- Add `{ to: "/chat", label: "Chat", icon: MessageSquareText }` to the Activity group in
  `NAV_GROUPS`, positioned as the first item in the Activity group (before Obligations).

### Req-10: Channel Type Extension

In `packages/daemon/src/types.ts`:
- Add `"dashboard"` to the `Channel` union type.

## Scope

**IN**: Chat page UI, daemon HTTP server with SSE streaming, NovaAgent streaming method, dashboard
API proxy routes, Telegram fallback mechanism, sidebar nav entry, channel type extension, markdown
renderer, message persistence, unified timeline with channel badges.

**OUT**: Voice messages, file uploads, message editing/deletion, multi-user chat, thread/reply UI,
emoji reactions, end-to-end encryption, push notifications, offline message queue.

## Impact

| Area | Change |
|------|--------|
| `packages/daemon/src/http.ts` | New: Hono HTTP server with `/health` and `POST /chat` SSE endpoint |
| `packages/daemon/src/brain/agent.ts` | Add `processMessageStream()` async generator method |
| `packages/daemon/src/index.ts` | Wire Hono server startup + graceful shutdown |
| `packages/daemon/src/types.ts` | Add `"dashboard"` to Channel union |
| `packages/daemon/package.json` | Add `hono` dependency |
| `apps/dashboard/app/chat/page.tsx` | New: chat page component with bubble layout + SSE streaming |
| `apps/dashboard/app/api/chat/send/route.ts` | New: proxy to daemon /chat with fallback |
| `apps/dashboard/app/api/chat/history/route.ts` | New: message history for chat page |
| `apps/dashboard/lib/markdown.tsx` | New: lightweight markdown renderer component |
| `apps/dashboard/types/api.ts` | Add ChatSendRequest, ChatSSEChunk, ChatSSEDone, ChatSSEError types |
| `apps/dashboard/components/Sidebar.tsx` | Add "Chat" nav entry in Activity group |

## Risks

| Risk | Mitigation |
|------|-----------|
| Daemon HTTP server conflicts with existing port usage | `daemonPort` (7700) is configured but currently unused (no HTTP server). Hono binds on startup; conflict detection via `EADDRINUSE` error handling. |
| SSE stream hangs if agent query stalls | 120s timeout on the SSE stream. If no chunks arrive within 30s, emit an error event and close. Agent SDK has its own timeout (maxTurns: 30). |
| Telegram fallback polling creates DB load | 3s poll interval with max 10 attempts (30s total). After 10 attempts, show "Response taking longer than expected" and stop polling. |
| Agent SDK `query()` not designed for streaming chunks | The `AsyncIterable<SDKMessage>` already yields assistant messages incrementally. `processMessageStream()` extracts text blocks from each assistant message as they arrive. |
| CORS on daemon HTTP server | Hono CORS middleware configured with dashboard origin. In dev, allows `localhost:*`. In prod, restricted to deployed dashboard URL. |
| Channel key collision with future channels | `"dashboard:web"` follows the `<channel>:<identifier>` pattern. Future dashboard instances could use `"dashboard:mobile"` etc. |
