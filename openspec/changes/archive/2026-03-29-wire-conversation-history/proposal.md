# Proposal: Wire Conversation History

## Change ID
`wire-conversation-history`

## Summary

Wire the existing `ConversationManager` into the daemon's agent loop so Nova maintains conversation context between messages within the same Telegram chat.

## Context
- Extends: `packages/daemon/src/index.ts` (daemon entry, message routing), `packages/daemon/src/brain/agent.ts` (NovaAgent)
- Uses: `packages/daemon/src/brain/conversation.ts` (ConversationManager — already implemented, not wired in)
- Schema: `packages/db/src/schema/messages.ts` (messages table — `id`, `channel`, `sender`, `content`, `metadata`, `created_at`, `embedding`)
- Related: archived spec `conversation-manager` (loadHistory + saveExchange — implemented), `openspec/specs/conversation-manager/spec.md`

## Motivation

Nova currently processes every message statelessly. The `processMessage` call in `index.ts` passes an empty array `[]` as history, and `agent.processMessage` ignores the `_history` parameter entirely. The `ConversationManager` class exists with working `loadHistory` and `saveExchange` methods, but nothing calls them. This means Nova cannot reference anything said earlier in a conversation — every message is a fresh start.

1. **No continuity** — users must re-explain context in every message. Nova cannot follow up on previous exchanges or recall what was just discussed.
2. **ConversationManager unused** — the class was built and tested in a prior spec but never integrated into the message processing pipeline.
3. **Channel column mismatch** — `ConversationManager.loadHistory` queries by `channel` column which stores `"telegram"` for all chats. Per-chat history requires querying by chat ID, which is currently only stored on the in-memory `Message` type (`chatId` field), not in the `messages` table. The `channel` column must carry the chat-level identifier (e.g., `telegram:123456789`) or the ConversationManager must be updated to query differently.

## Requirements

### Req-1: Per-Chat Channel Key

Store messages with a chat-level channel identifier by composing `channel:chatId` (e.g., `telegram:123456789`) as the `channel` column value in the `messages` table. This allows `ConversationManager.loadHistory` to retrieve per-chat history without schema changes. Update `ConversationManager.saveExchange` to use the composite key when persisting.

### Req-2: Load History Before Agent Call

Before each agent call in the message routing handler, call `ConversationManager.loadHistory(channelKey, limit)` to fetch the last N messages for the chat. The history depth defaults to 20 messages and is configurable via `NV_HISTORY_DEPTH` env var or `conversation.history_depth` in `nv.toml`.

### Req-3: Inject History into Agent Prompt

Format loaded conversation history as a structured block prepended to the system prompt:

```
<conversation_history>
[user] (2026-03-26 10:00): What's on my calendar today?
[nova] (2026-03-26 10:00): You have a standup at 10:30 and a 1:1 at 2pm.
[user] (2026-03-26 10:05): Reschedule the 1:1 to 3pm.
</conversation_history>
```

The history block is appended to the existing system prompt content so the agent sees both its persona instructions and the conversation context.

### Req-4: Save Exchange After Agent Call

After a successful agent response, call `ConversationManager.saveExchange(channelKey, userMsg, assistantMsg)` to persist both the user's message and Nova's reply. This runs fire-and-forget (like diary writes) so it does not block the Telegram response.

### Req-5: Configurable History Depth

Add `conversation.history_depth` to the TOML config and `NV_HISTORY_DEPTH` env var (env takes precedence). Default: 20. The `Config` interface gets a `conversationHistoryDepth: number` field.

## Scope
- **IN**: Wiring ConversationManager into index.ts message handler, formatting history for agent prompt, per-chat channel key, config for history depth, saving exchanges after responses
- **OUT**: Semantic/vector search over history, message summarization/compression, multi-channel thread linking, changes to the messages DB schema (no new columns — reuse `channel`), changes to the Agent SDK query mechanism

## Impact
| Area | Change |
|------|--------|
| `packages/daemon/src/index.ts` | Modified: instantiate ConversationManager, load history before agent call, save exchange after response |
| `packages/daemon/src/brain/agent.ts` | Modified: accept history messages, format and inject into system prompt |
| `packages/daemon/src/brain/conversation.ts` | Modified: update to use composite channel key in saveExchange |
| `packages/daemon/src/config.ts` | Modified: add conversationHistoryDepth field + TOML/env parsing |

## Risks
| Risk | Mitigation |
|------|-----------|
| History query adds latency to every message | Query is a simple indexed SELECT with LIMIT 20 on `channel` column — sub-5ms on small tables; monitor and add index if needed |
| Composite channel key breaks existing loadHistory callers | ConversationManager is not called from anywhere else currently; the only caller will be the new wiring code |
| System prompt grows large with 20 messages of history | 20 messages is ~2-4K tokens typically; well within Sonnet's 200K context; configurable via NV_HISTORY_DEPTH if needed |
| Conflicts with slim-daemon spec (same wave) | Both touch `packages/daemon/src/index.ts`; this spec's changes are additive (3 new lines in the message handler) and should merge cleanly regardless of order |
