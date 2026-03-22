# Implementation Tasks

<!-- beads:epic:TBD -->

## Telegram Reactions

- [x] [1.1] [P-1] Add set_message_reaction(chat_id, message_id, emoji) to TelegramClient — POST /setMessageReaction [owner:api-engineer]
- [x] [1.2] [P-1] Add remove_message_reaction(chat_id, message_id) to TelegramClient [owner:api-engineer]
- [x] [1.3] [P-2] Remove thinking message ("...") logic from agent.rs — replaced by reactions [owner:api-engineer]
- [x] [1.4] [P-2] Remove thinking ticker (60s update loop) — replaced by status updates [owner:api-engineer]

## Orchestrator

- [x] [2.1] [P-1] Create crates/nv-daemon/src/orchestrator.rs — Orchestrator struct with trigger receiver, worker pool, telegram client [owner:api-engineer]
- [x] [2.2] [P-1] Implement classify_trigger() — fast classification (Command/Query/Chat/Digest) without AI, based on keywords + trigger type [owner:api-engineer]
- [x] [2.3] [P-2] Implement run() loop — recv trigger, react 👀, classify, dispatch or respond inline for Chat [owner:api-engineer]
- [ ] [2.4] [P-2] [deferred] Implement status_update() — send brief Telegram message if worker >30s, e.g. "Searching Jira..." [owner:api-engineer]

## Worker Pool

- [x] [3.1] [P-1] Create crates/nv-daemon/src/worker.rs — Worker struct wrapping PersistentSession + task context [owner:api-engineer]
- [x] [3.2] [P-1] Create WorkerPool struct — max_concurrent (default 3), active workers, priority queue [owner:api-engineer]
- [x] [3.3] [P-2] Implement dispatch() — create Worker, inject context (SQLite recent messages + memory + task), spawn as tokio task [owner:api-engineer]
- [x] [3.4] [P-2] Implement on_worker_complete() — send response to Telegram, react ✅, log to diary + SQLite, release pool slot [owner:api-engineer]
- [x] [3.5] [P-2] Implement on_worker_error() — react ❌, send error summary, release pool slot [owner:api-engineer]
- [x] [3.6] [P-3] Implement priority queue — High jumps ahead of Normal, Low handled inline [owner:api-engineer]

## Agent Refactor

- [x] [4.1] [P-1] Extract tool execution logic from agent.rs into worker.rs (tool use loop, memory loading, Jira/Nexus calls) [owner:api-engineer]
- [x] [4.2] [P-2] Refactor agent.rs to become orchestrator — remove blocking Claude calls, delegate to WorkerPool [owner:api-engineer]
- [x] [4.3] [P-2] Update main.rs — create WorkerPool, pass to Orchestrator instead of AgentLoop [owner:api-engineer]
- [x] [4.4] [P-3] Update config — add max_workers (default 3), priority_keywords to agent config [owner:api-engineer]

## Verify

- [x] [5.1] cargo build passes [owner:api-engineer]
- [x] [5.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [x] [5.3] cargo test — orchestrator tests (classify, dispatch, priority queue) + worker tests (context injection, completion, error) + existing tests pass [owner:api-engineer]
- [ ] [5.4] [user] Manual test: send message on Telegram → see 👀 reaction → response arrives → ✅ reaction [owner:api-engineer]
- [ ] [5.5] [user] Manual test: send 3 messages rapidly → all get 👀 → responses arrive independently [owner:api-engineer]
