# Design: Quote-Based Thread Routing

## Architecture

```
Telegram message (with optional reply_to_message)
  │
  ▼
normalizeTextMessage() ─── captures replyToMessageId
  │
  ▼
ThreadResolver.resolve(chatId, replyToMessageId?)
  │
  ├─ replyToMessageId present → walk chain to root (cache + DB fallback)
  │                              threadId = "chatId:rootMessageId"
  │
  └─ no replyToMessageId    → new thread
                               threadId = "chatId:thisMessageId"
  │
  ▼
JobQueue.enqueue({ ..., threadId })
  │
  ├─ Per-thread sub-queue lookup (Map<threadId, ThreadQueue>)
  │   └─ ThreadQueue: waiting[] + runningJob?
  │
  ├─ Can start? = thread has no running job AND global slots available
  │   ├─ YES → start immediately, return { startedImmediately: true }
  │   └─ NO  → push to thread's waiting[], return { startedImmediately: false, position }
  │
  ▼
Handler executes with TelegramStreamWriter(adapter, chatId, replyToMessageId)
  │
  ├─ loadHistory(channelKey, depth, threadId) → thread-scoped context
  ├─ agent.processMessageStream() → streaming response
  └─ finalize() → sends with reply_to_message_id
```

## Thread Resolution Strategy

The `ThreadResolver` is a lightweight in-memory cache with DB fallback:

```typescript
class ThreadResolver {
  // messageId → threadRootMessageId (per-chat scope)
  private cache = new Map<string, number>();  // key: "chatId:messageId"

  async resolve(chatId: string, messageId: number, replyToMessageId?: number): Promise<string> {
    if (!replyToMessageId) {
      // No quote = new thread root
      this.cache.set(`${chatId}:${messageId}`, messageId);
      return `${chatId}:${messageId}`;
    }

    const cacheKey = `${chatId}:${replyToMessageId}`;
    const cached = this.cache.get(cacheKey);
    if (cached !== undefined) {
      this.cache.set(`${chatId}:${messageId}`, cached);
      return `${chatId}:${cached}`;
    }

    // DB walk: follow reply_to_message_id chain until NULL
    const root = await this.walkChain(chatId, replyToMessageId);
    this.cache.set(cacheKey, root);
    this.cache.set(`${chatId}:${messageId}`, root);
    return `${chatId}:${root}`;
  }
}
```

Cache never needs eviction — entries are `string → number` (~50 bytes each). 10,000 messages = ~500KB.

## Per-Thread Queue Design

The queue refactor replaces the flat `waiting: Job[]` with a `Map<threadId, ThreadQueue>`:

```typescript
interface ThreadQueue {
  waiting: Job[];
  running: Job | null;  // strict serial: at most 1
  lastActivityAt: number;
}

class JobQueue {
  private threads = new Map<string, ThreadQueue>();
  private globalRunning = 0;
  private readonly globalConcurrency: number;  // default 3

  enqueue(opts: EnqueueOpts): EnqueueResult {
    const tq = this.getOrCreateThread(opts.threadId);
    const job = createJob(opts);

    // Capture position BEFORE tryStart (fixes race condition)
    const position = tq.waiting.length;

    tq.waiting.push(job);
    const started = this.tryStart(opts.threadId);

    return {
      job,
      startedImmediately: started,
      position: started ? 0 : position + (tq.running ? 1 : 0),
      threadState: tq.running ? "thread-busy" : "global-full",
    };
  }

  private tryStart(threadId: string): boolean {
    const tq = this.threads.get(threadId);
    if (!tq || tq.running || tq.waiting.length === 0) return false;
    if (this.globalRunning >= this.globalConcurrency) return false;

    const job = tq.waiting.shift()!;
    tq.running = job;
    this.globalRunning++;
    this.startJob(job, threadId);
    return true;
  }
}
```

Key invariants:
- **Per-thread serial**: `tq.running` is a single Job, not a set
- **Global cap**: `globalRunning` tracks total across all threads
- **Position is atomic**: captured before `tryStart()` mutates the queue

## DB Schema Changes

```sql
ALTER TABLE messages ADD COLUMN thread_id text;
ALTER TABLE messages ADD COLUMN reply_to_message_id integer;
CREATE INDEX idx_messages_thread_id ON messages (thread_id) WHERE thread_id IS NOT NULL;
```

- `thread_id` format: `"telegram:chatId:rootMessageId"` — globally unique
- `reply_to_message_id` stores the raw Telegram messageId for chain walking
- Existing rows: `NULL` for both columns (backward compatible)

## Trade-offs

| Decision | Alternative considered | Why this way |
|----------|----------------------|--------------|
| Thread root = first unquoted message | Thread root = first message in time window | Explicit > implicit; time windows create ambiguity |
| In-memory ThreadResolver cache | Redis/DB-only resolution | Sub-ms resolution, tiny memory footprint, daemon is single-instance |
| Global concurrency cap (not per-chat) | Per-chat concurrency cap | Single user system — global cap is simpler and equivalent |
| Strict serial per thread | Configurable per-thread parallelism | YAGNI; serial matches user mental model of conversation |
| reply_to on ALL responses | reply_to only on quoted threads | Consistent UX; enables quoting any Nova response for follow-up |
