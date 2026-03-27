export type JobPriority = "high" | "normal" | "low";

export type JobStatus = "queued" | "running" | "completed" | "failed" | "cancelled";

export interface Job {
  id: string;
  chatId: string;
  content: string;
  priority: JobPriority;
  status: JobStatus;
  abortController: AbortController;
  handler: (signal: AbortSignal) => Promise<void>;
  threadId: string;
  createdAt: Date;
  startedAt?: Date;
  completedAt?: Date;
  error?: string;
}

export interface JobEvent {
  type: "enqueued" | "started" | "completed" | "failed" | "cancelled";
  job: Job;
  queueDepth: number;
}

export interface EnqueueOpts {
  threadId: string;
  replyToMessageId?: number;
}

export interface EnqueueResult {
  job: Job;
  startedImmediately: boolean;
  position: number;
  threadState: "thread-busy" | "global-full" | "started";
}

export interface QueueConfig {
  concurrency: number;
  maxQueueSize: number;
}
