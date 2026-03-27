# Implementation Tasks

<!-- beads:epic:pending -->

## DB Batch

(no database changes)

## API Batch

### Group A: Types and queue core

- [ ] [2.1] Create `packages/daemon/src/queue/types.ts` -- define `JobPriority` (`"high" | "normal" | "low"`), `JobStatus` (`"queued" | "running" | "completed" | "failed" | "cancelled"`), `Job` interface (id, chatId, content, priority, status, abortController, handler, createdAt, startedAt?, completedAt?, error?), `JobEvent` interface (type, job, queueDepth), and `QueueConfig` interface (concurrency, maxQueueSize). [owner:api-engineer]
- [ ] [2.2] Create `packages/daemon/src/queue/job-queue.ts` -- implement `JobQueue` class extending `EventEmitter`. Constructor takes `QueueConfig`. Methods: `enqueue(opts: { chatId, content, priority, handler })` returns `Job` and starts immediately if under concurrency limit or queues with position; `cancel(jobId)` aborts via AbortController and removes from queue or marks running job cancelled; `cancelByChatId(chatId)` cancels all jobs for a chat; `getStatus(chatId)` returns `{ state: "idle" | "running" | "queued"; position?: number }`; `drain()` cancels all queued jobs and waits for running jobs to finish. Priority ordering: high > normal > low, FIFO within same priority. Emit `JobEvent` on each state transition. [owner:api-engineer]
- [ ] [2.3] Create `packages/daemon/src/queue/index.ts` -- barrel export: re-export `JobQueue` from `./job-queue.js`, re-export all types from `./types.js`. [owner:api-engineer]

### Group B: Config

- [ ] [2.4] Add queue config to `packages/daemon/src/config.ts` -- add `queue: QueueConfig` to `Config` interface (import `QueueConfig` from `./queue/index.js`). Add `queue?` section to `TomlConfig` interface with `concurrency?: number` and `max_queue_size?: number`. In `loadConfig()`, parse `NV_QUEUE_CONCURRENCY` / `NV_QUEUE_MAX_SIZE` env vars and `toml.queue?.concurrency` / `toml.queue?.max_queue_size` with defaults `concurrency: 2`, `maxQueueSize: 20`. [owner:api-engineer]
- [ ] [2.5] Add `[queue]` section to `config/nv.toml` with `concurrency = 2` and `max_queue_size = 20`. Place after `[dream]` section. [owner:api-engineer]

### Group C: Integration

- [ ] [2.6] Modify `packages/daemon/src/index.ts` -- replace the inline `void (async () => { ... })()` message handler (lines 257-350) with queue-based dispatch. Instantiate `JobQueue` after agent creation using `config.queue`. The handler function passed to `enqueue()` wraps the existing logic: load conversation history, call `agent.processMessage()`, save exchange, split and send response chunks. The handler receives `signal: AbortSignal` and checks `signal.aborted` before sending each chunk. On enqueue, if job is queued (not immediately running), send ack message: `"Queued (N ahead). I'll respond when ready."` [owner:api-engineer]
- [ ] [2.7] Add cancel-phrase detection to message routing in `packages/daemon/src/index.ts` -- before enqueueing, check if `msg.text` matches cancel phrases (`/^(cancel|stop|never mind|nvm)$/i`). If matched, call `queue.cancelByChatId(msg.chatId)`, send confirmation (`"Cancelled."` or `"Nothing to cancel."` if no jobs), and return early. [owner:api-engineer]
- [ ] [2.8] Wire queue events to typing indicator lifecycle in `packages/daemon/src/index.ts` -- subscribe to `queue.on("started", ...)` to begin typing interval for that job's chatId, subscribe to `queue.on("completed" | "failed" | "cancelled", ...)` to clear the typing interval. Remove the per-message typing interval from the old handler. [owner:api-engineer]
- [ ] [2.9] Add queue to graceful shutdown in `packages/daemon/src/index.ts` -- in the `shutdown()` function, call `queue.drain()` before `pool.end()`. Log number of cancelled/drained jobs. [owner:api-engineer]

## UI Batch

(no UI changes)

## E2E Batch

- [ ] [4.1] Verify daemon compiles after changes -- `pnpm typecheck` passes with no errors in `packages/daemon`. No dangling imports, no unused variables, no type mismatches in the queue integration. [owner:e2e-engineer]
