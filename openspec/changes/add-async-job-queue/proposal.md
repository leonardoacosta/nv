# Proposal: Add Async Job Queue

## Change ID
`add-async-job-queue`

## Summary
Replace the blocking inline message handler in the daemon with an in-memory job queue. Users get an immediate acknowledgment, results are delivered asynchronously. Multiple messages can be queued while the agent is busy. Users can cancel running tasks with natural-language cancel phrases.

## Context
- Depends on: none (modifies daemon internals only)
- Conflicts with: none (the message handler in `index.ts` is self-contained)
- Roadmap: Wave 2 (daemon reliability)
- Current handler: `packages/daemon/src/index.ts` lines 257-350, `void (async () => { ... })()`

## Motivation
The current message handler is blocking per-chat. When a user sends a message, the daemon immediately fires off `agent.processMessage()` in a fire-and-forget async IIFE. If the user sends another message while the agent is working, a second parallel agent call starts. This creates three problems:

1. **Rate limiting** -- Vercel AI Gateway rate-limits concurrent sessions. Two parallel `query()` calls are safe, but three or more hit 429s and fail silently.
2. **No feedback** -- The user has no idea whether their message is being processed or queued. The only signal is a typing indicator that may or may not be visible.
3. **No cancellation** -- If the user sends "never mind" or "cancel", there is no way to stop the in-flight agent call. The agent finishes, sends a stale response, and the user ignores it.

A bounded job queue with concurrency control, priority levels, and cancel detection solves all three. The queue is in-memory only -- acceptable for a single-user daemon where jobs are short-lived (10-60s) and restart loss is tolerable.

## Requirements

### Req-1: JobQueue class
Create `packages/daemon/src/queue/job-queue.ts` with a `JobQueue` class that manages a bounded queue of jobs.

- **Concurrency**: configurable, default 2. At most N jobs run simultaneously.
- **Max queue size**: configurable, default 20. Reject new jobs when the queue is full.
- **Enqueue**: accepts a job definition (chatId, content, priority, handler function), returns a `Job` object. If the queue has capacity, start immediately; otherwise queue and return position.
- **Priority**: `high` > `normal` > `low`. Higher-priority jobs jump ahead in the waiting queue. Within the same priority, FIFO order.
- **Cancel**: `cancel(jobId)` aborts a running or queued job via its `AbortController`. Returns boolean (true if job existed).
- **CancelByChatId**: `cancelByChatId(chatId)` cancels all jobs for a given chat. Used for cancel-phrase detection.
- **Status**: `getStatus(chatId)` returns the number of jobs ahead of the caller's oldest queued job, or `running` if their job is active, or `idle` if no jobs for that chat.
- **Events**: Jobs emit lifecycle events (`enqueued`, `started`, `completed`, `failed`, `cancelled`) via an EventEmitter. The daemon subscribes to these for typing indicator management and logging.

### Req-2: Job and type definitions
Create `packages/daemon/src/queue/types.ts`:

- `JobPriority`: `"high" | "normal" | "low"`
- `JobStatus`: `"queued" | "running" | "completed" | "failed" | "cancelled"`
- `Job`: `{ id: string; chatId: string; content: string; priority: JobPriority; status: JobStatus; abortController: AbortController; createdAt: Date; startedAt?: Date; completedAt?: Date; error?: string }`
- `JobEvent`: `{ type: "enqueued" | "started" | "completed" | "failed" | "cancelled"; job: Job; queueDepth: number }`
- `QueueConfig`: `{ concurrency: number; maxQueueSize: number }`

### Req-3: Barrel export
Create `packages/daemon/src/queue/index.ts` that re-exports `JobQueue`, all types, and `QueueConfig`.

### Req-4: Integrate queue into message routing
Modify `packages/daemon/src/index.ts`:

- Instantiate `JobQueue` after agent creation with config-sourced concurrency and maxQueueSize.
- Replace the inline `void (async () => { ... })()` block (lines 257-350) with queue dispatch:
  1. Check for cancel phrases (`cancel`, `stop`, `never mind`, `nvm`, case-insensitive). If matched, call `queue.cancelByChatId(msg.chatId)` and send a confirmation message.
  2. Determine priority: `/brief` and `/dream` commands get `low`, user messages get `normal`. (P0 alert routing is future work.)
  3. Call `queue.enqueue(...)` with a handler function that wraps the existing agent call (load history, processMessage, save exchange, send response chunks).
  4. On enqueue, send an acknowledgment if the job is queued (not immediately started): `"Queued (N ahead). I'll respond when ready."` If it starts immediately, send only the typing indicator (existing behavior).
- Subscribe to queue events for typing indicator lifecycle: start typing on `started`, stop on `completed`/`failed`/`cancelled`.
- Wire queue shutdown into the graceful shutdown handler (`queue.drain()` or `queue.cancelAll()`).

### Req-5: Config additions
Modify `packages/daemon/src/config.ts`:

- Add `QueueConfig` to the `Config` interface: `queue: QueueConfig`.
- Source from `[queue]` section in TOML or env vars `NV_QUEUE_CONCURRENCY` / `NV_QUEUE_MAX_SIZE`.
- Defaults: `concurrency: 2`, `maxQueueSize: 20`.

Add `QueueConfig` parsing to `TomlConfig` interface and `loadConfig()`.

### Req-6: TOML config section
Add a `[queue]` section to `config/nv.toml`:

```toml
[queue]
concurrency = 2
max_queue_size = 20
```

### Req-7: AbortController propagation
Each job gets its own `AbortController`. The handler function receives `signal: AbortSignal` and checks `signal.aborted` before sending results. Note: the Agent SDK `query()` does NOT accept `AbortSignal` -- cancellation only prevents sending the result to Telegram, not the underlying API call. This is a known limitation documented in the code.

## Scope
- **IN**: JobQueue class, types, cancel detection, priority ordering, queue status messages, config additions, typing indicator lifecycle tied to jobs, graceful shutdown integration
- **OUT**: Persistent queue (Redis/DB), distributed queue, P0 alert priority routing, Agent SDK abort support, multi-user queue fairness, retry logic for failed jobs, dashboard queue visibility

## Impact
| Area | Change |
|------|--------|
| `packages/daemon/src/queue/types.ts` | NEW -- Job, JobPriority, JobStatus, JobEvent, QueueConfig types |
| `packages/daemon/src/queue/job-queue.ts` | NEW -- JobQueue class with enqueue, cancel, cancelByChatId, getStatus, drain |
| `packages/daemon/src/queue/index.ts` | NEW -- barrel export |
| `packages/daemon/src/index.ts` | MODIFY -- replace inline async handler with queue dispatch, add cancel detection, wire queue events to typing indicators, add queue to shutdown |
| `packages/daemon/src/config.ts` | MODIFY -- add `queue: QueueConfig` to Config, add `[queue]` TOML parsing |
| `config/nv.toml` | MODIFY -- add `[queue]` section |

## Risks
| Risk | Mitigation |
|------|-----------|
| Agent SDK `query()` ignores AbortSignal | Cancel only prevents result delivery, not the API call. Document in code. The API call completes and tokens are consumed. Acceptable for single-user. |
| Queue full during burst input | maxQueueSize=20 is generous for single-user. Reject with a clear message: "Queue full, try again in a moment." |
| Typing indicator desync | Tie typing start/stop to job events, not message receipt. Clear interval on any terminal event (completed/failed/cancelled). |
| Priority inversion (low-priority job blocks queue slot) | Concurrency=2 means at most 2 slots occupied. Low-priority jobs only start when a slot is free. High-priority jobs preempt in the waiting queue, not running jobs. |
| Restart loses in-flight jobs | Acceptable for single-user daemon. Jobs are 10-60s. Graceful shutdown drains or cancels. |
