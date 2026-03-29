# Implementation Tasks

## API Batch: Daemon HTTP Server

- [x] [1.1] [P-1] Add `hono` and `@hono/node-server` dependencies to `packages/daemon/package.json` [owner:api-engineer]
- [x] [1.2] [P-1] Add `"dashboard"` to the `Channel` union type in `packages/daemon/src/types.ts` [owner:api-engineer]
- [x] [1.3] [P-1] Create `packages/daemon/src/http.ts` — export `createHttpServer()` function that creates a Hono app with `cors()` and `secureHeaders()` middleware, following the pattern in `packages/tools/channels-svc/src/server.ts` [owner:api-engineer]
- [x] [1.4] [P-1] Add `GET /health` route in `http.ts` — returns `{ status: "ok", service: "nova-daemon", uptime_secs: number }` [owner:api-engineer]
- [x] [1.5] [P-1] Add `POST /chat` route in `http.ts` — accepts `{ message: string }` JSON body, returns `Content-Type: text/event-stream` SSE response; constructs a `Message` object with `channel: "dashboard"`, `chatId: "dashboard:web"`, `senderId: "dashboard-user"`; loads conversation history via `ConversationManager.loadHistory("dashboard:web", config.conversationHistoryDepth)`; iterates `processMessageStream()` async generator; emits `{"type":"chunk","text":"..."}` for each text chunk; emits `{"type":"done","full_text":"..."}` on completion; saves exchange via `ConversationManager.saveExchange()` [owner:api-engineer]
- [x] [1.6] [P-1] Add `processMessageStream()` async generator method to `NovaAgent` in `packages/daemon/src/brain/agent.ts` — yields `{ type: "chunk", text: string }` events from assistant message text blocks as they arrive from `query()` `AsyncIterable<SDKMessage>`, yields `{ type: "done", response: AgentResponse }` on result; handles tool calls and token counting same as existing `processMessage()`; fires diary entry on completion [owner:api-engineer]
- [x] [1.7] [P-2] Wire Hono server startup in `packages/daemon/src/index.ts` — call `createHttpServer()` passing `agent`, `conversationManager`, and `config`; start listening on `config.daemonPort` (7700); add server close to the `shutdown()` function [owner:api-engineer]
- [x] [1.8] [P-2] Add SSE error handling in `POST /chat` — on agent processing error, emit `{"type":"error","message":"..."}` event and close stream; add 120s overall timeout; add 30s inactivity timeout (no chunks emitted) [owner:api-engineer]
- [x] [1.9] [P-3] Add request logging in `POST /chat` — log message receipt (content length, content preview) and completion (stop reason, tool call count, latency) using existing `createLogger` [owner:api-engineer]

## API Batch: Dashboard API Routes

- [x] [2.1] [P-1] Create `apps/dashboard/app/api/chat/send/route.ts` — POST handler that reads `DAEMON_URL` from env (default `http://localhost:7700`), proxies request body to `${DAEMON_URL}/chat`, streams SSE response back to client; on daemon failure (connection refused, 503, timeout >10s), returns 503 with `{ error: "daemon_unavailable", fallback: "telegram" }` [owner:api-engineer]
- [x] [2.2] [P-1] Create `apps/dashboard/app/api/chat/history/route.ts` — GET handler that queries messages table via Drizzle with `limit=50`, ordered by `createdAt DESC`, returns all channels (no filter); maps rows to `StoredMessage` shape matching existing `/api/messages` contract [owner:api-engineer]
- [x] [2.3] [P-1] Add chat-related types to `apps/dashboard/types/api.ts` — `ChatSendRequest` (`message: string`), `ChatSSEChunk` (`type: "chunk"`, `text: string`), `ChatSSEDone` (`type: "done"`, `full_text: string`), `ChatSSEError` (`type: "error"`, `message: string`), `ChatSSEEvent` (union of the three) [owner:api-engineer]

## UI Batch: Chat Page + Sidebar

- [x] [3.1] [P-1] Create `apps/dashboard/lib/markdown.tsx` — export `MarkdownContent` component that renders a markdown string as styled React elements; supports `**bold**`, `*italic*`, `` `inline code` ``, ```` ```code blocks``` ````, `- lists`; uses Tailwind ds-gray design tokens; no external library [owner:ui-engineer]
- [x] [3.2] [P-1] Create `apps/dashboard/app/chat/page.tsx` — "use client" page with state: `messages: StoredMessage[]`, `inputValue: string`, `sending: boolean`, `streamingText: string`, `error: string | null`, `transportMode: "direct" | "telegram"` [owner:ui-engineer]
- [x] [3.3] [P-1] Implement initial message load — `useEffect` on mount fetches `GET /api/chat/history`, populates `messages` state; shows loading skeleton (8 pulse placeholders) while fetching; shows `ErrorBanner` on failure with retry [owner:ui-engineer]
- [x] [3.4] [P-1] Implement bubble layout — user messages right-aligned with blue-tinted background (`bg-blue-900/20`), Nova messages left-aligned with dark gray background (`bg-ds-gray-200`); each bubble shows sender label, timestamp, and small channel badge using `channelAccentColor()` from `@/lib/channel-colors`; Nova bubbles render content via `MarkdownContent` component [owner:ui-engineer]
- [x] [3.5] [P-1] Implement message send with SSE streaming — on form submit, POST to `/api/chat/send` with `{ message: inputValue }`; read SSE stream using `EventSource` or `fetch` + `ReadableStream` reader; parse each `data:` line as JSON; append `chunk.text` to `streamingText` in real-time; on `done` event, append full message to `messages` array and clear `streamingText`; on `error` event, show error in `ErrorBanner` [owner:ui-engineer]
- [x] [3.6] [P-1] Implement typing indicator — show pulsing dots animation in a left-aligned Nova bubble while `sending === true` and SSE stream is active; remove when `done` or `error` event arrives; the streaming text appears inside this bubble, replacing the dots once the first chunk arrives [owner:ui-engineer]
- [x] [3.7] [P-1] Implement auto-scroll — `useRef` on the scroll container div; scroll to bottom on mount, on new message appended to `messages`, and on each `streamingText` update; use `scrollIntoView({ behavior: "smooth" })` [owner:ui-engineer]
- [x] [3.8] [P-1] Implement input bar — fixed at bottom of the chat area; textarea (auto-growing, max 4 lines) with send button; Enter sends (Shift+Enter for newline); disabled while `sending === true`; clear input after successful send [owner:ui-engineer]
- [x] [3.9] [P-2] Implement Telegram fallback — on 503 from `/api/chat/send`, set `transportMode` to `"telegram"`; send message via `apiFetch("/api/chat/send-telegram", { method: "POST", body })` or use channels-svc endpoint; poll `GET /api/messages?channel=telegram&limit=5` every 3s up to 10 attempts; when Nova's response appears (sender=nova, newer than send timestamp), append to messages; show "Sent via Telegram -- waiting for response..." indicator during polling [owner:ui-engineer]
- [x] [3.10] [P-2] Implement transport mode indicator — small badge in the input bar area showing current mode: "Direct" (green dot) or "Telegram" (blue dot with Telegram icon); auto-switches based on last request result; optional manual toggle button [owner:ui-engineer]
- [x] [3.11] [P-2] Wrap page in `PageShell` with title "Chat" and subtitle "Talk to Nova directly" [owner:ui-engineer]
- [x] [3.12] [P-1] Update sidebar navigation — in `apps/dashboard/components/Sidebar.tsx`, import `MessageSquareText` from lucide-react; add `{ to: "/chat", label: "Chat", icon: MessageSquareText }` as the first item in the Activity group of `NAV_GROUPS` (before Obligations) [owner:ui-engineer]

## E2E Batch: Verification

- [x] [4.1] TypeScript compilation: `pnpm --filter @nova/daemon typecheck` passes with no errors [owner:api-engineer]
- [x] [4.2] TypeScript compilation: `pnpm --filter nova-dashboard typecheck` passes with no errors [owner:ui-engineer]
- [x] [4.3] Dashboard build: `pnpm --filter dashboard build` — TS compilation passes ("Compiled successfully"); page data collection fails due to missing DATABASE_URL at build time (pre-existing env issue, not a code regression) [owner:ui-engineer]
- [ ] [4.4] [user] Manual smoke: start daemon, navigate to `/chat`, verify message history loads from all channels with channel badges
- [ ] [4.5] [user] Manual smoke: send a message via the chat input, verify SSE streaming renders chunks in real-time in a Nova bubble with typing indicator
- [ ] [4.6] [user] Manual smoke: verify Nova's response renders with markdown formatting (bold, code blocks, lists)
- [ ] [4.7] [user] Manual smoke: stop the daemon, send a message, verify Telegram fallback activates with "Sent via Telegram" indicator
- [ ] [4.8] [user] Manual smoke: verify "Chat" appears in sidebar Activity group and navigates to `/chat`
- [ ] [4.9] [user] Manual smoke: verify auto-scroll works when receiving a long streaming response
