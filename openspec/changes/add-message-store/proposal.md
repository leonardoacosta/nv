# Proposal: Add SQLite Message Store

## Change ID
`add-message-store`

## Summary

Persistent SQLite message log at `~/.nv/messages.db` that records all inbound and outbound
Telegram messages. Last N messages automatically injected into Claude's context before each
turn, eliminating the "I'm missing context" problem between stateless CLI invocations.

## Context
- Extends: `crates/nv-daemon/src/agent.rs` (context injection), `crates/nv-daemon/src/telegram/mod.rs` (message logging)
- Related: Existing `conversation_history` Vec in agent.rs (in-memory, lost on restart), `~/.nv/memory/conversations.md` (unreliable, depends on Claude writing it)

## Motivation

Each `claude -p` invocation is stateless. Conversation history is serialized into the prompt
but lost on daemon restart. Claude frequently says "I'm missing context" or "conversations.md
is empty" because it relies on itself to write conversation summaries — which it often forgets.

A SQLite message store provides:
1. **Guaranteed persistence** — every message logged by Rust, not by Claude
2. **Automatic context** — last N messages injected before each Claude call
3. **Queryable history** — agent can search past conversations via tool
4. **Zero token cost** — storage is free, only loaded messages cost tokens

## Requirements

### Req-1: Message Table

SQLite database at `~/.nv/messages.db` with schema:

```sql
CREATE TABLE messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL,
    direction TEXT NOT NULL,      -- 'inbound' | 'outbound'
    channel TEXT NOT NULL,        -- 'telegram' | 'discord' | 'teams' | 'cli'
    sender TEXT,                  -- username or 'nova'
    content TEXT NOT NULL,
    telegram_message_id INTEGER,
    trigger_type TEXT,            -- 'message' | 'cron' | 'nexus' | 'cli'
    response_time_ms INTEGER,    -- NULL for inbound, ms for outbound (time from trigger to send)
    tokens_in INTEGER,           -- NULL for inbound, token count for outbound
    tokens_out INTEGER           -- NULL for inbound, token count for outbound
);

CREATE INDEX idx_messages_timestamp ON messages(timestamp);
CREATE INDEX idx_messages_direction ON messages(direction);
```

### Req-2: Automatic Logging

- **Inbound**: logged in agent loop when trigger batch is drained (before Claude call)
- **Outbound**: logged after response is sent to Telegram (after send_message/edit_message)
- Logging is Rust-side, not Claude-side — guaranteed, no tool call needed

### Req-3: Context Injection

Before each Claude call, query last 20 messages and inject as `<recent_messages>` block
in the user message. Format:

```
<recent_messages>
[12:30] Leo: What projects am I working on?
[12:31] Nova: You have 14 active projects. OO and TC are highest priority...
[12:35] Leo: Can you check the TC issues?
[12:36] Nova: TC has 3 open issues...
</recent_messages>
```

This replaces the unreliable `conversation_history` Vec for cross-invocation context.

### Req-4: Agent Tool

`get_recent_messages(count)` tool — returns last N messages formatted for Claude.
Default count: 20. Max: 100. Used when Claude wants to review more history than
the auto-injected context provides.

## Scope
- **IN**: SQLite store, auto-logging, context injection, get_recent_messages tool, response time tracking, message volume stats, usage dashboard via CLI (`nv stats`)
- **OUT**: Full-text search, message editing/deletion, multi-channel message routing, topic frequency analysis (schema supports it later)

## Impact
| Area | Change |
|------|--------|
| `Cargo.toml` | Add `rusqlite` workspace dependency |
| `crates/nv-daemon/src/messages.rs` | New: MessageStore (init, log, query) |
| `crates/nv-daemon/src/agent.rs` | Inject recent messages into prompt context |
| `crates/nv-daemon/src/tools.rs` | Add get_recent_messages tool |
| `crates/nv-daemon/src/main.rs` | Init MessageStore, pass to AgentLoop |

## Risks
| Risk | Mitigation |
|------|-----------|
| DB file grows large | ~1KB per message, ~365KB/year at 1 msg/day. Not a concern. |
| SQLite write blocks async | Use `spawn_blocking` for writes, or sync (writes are <1ms) |
| Context injection too large | Cap at 20 messages, truncate content >500 chars per message |
