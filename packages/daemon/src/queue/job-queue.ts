import { randomUUID } from "node:crypto";
import { EventEmitter } from "node:events";
import type { Job, JobEvent, JobPriority, QueueConfig, EnqueueResult } from "./types.js";

/** Priority ordering — lower index = higher priority. */
const PRIORITY_ORDER: JobPriority[] = ["high", "normal", "low"];

interface EnqueueOpts {
  chatId: string;
  /** Defaults to chatId when not provided — each chat becomes its own thread. */
  threadId?: string;
  content: string;
  priority: JobPriority;
  handler: (signal: AbortSignal) => Promise<void>;
}

interface ThreadQueue {
  waiting: Job[];
  running: Job | null;
  lastActivityAt: number;
}

export interface JobQueueEvents {
  enqueued: [event: JobEvent];
  started: [event: JobEvent];
  completed: [event: JobEvent];
  failed: [event: JobEvent];
  cancelled: [event: JobEvent];
}

export class JobQueue extends EventEmitter<JobQueueEvents> {
  private readonly config: QueueConfig;
  private readonly threads = new Map<string, ThreadQueue>();
  private globalRunning = 0;
  private readonly globalConcurrency: number;

  constructor(config: QueueConfig) {
    super();
    this.config = config;
    this.globalConcurrency = config.concurrency;
  }

  // ── Public API ──────────────────────────────────────────────────────────────

  /**
   * Enqueue a job. Returns the job, whether it started immediately, its queue
   * position within the thread, and the thread state.
   * Throws if the queue is full.
   */
  enqueue(opts: EnqueueOpts): EnqueueResult {
    const totalJobs = this.totalWaiting() + this.globalRunning;
    if (totalJobs >= this.config.maxQueueSize) {
      throw new Error("Queue full — try again in a moment.");
    }

    // Resolve threadId — fall back to chatId so each chat is its own serial thread
    const threadId = opts.threadId ?? opts.chatId;

    // Ensure the thread sub-queue exists
    if (!this.threads.has(threadId)) {
      this.threads.set(threadId, {
        waiting: [],
        running: null,
        lastActivityAt: Date.now(),
      });
    }
    const tq = this.threads.get(threadId)!;

    const job: Job = {
      id: randomUUID(),
      chatId: opts.chatId,
      threadId,
      content: opts.content,
      priority: opts.priority,
      status: "queued",
      abortController: new AbortController(),
      handler: opts.handler,
      createdAt: new Date(),
    };

    // Insert into thread waiting queue in priority order (stable FIFO within same priority)
    this.insertByPriority(tq.waiting, job);

    // Capture position BEFORE tryStart() mutates the queue — fixes the race condition
    const position = tq.waiting.indexOf(job) + 1;

    this.emit("enqueued", {
      type: "enqueued",
      job,
      queueDepth: this.totalWaiting(),
    });

    // Try to start jobs for this thread
    const started = this.tryStart(threadId);

    let threadState: EnqueueResult["threadState"];
    if (started.has(job.id)) {
      threadState = "started";
    } else if (tq.running !== null) {
      threadState = "thread-busy";
    } else {
      threadState = "global-full";
    }

    return {
      job,
      startedImmediately: started.has(job.id),
      position,
      threadState,
    };
  }

  /**
   * Cancel a specific job by ID.
   * Returns true if the job was found and cancelled.
   */
  cancel(jobId: string): boolean {
    for (const [threadId, tq] of this.threads) {
      // Check waiting queue for this thread
      const waitIdx = tq.waiting.findIndex((j) => j.id === jobId);
      if (waitIdx !== -1) {
        const job = tq.waiting.splice(waitIdx, 1)[0]!;
        job.status = "cancelled";
        job.completedAt = new Date();
        job.abortController.abort();
        tq.lastActivityAt = Date.now();
        this.emit("cancelled", {
          type: "cancelled",
          job,
          queueDepth: this.totalWaiting(),
        });
        return true;
      }

      // Check running job for this thread
      if (tq.running?.id === jobId) {
        const job = tq.running;
        job.status = "cancelled";
        job.completedAt = new Date();
        job.abortController.abort();
        tq.running = null;
        this.globalRunning--;
        tq.lastActivityAt = Date.now();
        this.emit("cancelled", {
          type: "cancelled",
          job,
          queueDepth: this.totalWaiting(),
        });
        // Start next job in this thread since a slot freed up
        this.tryStart(threadId);
        return true;
      }
    }

    return false;
  }

  /**
   * Cancel all jobs for a given chatId.
   * Returns the number of jobs cancelled.
   */
  cancelByChatId(chatId: string): number {
    let cancelled = 0;

    for (const [threadId, tq] of this.threads) {
      // Cancel waiting jobs (iterate in reverse to safely splice)
      for (let i = tq.waiting.length - 1; i >= 0; i--) {
        const job = tq.waiting[i]!;
        if (job.chatId === chatId) {
          tq.waiting.splice(i, 1);
          job.status = "cancelled";
          job.completedAt = new Date();
          job.abortController.abort();
          tq.lastActivityAt = Date.now();
          this.emit("cancelled", {
            type: "cancelled",
            job,
            queueDepth: this.totalWaiting(),
          });
          cancelled++;
        }
      }

      // Cancel running job if it matches
      if (tq.running?.chatId === chatId) {
        const job = tq.running;
        job.status = "cancelled";
        job.completedAt = new Date();
        job.abortController.abort();
        tq.running = null;
        this.globalRunning--;
        tq.lastActivityAt = Date.now();
        this.emit("cancelled", {
          type: "cancelled",
          job,
          queueDepth: this.totalWaiting(),
        });
        cancelled++;
        // Start next job in this thread since a slot freed up
        this.tryStart(threadId);
      }
    }

    return cancelled;
  }

  /**
   * Get the status of jobs for a given chatId, optionally scoped to a threadId.
   */
  getStatus(
    chatId: string,
    threadId?: string,
  ): { state: "idle" | "running" | "queued"; position?: number } {
    const threadsToCheck = threadId
      ? ([this.threads.get(threadId)].filter(Boolean) as ThreadQueue[])
      : [...this.threads.values()];

    // Check running first
    for (const tq of threadsToCheck) {
      if (tq.running?.chatId === chatId) {
        return { state: "running" };
      }
    }

    // Check waiting queues — find the earliest queued job for this chatId
    for (const tq of threadsToCheck) {
      for (let i = 0; i < tq.waiting.length; i++) {
        if (tq.waiting[i]!.chatId === chatId) {
          return { state: "queued", position: i + 1 };
        }
      }
    }

    return { state: "idle" };
  }

  /**
   * Drain the queue: cancel all waiting jobs and wait for running jobs to finish.
   * Returns after all running jobs complete (or timeout).
   */
  async drain(timeoutMs = 10_000): Promise<{ cancelled: number; drained: number }> {
    let cancelled = 0;
    const runningCount = this.globalRunning;

    // Cancel all waiting jobs across all threads
    for (const tq of this.threads.values()) {
      while (tq.waiting.length > 0) {
        const job = tq.waiting.pop()!;
        job.status = "cancelled";
        job.completedAt = new Date();
        job.abortController.abort();
        this.emit("cancelled", {
          type: "cancelled",
          job,
          queueDepth: this.totalWaiting(),
        });
        cancelled++;
      }
    }

    // Abort all running jobs across all threads
    for (const tq of this.threads.values()) {
      if (tq.running) {
        tq.running.abortController.abort();
      }
    }

    // Wait for running jobs to complete (they should check signal.aborted)
    if (this.globalRunning > 0) {
      await new Promise<void>((resolve) => {
        const checkAndClean = (): void => {
          if (this.globalRunning === 0) {
            clearTimeout(timer);
            this.off("completed", checkAndClean);
            this.off("failed", checkAndClean);
            this.off("cancelled", checkAndClean);
            resolve();
          }
        };

        this.on("completed", checkAndClean);
        this.on("failed", checkAndClean);
        this.on("cancelled", checkAndClean);

        // Timeout safety net
        const timer = setTimeout(() => {
          this.off("completed", checkAndClean);
          this.off("failed", checkAndClean);
          this.off("cancelled", checkAndClean);
          resolve();
        }, timeoutMs);

        // Check immediately in case all already finished
        checkAndClean();
      });
    }

    return { cancelled, drained: runningCount };
  }

  /**
   * Drain all threads: alias for drain() — cancels waiting and waits for running.
   */
  async drainAll(timeoutMs = 10_000): Promise<{ cancelled: number; drained: number }> {
    return this.drain(timeoutMs);
  }

  // ── Internal ────────────────────────────────────────────────────────────────

  /** Count total waiting jobs across all threads. */
  private totalWaiting(): number {
    let total = 0;
    for (const tq of this.threads.values()) {
      total += tq.waiting.length;
    }
    return total;
  }

  /** Insert job into a waiting queue respecting priority order (FIFO within same priority). */
  private insertByPriority(waiting: Job[], job: Job): void {
    const jobPriorityIdx = PRIORITY_ORDER.indexOf(job.priority);

    // Find the first job with strictly lower priority
    let insertAt = waiting.length;
    for (let i = 0; i < waiting.length; i++) {
      const existingPriorityIdx = PRIORITY_ORDER.indexOf(waiting[i]!.priority);
      if (existingPriorityIdx > jobPriorityIdx) {
        insertAt = i;
        break;
      }
    }

    waiting.splice(insertAt, 0, job);
  }

  /**
   * Try to start jobs.
   * If threadId is given, attempt only that thread's queue.
   * If not given, attempt all threads.
   * Returns the set of job IDs that were started.
   */
  private tryStart(threadId?: string): Set<string> {
    const started = new Set<string>();

    if (threadId !== undefined) {
      const tq = this.threads.get(threadId);
      if (tq) {
        this.tryStartThread(threadId, tq, started);
      }
    } else {
      for (const [tid, tq] of this.threads) {
        this.tryStartThread(tid, tq, started);
      }
    }

    return started;
  }

  /**
   * Attempt to start the next waiting job in a single thread.
   * Respects both per-thread serial constraint and global concurrency cap.
   */
  private tryStartThread(threadId: string, tq: ThreadQueue, started: Set<string>): void {
    // Per-thread: at most 1 running job; global: cap total running
    if (tq.running !== null) return;
    if (this.globalRunning >= this.globalConcurrency) return;
    if (tq.waiting.length === 0) return;

    const job = tq.waiting.shift()!;
    this.startJob(threadId, tq, job);
    started.add(job.id);
  }

  /** Start a single job — move to running, emit event, execute handler. */
  private startJob(threadId: string, tq: ThreadQueue, job: Job): void {
    job.status = "running";
    job.startedAt = new Date();
    tq.running = job;
    this.globalRunning++;
    tq.lastActivityAt = Date.now();

    this.emit("started", {
      type: "started",
      job,
      queueDepth: this.totalWaiting(),
    });

    // Execute the handler
    job
      .handler(job.abortController.signal)
      .then(() => {
        // Only mark completed if not already cancelled
        if (job.status === "running") {
          job.status = "completed";
          job.completedAt = new Date();
          tq.running = null;
          this.globalRunning--;
          tq.lastActivityAt = Date.now();
          this.emit("completed", {
            type: "completed",
            job,
            queueDepth: this.totalWaiting(),
          });
          // Start next job in this thread
          this.tryStart(threadId);
        }
      })
      .catch((err: unknown) => {
        // Only mark failed if not already cancelled
        if (job.status === "running") {
          job.status = "failed";
          job.completedAt = new Date();
          job.error = err instanceof Error ? err.message : String(err);
          tq.running = null;
          this.globalRunning--;
          tq.lastActivityAt = Date.now();
          this.emit("failed", {
            type: "failed",
            job,
            queueDepth: this.totalWaiting(),
          });
          // Start next job in this thread
          this.tryStart(threadId);
        }
      });
  }
}
