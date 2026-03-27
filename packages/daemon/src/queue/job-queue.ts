import { randomUUID } from "node:crypto";
import { EventEmitter } from "node:events";
import type { Job, JobEvent, JobPriority, QueueConfig } from "./types.js";

/** Priority ordering — lower index = higher priority. */
const PRIORITY_ORDER: JobPriority[] = ["high", "normal", "low"];

interface EnqueueOpts {
  chatId: string;
  content: string;
  priority: JobPriority;
  handler: (signal: AbortSignal) => Promise<void>;
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
  private readonly waiting: Job[] = [];
  private readonly running = new Map<string, Job>();

  constructor(config: QueueConfig) {
    super();
    this.config = config;
  }

  // ── Public API ──────────────────────────────────────────────────────────────

  /**
   * Enqueue a job. Returns the Job and whether it started immediately.
   * Throws if the queue is full.
   */
  enqueue(opts: EnqueueOpts): { job: Job; startedImmediately: boolean } {
    const totalJobs = this.waiting.length + this.running.size;
    if (totalJobs >= this.config.maxQueueSize) {
      throw new Error("Queue full — try again in a moment.");
    }

    const job: Job = {
      id: randomUUID(),
      chatId: opts.chatId,
      content: opts.content,
      priority: opts.priority,
      status: "queued",
      abortController: new AbortController(),
      handler: opts.handler,
      createdAt: new Date(),
    };

    // Insert into waiting queue in priority order (stable FIFO within same priority)
    this.insertByPriority(job);

    this.emit("enqueued", {
      type: "enqueued",
      job,
      queueDepth: this.waiting.length,
    });

    // Try to start jobs if slots are available
    const started = this.tryStart();

    return { job, startedImmediately: started.has(job.id) };
  }

  /**
   * Cancel a specific job by ID.
   * Returns true if the job was found and cancelled.
   */
  cancel(jobId: string): boolean {
    // Check waiting queue
    const waitIdx = this.waiting.findIndex((j) => j.id === jobId);
    if (waitIdx !== -1) {
      const job = this.waiting.splice(waitIdx, 1)[0]!;
      job.status = "cancelled";
      job.completedAt = new Date();
      job.abortController.abort();
      this.emit("cancelled", {
        type: "cancelled",
        job,
        queueDepth: this.waiting.length,
      });
      return true;
    }

    // Check running jobs
    const job = this.running.get(jobId);
    if (job) {
      job.status = "cancelled";
      job.completedAt = new Date();
      job.abortController.abort();
      this.running.delete(jobId);
      this.emit("cancelled", {
        type: "cancelled",
        job,
        queueDepth: this.waiting.length,
      });
      // Start next queued job since a slot freed up
      this.tryStart();
      return true;
    }

    return false;
  }

  /**
   * Cancel all jobs for a given chatId.
   * Returns the number of jobs cancelled.
   */
  cancelByChatId(chatId: string): number {
    let cancelled = 0;

    // Cancel waiting jobs (iterate in reverse to safely splice)
    for (let i = this.waiting.length - 1; i >= 0; i--) {
      const job = this.waiting[i]!;
      if (job.chatId === chatId) {
        this.waiting.splice(i, 1);
        job.status = "cancelled";
        job.completedAt = new Date();
        job.abortController.abort();
        this.emit("cancelled", {
          type: "cancelled",
          job,
          queueDepth: this.waiting.length,
        });
        cancelled++;
      }
    }

    // Cancel running jobs
    for (const [id, job] of this.running) {
      if (job.chatId === chatId) {
        job.status = "cancelled";
        job.completedAt = new Date();
        job.abortController.abort();
        this.running.delete(id);
        this.emit("cancelled", {
          type: "cancelled",
          job,
          queueDepth: this.waiting.length,
        });
        cancelled++;
      }
    }

    // Start next queued jobs since slots may have freed up
    if (cancelled > 0) {
      this.tryStart();
    }

    return cancelled;
  }

  /**
   * Get the status of jobs for a given chatId.
   */
  getStatus(chatId: string): { state: "idle" | "running" | "queued"; position?: number } {
    // Check running first
    for (const job of this.running.values()) {
      if (job.chatId === chatId) {
        return { state: "running" };
      }
    }

    // Check waiting queue — find the oldest queued job for this chat
    // and count how many jobs are ahead of it
    let earliestIdx = -1;
    for (let i = 0; i < this.waiting.length; i++) {
      if (this.waiting[i]!.chatId === chatId) {
        earliestIdx = i;
        break;
      }
    }

    if (earliestIdx !== -1) {
      return { state: "queued", position: earliestIdx + 1 };
    }

    return { state: "idle" };
  }

  /**
   * Drain the queue: cancel all waiting jobs and wait for running jobs to finish.
   * Returns after all running jobs complete (or timeout).
   */
  async drain(timeoutMs = 10_000): Promise<{ cancelled: number; drained: number }> {
    // Cancel all waiting jobs
    let cancelled = 0;
    while (this.waiting.length > 0) {
      const job = this.waiting.pop()!;
      job.status = "cancelled";
      job.completedAt = new Date();
      job.abortController.abort();
      this.emit("cancelled", {
        type: "cancelled",
        job,
        queueDepth: this.waiting.length,
      });
      cancelled++;
    }

    // Abort all running jobs
    const runningCount = this.running.size;
    for (const job of this.running.values()) {
      job.abortController.abort();
    }

    // Wait for running jobs to complete (they should check signal.aborted)
    if (this.running.size > 0) {
      await new Promise<void>((resolve) => {
        const check = (): void => {
          if (this.running.size === 0) {
            resolve();
          }
        };

        // Check on every terminal event
        const onTerminal = (): void => check();
        this.on("completed", onTerminal);
        this.on("failed", onTerminal);
        this.on("cancelled", onTerminal);

        // Timeout safety net
        const timer = setTimeout(() => {
          this.off("completed", onTerminal);
          this.off("failed", onTerminal);
          this.off("cancelled", onTerminal);
          resolve();
        }, timeoutMs);

        // Check immediately in case all already finished
        check();

        // Clean up listeners if resolved normally
        const origResolve = resolve;
        const wrappedResolve = (): void => {
          clearTimeout(timer);
          this.off("completed", onTerminal);
          this.off("failed", onTerminal);
          this.off("cancelled", onTerminal);
          origResolve();
        };

        // Replace the check to use wrappedResolve
        const checkAndClean = (): void => {
          if (this.running.size === 0) {
            wrappedResolve();
          }
        };

        // Re-register with cleanup version
        this.off("completed", onTerminal);
        this.off("failed", onTerminal);
        this.off("cancelled", onTerminal);
        this.on("completed", checkAndClean);
        this.on("failed", checkAndClean);
        this.on("cancelled", checkAndClean);
        checkAndClean();
      });
    }

    return { cancelled, drained: runningCount };
  }

  // ── Internal ────────────────────────────────────────────────────────────────

  /** Insert job into waiting queue respecting priority order (FIFO within same priority). */
  private insertByPriority(job: Job): void {
    const jobPriorityIdx = PRIORITY_ORDER.indexOf(job.priority);

    // Find the first job with strictly lower priority
    let insertAt = this.waiting.length;
    for (let i = 0; i < this.waiting.length; i++) {
      const existingPriorityIdx = PRIORITY_ORDER.indexOf(this.waiting[i]!.priority);
      if (existingPriorityIdx > jobPriorityIdx) {
        insertAt = i;
        break;
      }
    }

    this.waiting.splice(insertAt, 0, job);
  }

  /**
   * Try to start jobs from the waiting queue if concurrency slots are available.
   * Returns the set of job IDs that were started.
   */
  private tryStart(): Set<string> {
    const started = new Set<string>();

    while (this.running.size < this.config.concurrency && this.waiting.length > 0) {
      const job = this.waiting.shift()!;
      this.startJob(job);
      started.add(job.id);
    }

    return started;
  }

  /** Start a single job — move to running, emit event, execute handler. */
  private startJob(job: Job): void {
    job.status = "running";
    job.startedAt = new Date();
    this.running.set(job.id, job);

    this.emit("started", {
      type: "started",
      job,
      queueDepth: this.waiting.length,
    });

    // Execute the handler
    job
      .handler(job.abortController.signal)
      .then(() => {
        // Only mark completed if not already cancelled
        if (job.status === "running") {
          job.status = "completed";
          job.completedAt = new Date();
          this.running.delete(job.id);
          this.emit("completed", {
            type: "completed",
            job,
            queueDepth: this.waiting.length,
          });
          this.tryStart();
        }
      })
      .catch((err: unknown) => {
        // Only mark failed if not already cancelled
        if (job.status === "running") {
          job.status = "failed";
          job.completedAt = new Date();
          job.error = err instanceof Error ? err.message : String(err);
          this.running.delete(job.id);
          this.emit("failed", {
            type: "failed",
            job,
            queueDepth: this.waiting.length,
          });
          this.tryStart();
        }
      });
  }
}
