# Proposal: Improve Chat UX

## Change ID
`improve-chat-ux`

## Summary

Bundle four Telegram chat UX improvements: reply threading so every response maps to its trigger
message, typing indicator while workers process, long-task confirmation for operations estimated
over one minute, and a quiet hours config window that suppresses non-P0 outbound during
configurable nighttime hours.

## Context
- Extends: `crates/nv-daemon/src/telegram/client.rs` (sendChatAction, reply_to_message_id), `crates/nv-daemon/src/worker.rs` (typing + confirmation before Claude call), `crates/nv-daemon/src/orchestrator.rs` (quiet hours gate)
- Related: PRD section 7.1, existing `send_message` already accepts `reply_to: Option<String>`, `WorkerTask` already carries `telegram_message_id`
- Depends on: `fix-chat-bugs` (spec 1) — bugs must be resolved before UX polish

## Motivation

Nova's Telegram responses arrive as standalone messages disconnected from the question that
triggered them. During long operations the user sees no feedback. At night, non-urgent digests
and session events interrupt sleep. These four changes collectively make the chat feel responsive,
traceable, and respectful of downtime.

## Requirements

### Req-1: Reply Threading

All outbound messages sent in response to a user trigger MUST use `reply_to_message_id` set to the
original Telegram message ID. This maps every response to its trigger in the Telegram UI.

- Worker already receives `telegram_message_id` in `WorkerTask`
- Pass this through to every `send_message` call in the worker response path
- Digest and cron-triggered messages have no trigger message — send without reply_to (unchanged)

### Req-2: Typing Indicator

Call `sendChatAction(chat_id, "typing")` immediately when a worker picks up a task, before any
Claude API call. This shows the "typing..." bubble in Telegram within ~100ms of the user's message.

- Add `send_chat_action(chat_id, action)` method to `TelegramClient`
- Call it as the first step in the worker's `process_task()` before building the prompt
- Fire-and-forget — do not block on the result, log errors at warn level

### Req-3: Long-Task Confirmation

When the orchestrator estimates a task will take >1 minute (based on trigger classification or
explicit tool hints), send a confirmation message before dispatching to the worker pool:

```
"This will take ~2min. Searching Jira across all projects. Be right back."
```

- Classification heuristic: multi-project queries, `/apply` commands, full digest generation
- Message format: estimated time + what's happening + "Be right back."
- Sent via `send_message` with `reply_to_message_id` (Req-1)

### Req-4: Quiet Hours Config

Add `quiet_start` and `quiet_end` fields to `DaemonConfig`:

```toml
[daemon]
quiet_start = "23:00"
quiet_end = "07:00"
```

During the quiet window, suppress all outbound messages except those classified as P0 (Priority::High
in the worker pool). P0 messages always get through.

- Parse as `NaiveTime` in config
- Check in orchestrator before dispatching non-High-priority tasks
- Queued tasks are held until quiet window ends, then dispatched in order
- If no quiet hours configured, all messages pass through (backwards compatible)

## Scope
- **IN**: reply_to_message_id on all worker responses, sendChatAction typing, long-task confirmation heuristic, quiet hours config + gate in orchestrator
- **OUT**: Per-user quiet hours (single user), read receipts, message scheduling UI, custom typing indicators

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/telegram/client.rs` | Add `send_chat_action()` method |
| `crates/nv-daemon/src/worker.rs` | Call typing indicator on task pickup, thread reply_to through response path |
| `crates/nv-daemon/src/orchestrator.rs` | Long-task confirmation heuristic, quiet hours gate |
| `crates/nv-core/src/config.rs` | Add `quiet_start`, `quiet_end` to DaemonConfig |

## Risks
| Risk | Mitigation |
|------|-----------|
| reply_to_message_id on deleted messages fails | Telegram silently ignores invalid reply_to — no error handling needed |
| Typing indicator adds latency | Fire-and-forget async call, ~5ms, non-blocking |
| Long-task estimate inaccurate | Heuristic only — better to over-estimate than under-estimate |
| Quiet hours timezone confusion | Use system local time (single user, single machine) |
