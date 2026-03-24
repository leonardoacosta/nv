# Proposal: Fix Conversation Amnesia

## Change ID
`fix-conversation-amnesia`

## Summary

Create a proper `ConversationStore` with session expiry and bounded history, wire it into the
worker loop so Nova retains multi-turn context, add tool result truncation, improve
`format_recent_for_context`, consolidate history constants, populate identity/user config files,
and cover everything with unit tests.

## Context
- Extends: `crates/nv-daemon/src/conversation.rs` (new file or existing stub)
- Extends: `crates/nv-daemon/src/worker.rs` (SharedDeps, Worker::run)
- Extends: `crates/nv-daemon/src/main.rs` (SharedDeps construction)
- Extends: `crates/nv-daemon/src/agent.rs` (constant cleanup)
- Extends: `crates/nv-daemon/src/messages.rs` (format_recent_for_context)
- Extends: `config/identity.md`, `config/user.md`
- Related: beads epic nv-4u1

## Motivation

Nova currently has no cross-invocation conversation memory at the Claude API level. Each worker
cycle starts with a blank message list, so Nova cannot reference anything the operator said in a
previous turn. This makes multi-turn conversations impossible — the operator must repeat context
every time.

The root causes:

1. **No persistent conversation store** — there is no mechanism to accumulate (user, assistant)
   turn pairs and feed them back into subsequent Claude API calls.
2. **Unbounded growth risk** — without turn count and character limits, naive history accumulation
   would blow the context window or spike API costs.
3. **Tool result bloat** — tool results (e.g., Jira search output, memory file contents) can be
   thousands of characters. Replaying them verbatim in history wastes context budget.
4. **Stale sessions** — after inactivity, old conversation turns become irrelevant noise. A session
   expiry mechanism is needed to clear history after a timeout.
5. **Incomplete identity/user config** — `config/identity.md` and `config/user.md` contain
   placeholder content that should be populated with real details so Nova's system context is
   accurate.
6. **History constants scattered** — constants like `MAX_HISTORY_TURNS` and `MAX_HISTORY_CHARS`
   belong in `conversation.rs` alongside the store, not spread across modules. Dead `#[allow(dead_code)]`
   attrs on moved constants should be cleaned up.

## Requirements

### Req-1: ConversationStore with Session Expiry and Bounds

Create `conversation.rs` with a `ConversationStore` struct that:
- Stores `(user_message, assistant_message)` turn pairs
- Enforces `MAX_HISTORY_TURNS` (20) and `MAX_HISTORY_CHARS` (50,000) bounds
- Implements session expiry: clears stored turns after `SESSION_TIMEOUT` (600s) of inactivity
- Exposes `push(user, assistant)` and `load() -> Vec<Message>` methods
- Resets the activity timer on both push and load

### Req-2: SharedDeps Registration

Add `conversation_store: Arc<Mutex<ConversationStore>>` to `SharedDeps` in `worker.rs`.
Construct the store in `main.rs` and pass it into the shared deps struct.

### Req-3: Worker Loop Integration

In `Worker::run`, before calling Claude:
- Lock the conversation store, call `load()` to get prior turns
- Prepend them to the current message list

After receiving Claude's response:
- Push the completed (user, assistant) turn pair to the store

### Req-4: Tool Result Truncation

In `ConversationStore::push`, truncate `tool_result` content blocks that exceed 1,000 characters.
Append `...[truncated]` to indicate truncation. This prevents tool output from dominating the
context window in subsequent turns.

### Req-5: format_recent_for_context Improvements

Bump the content truncation threshold in `format_recent_for_context` from 500 to 2,000 characters.
Add turn-pair grouping: wrap each (user, assistant) exchange in `--- turn ---` / `--- end turn ---`
markers so Claude can parse conversation structure.

### Req-6: Constant Consolidation

Move any history-related constants (`MAX_HISTORY_TURNS`, `MAX_HISTORY_CHARS`) that remain in
`agent.rs` into `conversation.rs`. Remove `#[allow(dead_code)]` attributes on constants that are
now actively used by the store.

### Req-7: Identity and User Config

Populate `config/identity.md` with Nova's full identity details (name, nature, operator, channel,
personality traits) — remove any placeholder text. Populate `config/user.md` with the actual
operator details (name, timezone, work context, communication preferences, decision patterns).

## Scope
- **IN**: ConversationStore, SharedDeps wiring, worker loop integration, tool result truncation,
  format_recent_for_context improvements, constant consolidation, config file population, unit tests
- **OUT**: Disk persistence of conversation history, cross-session memory, new tool definitions,
  changes to Claude API calling conventions

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/conversation.rs` | New module: ConversationStore struct, push/load/trim/expire, tool result truncation, constants |
| `crates/nv-daemon/src/worker.rs` | Add conversation_store to SharedDeps, load prior turns + push completed turns in Worker::run |
| `crates/nv-daemon/src/main.rs` | Construct ConversationStore, pass into SharedDeps |
| `crates/nv-daemon/src/agent.rs` | Remove relocated constants, clean up dead_code attrs |
| `crates/nv-daemon/src/messages.rs` | Bump truncation to 2000 chars, add turn-pair grouping |
| `config/identity.md` | Populate with full Nova identity details |
| `config/user.md` | Populate with actual operator details |

## Risks
| Risk | Mitigation |
|------|-----------|
| Context window overflow from long histories | Bounded by MAX_HISTORY_TURNS (20) and MAX_HISTORY_CHARS (50,000); trim drops oldest turns first |
| Stale context confusing Nova | SESSION_TIMEOUT (600s) auto-clears after inactivity; fresh sessions start clean |
| Tool result truncation loses important data | 1,000 char limit is generous for summaries; full results are still available via tool re-invocation |
| Mutex contention on ConversationStore | Workers are sequential per-task; lock hold time is microseconds for push/load |
