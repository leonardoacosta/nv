# Proposal: Fix Nova Amnesia

## Change ID
`fix-nova-amnesia`

## Summary

Populate hollow config stubs (`user.md`, `identity.md`) with actual operator details and add
two-layer conversation memory: enhanced MessageStore context (higher truncation, structured turns)
plus shared API-level conversation history across worker invocations with session expiry.

## Context
- Extends: `config/user.md`, `config/identity.md` (data-only), `crates/nv-daemon/src/worker.rs`
  (context injection + shared history), `crates/nv-daemon/src/messages.rs` (truncation limit)
- Related: `fix-chat-bugs` (worker.rs touch), `improve-chat-ux` (worker.rs touch — no overlap)
- Existing dead code: `MAX_HISTORY_TURNS: 20`, `MAX_HISTORY_CHARS: 50_000`, `SESSION_TIMEOUT: 600s`
  in `agent.rs` — designed for this but never wired

## Motivation

Nova asks onboarding questions every message because `user.md` and `identity.md` contain
"(discovered during bootstrap)" placeholders — Claude reads these each turn and concludes it's
a first session. Additionally, each worker creates a fresh `Vec<Message>` with only the current
user message, so Nova has no conversational continuity beyond the flat `<recent_messages>` log
(truncated to 500 chars per message, losing most of Nova's responses).

These two issues compound: hollow identity + no conversation memory = every turn feels like
meeting a stranger.

## Requirements

### Req-1: Populate Config Files

Fill `config/user.md` and `config/identity.md` with actual operator details. Remove all
"(discovered during bootstrap)" placeholders. This is a data change, not a code change.

### Req-2: Enhanced MessageStore Context

Raise the per-message truncation limit in `format_recent_for_context()` from 500 to 2000 chars
so Nova's full responses are preserved in the context window. Add turn-pair structure markers
(user/assistant grouping) to make the conversation flow clearer to Claude.

### Req-3: Shared API Conversation History

Add a `ConversationStore` to `SharedDeps` that persists actual Claude API message pairs
(user + assistant + tool calls) across worker invocations. Workers inject prior turns into their
`conversation_history` before calling Claude. Sessions expire after 10 minutes of inactivity
(reusing the existing `SESSION_TIMEOUT` constant). Bounded by `MAX_HISTORY_TURNS` (20) and
`MAX_HISTORY_CHARS` (50,000).

### Req-4: Worker Integration

Wire the `ConversationStore` into the worker's `run()` method:
1. Before building `conversation_history`, load recent turns from the store
2. Prepend prior turns, then append the current user message
3. After receiving Claude's response, push the complete turn (user + assistant + tool results)
   back to the store
4. Trim to bounds after each push

## Scope
- **IN**: config file population, MessageStore truncation bump, new ConversationStore struct,
  worker integration, session expiry
- **OUT**: cross-chat history (single Telegram chat), conversation summarization/compression,
  bootstrap flow changes, persistent disk-backed conversation history (memory for that stays
  in MessageStore)

## Impact
| Area | Change |
|------|--------|
| `config/user.md` | Replace placeholders with actual operator details |
| `config/identity.md` | Replace placeholders with actual identity details |
| `crates/nv-daemon/src/messages.rs` | Bump truncation 500→2000, add turn markers |
| `crates/nv-daemon/src/worker.rs` | Inject prior turns, push completed turns to store |
| `crates/nv-daemon/src/conversation.rs` | New file: `ConversationStore` with session expiry |
| `crates/nv-daemon/src/main.rs` | Add `ConversationStore` to `SharedDeps` |

## Risks
| Risk | Mitigation |
|------|-----------|
| Token cost increases with history depth | Bounded by MAX_HISTORY_CHARS (50K) — ~15% of context window |
| Stale context from expired sessions confuses Claude | 10min expiry clears history; fresh start on idle |
| Tool call content bloats history | Strip tool result content beyond 1000 chars when storing |
| Race condition on shared store | Single Mutex — workers are sequential in practice (pool size 1-2) |
