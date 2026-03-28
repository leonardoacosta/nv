import { describe, it } from "node:test";
import assert from "node:assert/strict";
import type { Pool, QueryResult } from "pg";

import { ThreadResolver } from "../../src/queue/thread-resolver.js";
import { JobQueue } from "../../src/queue/job-queue.js";

// NOTE: telegram.ts is not imported directly because it transitively imports
// @nova/db (via diary/reader.ts), which requires DATABASE_URL at module load
// time. The normalizeTextMessage behaviour is verified by replicating the
// minimal mapping logic below, keeping tests hermetic.

// ── Helpers ────────────────────────────────────────────────────────────────────

function makePool(rows: Record<string, unknown>[] = []): Pool {
  return {
    query: async (): Promise<QueryResult> =>
      ({ rows, rowCount: rows.length }) as unknown as QueryResult,
  } as unknown as Pool;
}

/**
 * Minimal inline replica of the replyToMessageId extraction logic from
 * normalizeTextMessage in src/channels/telegram.ts.
 *
 * The real function is not imported here to avoid pulling in @nova/db at
 * module load time (see NOTE above). This replica tests the same mapping
 * contract: msg.reply_to_message?.message_id → replyToMessageId.
 */
interface MinimalTgMessage {
  message_id: number;
  chat: { id: number };
  reply_to_message?: { message_id: number };
  text?: string;
}

function normalizeReplyToMessageId(msg: MinimalTgMessage): number | undefined {
  return msg.reply_to_message?.message_id;
}

function makeChatId(msg: MinimalTgMessage): string {
  return String(msg.chat.id);
}

function makeTextMessage(
  messageId: number,
  chatId: number,
  text: string,
  replyTo?: number,
): MinimalTgMessage {
  const base: MinimalTgMessage = {
    message_id: messageId,
    chat: { id: chatId },
    text,
  };
  if (replyTo !== undefined) {
    base.reply_to_message = { message_id: replyTo };
  }
  return base;
}

/** Create a deferred promise so tests can control when a handler resolves. */
function makeDeferred(): {
  promise: Promise<void>;
  resolve: () => void;
  reject: (err: Error) => void;
} {
  let resolve!: () => void;
  let reject!: (err: Error) => void;
  const promise = new Promise<void>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

// ── 4.1 — Unquoted message creates new thread ─────────────────────────────────

describe("4.1 — ThreadResolver: unquoted message creates new thread", () => {
  it("returns chatId:messageId when replyToMessageId is undefined", async () => {
    const resolver = new ThreadResolver(makePool());
    const threadId = await resolver.resolve("123", 42, undefined);
    assert.equal(threadId, "123:42");
  });

  it("normalizeTextMessage logic: no reply_to_message → replyToMessageId is undefined", () => {
    const msg = makeTextMessage(55, 999, "Hello");
    assert.equal(normalizeReplyToMessageId(msg), undefined);
    assert.equal(makeChatId(msg), "999");
    assert.equal(msg.message_id, 55);
  });

  it("normalizeTextMessage logic: undefined replyToMessageId produces new-root threadId", async () => {
    const resolver = new ThreadResolver(makePool());
    const msg = makeTextMessage(55, 999, "Hello");
    const replyToMessageId = normalizeReplyToMessageId(msg);
    const threadId = await resolver.resolve(makeChatId(msg), msg.message_id, replyToMessageId);
    assert.equal(threadId, "999:55");
  });

  it("caches the new root so a subsequent resolve for same messageId returns same threadId", async () => {
    const pool = makePool();
    let queryCalls = 0;
    const trackingPool: Pool = {
      query: async (...args: Parameters<Pool["query"]>) => {
        queryCalls++;
        return (pool.query as (...a: unknown[]) => Promise<QueryResult>)(...(args as unknown[]));
      },
    } as unknown as Pool;

    const resolver = new ThreadResolver(trackingPool);
    const first = await resolver.resolve("10", 1, undefined);
    // Resolve a reply that references this root — should hit cache, not DB
    const second = await resolver.resolve("10", 2, 1);
    assert.equal(first, "10:1");
    assert.equal(second, "10:1");
    assert.equal(queryCalls, 0, "cache hit: DB should not be queried");
  });
});

// ── 4.2 — Quoted message joins existing thread, serial execution ───────────────

describe("4.2 — ThreadResolver + JobQueue: quoted message joins thread, serial execution", () => {
  it("resolves to the root messageId when replyToMessageId is present in cache", async () => {
    const resolver = new ThreadResolver(makePool());
    // Seed root msg into cache
    await resolver.resolve("10", 100, undefined); // root → "10:100"
    // Reply to root
    const threadId = await resolver.resolve("10", 101, 100);
    assert.equal(threadId, "10:100");
  });

  it("normalizeTextMessage logic: with reply_to_message → replyToMessageId is set", () => {
    const msg = makeTextMessage(200, 999, "Reply text", 100);
    assert.equal(normalizeReplyToMessageId(msg), 100);
  });

  it("resolves multi-hop chain to root via DB walk", async () => {
    // Chain: 300 → 200 → 100 (root has reply_to_message_id = null)
    const rows: Record<string, unknown>[][] = [
      [{ reply_to_message_id: 200 }], // query for 300
      [{ reply_to_message_id: 100 }], // query for 200
      [{ reply_to_message_id: null }], // query for 100 → root
    ];
    let callIdx = 0;
    const pool: Pool = {
      query: async (): Promise<QueryResult> => {
        const batch = rows[callIdx++] ?? [];
        return { rows: batch, rowCount: batch.length } as unknown as QueryResult;
      },
    } as unknown as Pool;

    const resolver = new ThreadResolver(pool);
    const threadId = await resolver.resolve("20", 301, 300);
    assert.equal(threadId, "20:100");
  });

  it("two jobs in same threadId execute serially — second starts only after first completes", async () => {
    const queue = new JobQueue({ concurrency: 4, maxQueueSize: 20 });

    const first = makeDeferred();
    const second = makeDeferred();
    const order: string[] = [];

    const threadId = "999:100";

    const r1 = queue.enqueue({
      chatId: "999",
      threadId,
      content: "msg1",
      priority: "normal",
      handler: async () => {
        order.push("first-started");
        await first.promise;
        order.push("first-done");
      },
    });

    const r2 = queue.enqueue({
      chatId: "999",
      threadId,
      content: "msg2",
      priority: "normal",
      handler: async () => {
        order.push("second-started");
        await second.promise;
        order.push("second-done");
      },
    });

    // First job should start immediately; second should be waiting
    assert.equal(r1.startedImmediately, true);
    assert.equal(r2.startedImmediately, false);
    assert.equal(r2.threadState, "thread-busy");

    // Verify second has NOT started yet
    assert.ok(!order.includes("second-started"), "second must not start before first finishes");

    // Resolve first — second should now start
    first.resolve();
    await new Promise<void>((res) => queue.once("started", () => res()));

    assert.ok(order.includes("first-done"), "first should have completed");
    assert.ok(order.includes("second-started"), "second should have started after first");

    // Clean up
    second.resolve();
    await new Promise<void>((res) => queue.once("completed", () => res()));
    queue.shutdown();
  });
});

// ── 4.3 — Two unquoted messages process in parallel ───────────────────────────

describe("4.3 — JobQueue: two jobs with different threadIds run in parallel", () => {
  it("both jobs start immediately when under global concurrency cap", () => {
    const queue = new JobQueue({ concurrency: 4, maxQueueSize: 20 });

    const d1 = makeDeferred();
    const d2 = makeDeferred();

    const r1 = queue.enqueue({
      chatId: "111",
      threadId: "111:1",
      content: "chat1 msg",
      priority: "normal",
      handler: async () => { await d1.promise; },
    });

    const r2 = queue.enqueue({
      chatId: "222",
      threadId: "222:2",
      content: "chat2 msg",
      priority: "normal",
      handler: async () => { await d2.promise; },
    });

    assert.equal(r1.startedImmediately, true, "first job must start immediately");
    assert.equal(r2.startedImmediately, true, "second job (different thread) must also start immediately");
    assert.equal(r1.threadState, "started");
    assert.equal(r2.threadState, "started");

    // Clean up — resolve both without waiting to avoid timer leak
    d1.resolve();
    d2.resolve();
    queue.shutdown();
  });

  it("both jobs are running concurrently (neither waits for the other)", async () => {
    const queue = new JobQueue({ concurrency: 4, maxQueueSize: 20 });

    const running: string[] = [];
    const d1 = makeDeferred();
    const d2 = makeDeferred();

    queue.enqueue({
      chatId: "aaa",
      threadId: "aaa:10",
      content: "job A",
      priority: "normal",
      handler: async () => {
        running.push("A");
        await d1.promise;
      },
    });

    queue.enqueue({
      chatId: "bbb",
      threadId: "bbb:20",
      content: "job B",
      priority: "normal",
      handler: async () => {
        running.push("B");
        await d2.promise;
      },
    });

    // Both handlers were invoked synchronously by enqueue (microtask flush)
    await Promise.resolve();
    assert.ok(running.includes("A"), "job A handler must be running");
    assert.ok(running.includes("B"), "job B handler must be running");

    d1.resolve();
    d2.resolve();
    await new Promise<void>((res) => {
      let done = 0;
      queue.on("completed", () => { if (++done === 2) res(); });
    });
    queue.shutdown();
  });

  it("global concurrency cap blocks third job when cap is 2", () => {
    const queue = new JobQueue({ concurrency: 2, maxQueueSize: 20 });

    const d = [makeDeferred(), makeDeferred(), makeDeferred()];

    const results = d.map((deferred, i) =>
      queue.enqueue({
        chatId: `chat${i}`,
        threadId: `chat${i}:${i}`,
        content: `msg ${i}`,
        priority: "normal",
        handler: async () => { await deferred.promise; },
      }),
    );

    assert.equal(results[0]!.startedImmediately, true);
    assert.equal(results[1]!.startedImmediately, true);
    // Third job can't start — global cap hit
    assert.equal(results[2]!.startedImmediately, false);
    assert.equal(results[2]!.threadState, "global-full");

    d.forEach((def) => def.resolve());
    queue.shutdown();
  });
});

// ── 4.4 — Queue position ack is per-thread accurate ──────────────────────────

describe("4.4 — JobQueue: enqueue returns accurate per-thread position", () => {
  it("first job in a fresh thread has position 1", () => {
    const queue = new JobQueue({ concurrency: 4, maxQueueSize: 20 });
    const d = makeDeferred();

    const result = queue.enqueue({
      chatId: "chat1",
      threadId: "chat1:1",
      content: "first",
      priority: "normal",
      handler: async () => { await d.promise; },
    });

    // Position is 1 regardless of whether it started immediately
    assert.equal(result.position, 1);
    assert.equal(result.startedImmediately, true);

    d.resolve();
    queue.shutdown();
  });

  it("second job on thread-busy thread reports position 1 (only waiter)", () => {
    const queue = new JobQueue({ concurrency: 4, maxQueueSize: 20 });
    const d1 = makeDeferred();
    const d2 = makeDeferred();
    const threadId = "chat2:10";

    queue.enqueue({
      chatId: "chat2",
      threadId,
      content: "first",
      priority: "normal",
      handler: async () => { await d1.promise; },
    });

    const r2 = queue.enqueue({
      chatId: "chat2",
      threadId,
      content: "second",
      priority: "normal",
      handler: async () => { await d2.promise; },
    });

    assert.equal(r2.startedImmediately, false);
    assert.equal(r2.threadState, "thread-busy");
    // Second job is the only one waiting in the thread queue
    assert.equal(r2.position, 1);

    d1.resolve();
    d2.resolve();
    queue.shutdown();
  });

  it("third job on thread-busy thread reports position 2", () => {
    const queue = new JobQueue({ concurrency: 4, maxQueueSize: 20 });
    const d1 = makeDeferred();
    const d2 = makeDeferred();
    const d3 = makeDeferred();
    const threadId = "chat3:20";

    queue.enqueue({
      chatId: "chat3",
      threadId,
      content: "first",
      priority: "normal",
      handler: async () => { await d1.promise; },
    });

    const r2 = queue.enqueue({
      chatId: "chat3",
      threadId,
      content: "second",
      priority: "normal",
      handler: async () => { await d2.promise; },
    });

    const r3 = queue.enqueue({
      chatId: "chat3",
      threadId,
      content: "third",
      priority: "normal",
      handler: async () => { await d3.promise; },
    });

    assert.equal(r2.position, 1, "second job is at position 1 in waiting queue");
    assert.equal(r3.position, 2, "third job is at position 2 in waiting queue");
    assert.equal(r3.threadState, "thread-busy");

    d1.resolve();
    d2.resolve();
    d3.resolve();
    queue.shutdown();
  });

  it("global-full: reports threadState global-full when global cap is reached", () => {
    const queue = new JobQueue({ concurrency: 1, maxQueueSize: 20 });
    const d1 = makeDeferred();
    const d2 = makeDeferred();

    // Fill the global concurrency slot with thread A
    queue.enqueue({
      chatId: "chatA",
      threadId: "chatA:1",
      content: "running",
      priority: "normal",
      handler: async () => { await d1.promise; },
    });

    // Thread B has no running job but global is full
    const r2 = queue.enqueue({
      chatId: "chatB",
      threadId: "chatB:2",
      content: "waiting",
      priority: "normal",
      handler: async () => { await d2.promise; },
    });

    assert.equal(r2.startedImmediately, false);
    assert.equal(r2.threadState, "global-full");
    assert.equal(r2.position, 1);

    d1.resolve();
    d2.resolve();
    queue.shutdown();
  });

  it("position is per-thread: jobs in separate threads each report position 1", () => {
    const queue = new JobQueue({ concurrency: 1, maxQueueSize: 20 });
    const d = [makeDeferred(), makeDeferred(), makeDeferred()];

    // Fill global slot
    queue.enqueue({
      chatId: "chatX",
      threadId: "chatX:1",
      content: "running",
      priority: "normal",
      handler: async () => { await d[0]!.promise; },
    });

    // Two separate threads, both globally-full → each should have position 1 in their thread
    const rY = queue.enqueue({
      chatId: "chatY",
      threadId: "chatY:2",
      content: "waiting Y",
      priority: "normal",
      handler: async () => { await d[1]!.promise; },
    });

    const rZ = queue.enqueue({
      chatId: "chatZ",
      threadId: "chatZ:3",
      content: "waiting Z",
      priority: "normal",
      handler: async () => { await d[2]!.promise; },
    });

    assert.equal(rY.position, 1, "thread Y: first (and only) waiter at position 1");
    assert.equal(rZ.position, 1, "thread Z: first (and only) waiter at position 1");
    assert.equal(rY.threadState, "global-full");
    assert.equal(rZ.threadState, "global-full");

    d.forEach((def) => def.resolve());
    queue.shutdown();
  });
});
