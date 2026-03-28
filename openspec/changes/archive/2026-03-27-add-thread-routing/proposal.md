# Proposal: Quote-Based Thread Routing

## Change ID
`add-thread-routing`

## Summary
Use Telegram's `reply_to_message` metadata to create per-thread queues that enable parallel processing across independent conversation threads while maintaining strict serial ordering within each thread. Nova responses always reply-to the original message, creating visual thread chains users can quote to continue.

## Context
- Extends: `packages/daemon/src/queue/job-queue.ts`, `packages/daemon/src/channels/telegram.ts`, `packages/daemon/src/channels/stream-writer.ts`, `packages/daemon/src/index.ts`, `packages/db/src/schema/messages.ts`, `packages/daemon/src/brain/conversation.ts`
- Related: `add-async-job-queue` (completed — current JobQueue), `add-smart-routing` (completed — tier routing cascade)
- Depends on: conversation history in Postgres `messages` table

## Motivation
The current JobQueue is a flat global FIFO with a concurrency limit of 2. All messages compete for the same slots regardless of conversational relationship. This causes two problems:

1. **Misleading queue position** — "Queued (0 ahead)" always shows 0 because `getStatus()` runs after `tryStart()` has already moved the job from waiting to running. The position is racey and semantically wrong (conflates "your previous job is running" with "nothing ahead").

2. **No parallelism across conversations** — If a user sends three independent messages (about different topics), they serialize through the same 2-slot bottleneck. But if those messages are in different threads (unquoted = new thread), they could safely process in parallel since each thread's context is independent.

Telegram's `reply_to_message` provides a natural, user-understood threading signal. Users already quote messages to continue a topic. By capturing this metadata, the queue can route per-thread: serial within a thread (preserving ordering), parallel across threads (maximizing throughput).

## Requirements

### Req-1: Thread metadata in message normalization
Capture `reply_to_message.message_id` during Telegram message normalization and derive a `threadId` by walking the reply chain to the root message. Unquoted messages start a new thread (threadId = their own messageId).

### Req-2: Thread-aware DB schema
Add `thread_id` and `reply_to_message_id` columns to the `messages` table for queryable per-thread history and efficient indexing.

### Req-3: Per-thread job queue
Replace the flat global FIFO with per-thread queues. Each thread gets strict serial execution (one job at a time). Different threads process in parallel, bounded by a global concurrency limit. Queue position is reported per-thread.

### Req-4: Reply-to on all Nova responses
Nova's responses always set `reply_to_message_id` pointing at the user's original message. This creates visual threading in Telegram and makes it easy for users to quote Nova's response for follow-up.

## Scope
- **IN**: Thread metadata capture, per-thread queue routing, reply-to on responses, DB schema migration, queue position fix, conversation history scoped to thread
- **OUT**: Cross-channel threading (Discord/Teams — future), thread-aware smart routing (Tier 1/2 bypass), thread summarization, thread archival

## Impact
| Area | Change |
|------|--------|
| `packages/daemon/src/channels/telegram.ts` | Capture `reply_to_message` in normalization |
| `packages/daemon/src/types.ts` | Add `threadId`, `replyToMessageId` to Message |
| `packages/daemon/src/queue/job-queue.ts` | Per-thread FIFO with parallel across threads |
| `packages/daemon/src/queue/types.ts` | Add `threadId` to Job |
| `packages/daemon/src/channels/stream-writer.ts` | Accept and use `replyToMessageId` |
| `packages/daemon/src/index.ts` | Pass thread context through routing, fix queue ack |
| `packages/daemon/src/brain/conversation.ts` | Thread-scoped history loading |
| `packages/db/src/schema/messages.ts` | Add `thread_id`, `reply_to_message_id` columns |

## Risks
| Risk | Mitigation |
|------|-----------|
| Reply chain walk could be expensive for deep threads | Cache thread roots in-memory (Map<messageId, threadId>); chains rarely exceed 20 messages |
| Global concurrency could be exhausted by many parallel threads | Keep global concurrency cap (default 3); per-thread serial is within that budget |
| Existing messages have no thread_id | Null-safe: existing messages get `thread_id = NULL`, treated as legacy single-thread |
| Telegram message_id is per-chat, not globally unique | Thread key is `chatId:threadRootMessageId`, globally unique |
