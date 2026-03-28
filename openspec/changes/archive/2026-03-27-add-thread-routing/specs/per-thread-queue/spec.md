# Per-Thread Job Queue

## MODIFIED Requirements

### Requirement: Thread-keyed queue routing

The `JobQueue` MUST route jobs into per-thread sub-queues keyed by `threadId`. Each sub-queue SHALL enforce strict serial execution (one running job at a time). Different sub-queues MUST process in parallel, bounded by the global `concurrency` limit.

#### Scenario: Two messages in different threads
Given global concurrency = 3, thread "t1" has 0 running, thread "t2" has 0 running
When job A (thread t1) and job B (thread t2) are enqueued
Then both start immediately (parallel across threads)

#### Scenario: Two messages in the same thread
Given global concurrency = 3, thread "t1" has 1 running job
When job B (thread t1) is enqueued
Then job B waits until the running job in t1 completes

#### Scenario: Global concurrency exhausted
Given global concurrency = 3, all 3 slots occupied by threads t1, t2, t3
When job D (new thread t4) is enqueued
Then job D waits until any global slot frees up

### Requirement: Accurate per-thread queue position

`getStatus` MUST return the position within the thread's sub-queue, not the global queue. Position SHALL be captured atomically at enqueue time (before `tryStart`) to eliminate the race condition.

#### Scenario: First message in a new thread
Given thread "t1" has no queued or running jobs
When a job is enqueued for "t1"
Then enqueue returns startedImmediately true with position 0

#### Scenario: Second message in a busy thread
Given thread "t1" has 1 running job
When a second job is enqueued for "t1"
Then enqueue returns startedImmediately false with position 1 (1 job ahead)

#### Scenario: Message in a new thread while global concurrency full
Given threads t1, t2, t3 each have 1 running job (global concurrency = 3)
When a job is enqueued for new thread "t4"
Then enqueue returns startedImmediately false with position 0 and threadState "global-full"

### Requirement: Queue ack messages

The acknowledgment message MUST reflect the thread-aware queue state. Silent start when job begins immediately, thread-specific position when queued behind same-thread jobs, global-full message when all concurrency slots are occupied.

#### Scenario: Ack for same-thread queue
Given thread "t1" has 1 running job
When a second job is enqueued for "t1"
Then the user receives "Processing your previous message. This one is next."

#### Scenario: Ack for global-full queue
Given all 3 concurrency slots are occupied by different threads
When a new job is enqueued
Then the user receives "All workers busy. You're next when one frees up."

## ADDED Requirements

### Requirement: Job type extension

The `Job` interface MUST add a `threadId: string` field. The `EnqueueOpts` interface MUST add `threadId: string` and `replyToMessageId: number | undefined`.

#### Scenario: Job created with thread context
Given an enqueue call with threadId "telegram:123:10"
When the job is created
Then job.threadId is "telegram:123:10"

### Requirement: Thread sub-queue cleanup

When all jobs in a thread complete and no new jobs arrive within 60 seconds, the thread's sub-queue MUST be removed from memory. The ThreadResolver cache entry SHALL persist.

#### Scenario: Thread sub-queue cleanup after idle
Given thread "t1" has 0 queued and 0 running jobs
When 60 seconds pass with no new jobs for "t1"
Then the sub-queue for "t1" is removed from the per-thread map
