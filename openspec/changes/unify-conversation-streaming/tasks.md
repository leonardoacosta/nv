# Implementation Tasks

<!-- beads:epic:nv-wcm8 -->

## API Batch 1: Persistence + Unified Context

- [x] [1.1] [P-1] Add `saveExchange` call for Tier 1/2 responses in `packages/daemon/src/index.ts` -- after Telegram sendMessage and writeEntry, call `conversationManager.saveExchange(channelKey, msg, assistantMsg)` with constructed assistant Message (senderId: "nova", content: responseText) [owner:api-engineer] [beads:nv-qvtg]
- [x] [1.2] [P-1] Remove channel filter from `ConversationManager.loadHistory()` in `packages/daemon/src/brain/conversation.ts` -- change query from `WHERE channel = $1 ORDER BY created_at DESC LIMIT $2` to `ORDER BY created_at DESC LIMIT $1`, update method signature from `(channelId, limit)` to `(limit)` [owner:api-engineer] [beads:nv-lczv]
- [x] [1.3] [P-1] Update `formatHistoryBlock()` in `packages/daemon/src/brain/agent.ts` -- change line format from `[sender] (timestamp): content` to `[sender via channel] (timestamp): content` using `msg.channel` [owner:api-engineer] [beads:nv-55ku]
- [x] [1.4] [P-2] Update loadHistory call site in `packages/daemon/src/index.ts` line 609 -- change from `loadHistory(channelKey, depth)` to `loadHistory(depth)` [owner:api-engineer] [beads:nv-jhc2]
- [x] [1.5] [P-2] Update loadHistory call site in `packages/daemon/src/http.ts` line 111 -- change from `loadHistory("dashboard:web", depth)` to `loadHistory(depth)` [owner:api-engineer] [beads:nv-qmu7]
- [x] [1.6] [P-2] Add `dashboardToken` field to config in `packages/daemon/src/config.ts` -- source from `DASHBOARD_TOKEN` env var, optional (WebSocket auth disabled if unset) [owner:api-engineer] [beads:nv-wul1]

## API Batch 2: WebSocket Event Endpoint

- [x] [2.1] [P-1] Add `@hono/node-ws` to `packages/daemon/package.json` dependencies [owner:api-engineer] [beads:nv-pk4p]
- [x] [2.2] [P-1] Set up `createNodeWebSocket()` in `packages/daemon/src/index.ts` -- create the node-ws adapter, inject WebSocket middleware into the Hono app, and pass the `injectWebSocket` result to `httpServer` after `serve()` [owner:api-engineer] [beads:nv-hdx0]
- [x] [2.3] [P-1] Add `/ws/events` upgrade handler in `packages/daemon/src/http.ts` -- validate `token` query param against `config.dashboardToken` (close with 4001 if invalid), add connection to `Set<WSContext>`, remove on close, log connection count changes [owner:api-engineer] [beads:nv-pnzu]
- [x] [2.4] [P-1] Implement `broadcast(event: WsEvent)` helper in `packages/daemon/src/http.ts` -- serialize event as JSON, iterate active connections set, send to each with try/catch per connection, export the function [owner:api-engineer] [beads:nv-p4p4]
- [x] [2.5] [P-2] Add heartbeat interval in WebSocket handler -- send `{"type":"ping"}` every 30 seconds to all active connections, clear interval when last connection closes [owner:api-engineer] [beads:nv-7nnj]
- [x] [2.6] [P-2] Define `WsEvent` type in `packages/daemon/src/http.ts` -- type union for message.user, message.chunk, message.complete, message.typing, ping with fields: channel, sender, messageId, content, chunk, timestamp [owner:api-engineer] [beads:nv-y7eq]

## API Batch 3: Bidirectional Streaming

- [x] [3.1] [P-1] Add `telegram` adapter and `telegramChatId` to `HttpServerDeps` interface in `packages/daemon/src/http.ts` -- pass from `index.ts` when calling `createHttpApp()` [owner:api-engineer] [beads:nv-6xqb]
- [x] [3.2] [P-1] Broadcast `message.user` + `message.chunk` + `message.complete` events from Telegram Tier 3 job handler in `packages/daemon/src/index.ts` -- emit message.user on job start, message.chunk on each text_delta, message.complete on done [owner:api-engineer] [beads:nv-z84u]
- [x] [3.3] [P-1] Broadcast `message.user` + `message.complete` events from Telegram Tier 1/2 handler in `packages/daemon/src/index.ts` -- emit after saveExchange call [owner:api-engineer] [beads:nv-u9nk]
- [x] [3.4] [P-1] Broadcast `message.user` + `message.chunk` + `message.complete` events from dashboard `/chat` POST handler in `packages/daemon/src/http.ts` -- emit message.user when request arrives, message.chunk on each text_delta SSE, message.complete on done [owner:api-engineer] [beads:nv-1g5i]
- [x] [3.5] [P-2] Relay dashboard agent responses to Telegram in `/chat` POST handler -- on done event, call `telegram.sendMessage(config.telegramChatId, fullText, { parseMode: "Markdown" })` with plain-text fallback on Markdown failure [owner:api-engineer] [beads:nv-3w42]
- [x] [3.6] [P-2] Relay dashboard user messages to Telegram in `/chat` POST handler -- after constructing user Message, call `telegram.sendMessage(config.telegramChatId, "via Dashboard: {message}")` [owner:api-engineer] [beads:nv-layd]
- [x] [3.7] [P-2] Generate stable `messageId` (UUID) for each conversation exchange in both Telegram and dashboard paths -- use `crypto.randomUUID()`, pass through all broadcast events for client-side correlation [owner:api-engineer] [beads:nv-3v2p]

## UI Batch: Dashboard Chat Unified Stream

- [ ] [4.1] [P-1] Subscribe to WebSocket message events in `apps/dashboard/app/chat/page.tsx` -- import and call `useDaemonEvents` with filter `"message"`, handle message.user, message.chunk, message.complete event types [owner:ui-engineer] [beads:nv-9z0i]
- [ ] [4.2] [P-1] Handle `message.user` events -- append new inbound StoredMessage to messages array with event's channel and sender; skip if channel is "dashboard" and sender matches current user (already optimistically added) [owner:ui-engineer] [beads:nv-71vh]
- [ ] [4.3] [P-1] Handle `message.chunk` events -- track streaming state per messageId; accumulate chunks; render StreamingBubble for cross-channel streams [owner:ui-engineer] [beads:nv-g99x]
- [ ] [4.4] [P-1] Handle `message.complete` events -- replace streaming bubble with finalized StoredMessage; clear streaming state for that messageId; skip if channel is "dashboard" (SSE already handles) [owner:ui-engineer] [beads:nv-hvid]
- [ ] [4.5] [P-2] Extend `StreamingBubble` component to accept optional `channel` prop -- pass through to `ChannelBadge` so cross-channel streams show correct badge (e.g. "telegram" badge for Telegram-originated streams) [owner:ui-engineer] [beads:nv-um6r]
- [ ] [4.6] [P-2] Add reconnection catch-up logic -- when `useDaemonEvents` status transitions from "reconnecting" to "connected", call `loadHistory()` and merge results with existing messages (deduplicate by timestamp + content) [owner:ui-engineer] [beads:nv-yysk]
- [ ] [4.7] [P-2] Show disconnection banner -- when WS status is "reconnecting" or "disconnected", render a subtle top banner: "Live updates paused -- reconnecting..." [owner:ui-engineer] [beads:nv-37xl]

## E2E: Typecheck + Verification

- [ ] [5.1] [P-1] `pnpm --filter daemon exec tsc --noEmit` passes -- all daemon TypeScript changes compile without errors [owner:api-engineer] [beads:nv-lswg]
- [ ] [5.2] [P-1] `pnpm --filter dashboard exec tsc --noEmit` passes -- all dashboard TypeScript changes compile without errors [owner:ui-engineer] [beads:nv-gvpr]
- [ ] [5.3] [P-2] `pnpm build` passes -- full monorepo build succeeds [owner:api-engineer] [beads:nv-ig62]
- [ ] [5.4] [P-2] [user] Manual: send a message via Telegram, verify it appears in dashboard chat in real time via WebSocket (with Telegram badge) [owner:leo] [beads:nv-7s3y]
- [ ] [5.5] [P-2] [user] Manual: send a message via dashboard, verify Nova's response appears in Telegram and the dashboard simultaneously [owner:leo] [beads:nv-o2p7]
- [ ] [5.6] [P-2] [user] Manual: send a message via Telegram that triggers Tier 1 keyword route, verify the exchange appears in `/api/chat/history` and in the dashboard chat view [owner:leo] [beads:nv-4wjd]
- [ ] [5.7] [P-3] [user] Manual: disconnect dashboard WebSocket (e.g. stop daemon briefly), exchange messages via Telegram, restart daemon, verify dashboard reconnects and shows missed messages after catch-up [owner:leo] [beads:nv-cq51]
