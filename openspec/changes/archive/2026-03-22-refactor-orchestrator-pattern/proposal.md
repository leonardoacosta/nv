# Proposal: Refactor Agent Loop to Orchestrator Pattern

## Change ID
`refactor-orchestrator-pattern`

## Summary

Replace the blocking single-threaded agent loop with a non-blocking orchestrator that dispatches
Claude workers in the background, uses Telegram reactions as read receipts, and supports parallel
message processing with a priority queue.

## Context
- Extends: `crates/nv-daemon/src/agent.rs` (major refactor), `crates/nv-daemon/src/telegram/client.rs` (add reactions), `crates/nv-daemon/src/main.rs`
- Related: persistent-claude-session (workers reuse persistent session pattern), all channel specs (benefit from non-blocking)

## Motivation

The current agent loop blocks on every Claude call (2-14s depending on cold/warm start). While
one message processes, all others queue silently. With 6+ channels feeding messages, this creates
a bottleneck. The "thinking..." indicator helps but doesn't solve the fundamental problem: Nova
can only think about one thing at a time.

The orchestrator pattern makes Nova instantly responsive:
1. Acknowledge every message immediately (Telegram reaction)
2. Dispatch Claude workers for actual processing (background)
3. Stream status updates as workers complete
4. Handle multiple concurrent tasks

## Requirements

### Req-1: Telegram Read Receipts via Reactions

Replace the "..." thinking message with Telegram message reactions:

| Stage | Reaction | Meaning |
|-------|----------|---------|
| Received | 👀 | Message seen by orchestrator |
| Processing | ⏳ | Worker dispatched, Claude thinking |
| Complete | ✅ | Response sent |
| Error | ❌ | Worker failed |

Use `setMessageReaction` API (Telegram Bot API 7.3+).

### Req-2: Non-Blocking Orchestrator

The orchestrator runs in the agent loop but NEVER blocks on Claude:

```
recv(trigger) → classify → react(👀) → dispatch_worker() → loop back immediately
```

Classification (fast, no AI needed):
- **Command** ("create issue", "assign") → Priority::High
- **Query** ("what's blocking", "status of") → Priority::Normal
- **Chat** ("thanks", "ok") → Priority::Low (respond inline, no worker)
- **Digest** (cron) → Priority::Normal

### Req-3: Worker Pool

Managed pool of Claude subprocess workers:

```rust
struct WorkerPool {
    max_concurrent: usize,  // default 3
    active: Vec<WorkerHandle>,
    queue: PriorityQueue<Task>,
}
```

- Workers reuse the `PersistentSession` pattern from persistent-claude-session
- Each worker gets the task context + recent messages from SQLite
- Workers run as spawned tokio tasks
- On completion: send response to Telegram, react ✅, log to diary + SQLite

### Req-4: Priority Queue

Messages are prioritized before dispatch:

| Priority | Examples | Behavior |
|----------|----------|----------|
| High | P0 alerts, direct commands, urgent keywords | Jump queue, dispatch immediately |
| Normal | Queries, digest triggers, channel messages | FIFO within priority |
| Low | Chat, acknowledgments, "thanks" | Respond inline, no worker needed |

### Req-5: Status Updates

While workers process, send periodic Telegram updates:
- React ⏳ when worker starts
- If >30s: send brief status ("Searching Jira...")
- On complete: react ✅ + send response
- On error: react ❌ + send error summary

### Req-6: Context Sharing

Workers are isolated sessions but share context via:
- SQLite message store (last 20 messages auto-injected)
- Memory files (markdown, same as current)
- PendingAction state (prevent conflicting Jira writes)

## Scope
- **IN**: Orchestrator refactor, reactions, worker pool, priority queue, status updates
- **OUT**: Multi-model routing (haiku for triage), cross-worker conversation threading, worker-to-worker communication

## Impact
| Area | Change |
|------|--------|
| `agent.rs` | Major refactor: split into orchestrator.rs + worker.rs |
| `telegram/client.rs` | Add set_message_reaction() |
| `main.rs` | Create WorkerPool, pass to orchestrator |
| `claude.rs` | Workers create own PersistentSession instances |
| `config.rs` | Add max_workers, priority keywords config |

## Risks
| Risk | Mitigation |
|------|-----------|
| Race conditions on Jira writes | PendingAction confirmation prevents concurrent writes |
| Memory: 3 workers × 500MB = 1.5GB | MemoryMax=4G in systemd, pool limit enforced |
| Context fragmentation | SQLite message store provides shared history |
| Complexity increase | Orchestrator is simpler than current agent loop (no tool use loop) |
