# Implementation Tasks

<!-- beads:epic:TBD -->

## DB Batch

(No schema changes -- reuses existing `messages` table with composite `channel` key)

## API Batch

- [ ] [2.1] [P-1] Add `conversationHistoryDepth` to Config interface and parse from `conversation.history_depth` in TOML / `NV_HISTORY_DEPTH` env var (default: 20) in `packages/daemon/src/config.ts` [owner:api-engineer]
- [ ] [2.2] [P-1] Update `ConversationManager.saveExchange` to accept and use composite channel key (`telegram:chatId`) instead of bare channel name; update `rowToMessage` if needed for the composite key in `packages/daemon/src/brain/conversation.ts` [owner:api-engineer]
- [ ] [2.3] [P-1] Add `formatHistoryBlock(messages: Message[]): string` function to `packages/daemon/src/brain/agent.ts` that renders message history as a `<conversation_history>` block with `[sender] (timestamp): content` lines [owner:api-engineer]
- [ ] [2.4] [P-1] Update `NovaAgent.processMessage` to accept history messages and append the formatted history block to the system prompt before the agent SDK `query` call in `packages/daemon/src/brain/agent.ts` [owner:api-engineer]
- [ ] [2.5] [P-1] Wire ConversationManager into message routing in `packages/daemon/src/index.ts`: instantiate with pool, load history before agent call using composite `telegram:chatId` key, pass history to `agent.processMessage`, save exchange fire-and-forget after successful response [owner:api-engineer]

## UI Batch

(No UI tasks -- daemon-only change)

## E2E Batch

- [ ] [4.1] Send 3 sequential messages to Nova via Telegram where message 2 and 3 reference content from earlier messages; verify Nova's responses demonstrate awareness of prior conversation context [owner:e2e-engineer]
- [ ] [4.2] Verify `messages` table contains rows with composite channel keys (e.g., `telegram:123456789`) after a conversation exchange [owner:e2e-engineer]
