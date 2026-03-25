# Proposal: Fix Cold-Start Memory

## Change ID
`fix-cold-start-memory`

## Summary

Inject recent outbound messages into the cold-start system prompt so Nova has context of its
own recent conversation.

## Context
- Extends: `crates/nv-daemon/src/worker.rs` (query MessageStore, pass recent context)
- Extends: `crates/nv-daemon/src/claude.rs` (accept and prepend recent messages to system prompt)
- Extends: `crates/nv-daemon/src/messages.rs` (add get_recent_outbound if not present)

## Motivation

Each cold-start is amnesiac. Nova sends a digest with "ACT-1 through ACT-5", user replies
"implement ACT-1-5", Nova has no idea what that refers to. Requires 3 messages to recover
context.

The ConversationStore is in-memory only (10min timeout), and digests bypass it entirely.
MessageStore (SQLite) already persists outbound messages — it just isn't consulted at
cold-start time.

## Requirements

### Req-1: get_recent_outbound on MessageStore

Add `get_recent_outbound(limit: usize) -> Vec<OutboundMessage>` to `MessageStore` in
`messages.rs` if the method does not already exist. Query the messages table for the most
recent N outbound rows, ordered by timestamp descending.

### Req-2: Query recent context in worker.rs

Before building the conversation payload for a cold-start Claude call, query MessageStore
for the last 10 outbound messages. Pass the result as an optional `&str` to the Claude
send function.

### Req-3: Prepend to system prompt in claude.rs

In `send_messages_cold_start_with_image()`, accept an optional `recent_context: Option<&str>`
parameter. When present, prepend a "Your recent messages to Leo:" section to the system
prompt before the main instructions. Format each message as a timestamped line:

```
[HH:MM] Nova: {message_preview}
```

Limit each preview to 200 characters. Omit the section entirely if the slice is empty.

## Scope
- **IN**: `get_recent_outbound`, worker query, system prompt prepend
- **OUT**: Persistent multi-turn session (separate spec), ConversationStore refactor, new DB schema

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/messages.rs` | Add `get_recent_outbound(limit)` if missing |
| `crates/nv-daemon/src/worker.rs` | Query MessageStore before cold-start call, pass context |
| `crates/nv-daemon/src/claude.rs` | Accept `recent_context` param, prepend to system prompt |

## Risks
| Risk | Mitigation |
|------|-----------|
| Adds ~1-2KB to system prompt per call | Fixed limit of 10 messages; previews capped at 200 chars |
| Digest cold-starts now include prior digest content | Acceptable — digest context aids follow-up interpretation |
| MessageStore unavailable at query time | Return empty slice, proceed without context (no crash) |
