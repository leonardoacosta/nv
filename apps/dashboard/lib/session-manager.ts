import { exec, execSync } from "child_process";
import { promisify } from "util";

const execAsync = promisify(exec);

const CONTAINER_NAME = "nova-cc-session";
const START_TIMEOUT_MS = 30_000;
const STOP_TIMEOUT_S = 10;
const HEALTH_POLL_INTERVAL_MS = 15_000;
const IDLE_THRESHOLD_MS = 30 * 60 * 1000; // 30 minutes
const MAX_AUTO_RESTARTS = 3;
const AUTO_RESTART_WINDOW_MS = 5 * 60 * 1000; // 5 minutes
const MESSAGE_TIMEOUT_MS = 120_000;

export type SessionState = "active" | "idle" | "starting" | "stopping" | "stopped" | "error";

export interface SessionStatus {
  state: SessionState;
  uptime_secs: number | null;
  last_message_at: string | null;
  message_count: number;
  error_message?: string;
  restart_count: number;
}

class SessionManager {
  private state: SessionState = "stopped";
  private startedAt: Date | null = null;
  private lastMessageAt: Date | null = null;
  private messageCount = 0;
  private restartCount = 0;
  private errorMessage: string | undefined;
  private healthTimer: ReturnType<typeof setInterval> | null = null;
  private recentRestarts: number[] = [];

  constructor() {
    this.startHealthPolling();
  }

  // -------------------------------------------------------------------------
  // Public API
  // -------------------------------------------------------------------------

  async start(): Promise<void> {
    if (this.state === "active" || this.state === "starting") return;

    this.state = "starting";
    this.errorMessage = undefined;

    try {
      // Check if the container exists; if not, create it via docker compose up
      const exists = await this.containerExists();
      if (!exists) {
        await execAsync(`docker compose -f "${this.composePath()}" up -d --no-start`);
      }

      await execAsync(`docker start ${CONTAINER_NAME}`);
      await this.waitUntilRunning(START_TIMEOUT_MS);

      this.startedAt = new Date();
      this.state = "active";
    } catch (err) {
      this.state = "error";
      this.errorMessage = err instanceof Error ? err.message : String(err);
      throw err;
    }
  }

  async stop(): Promise<void> {
    if (this.state === "stopped" || this.state === "stopping") return;

    this.state = "stopping";
    try {
      await execAsync(`docker stop -t ${STOP_TIMEOUT_S} ${CONTAINER_NAME}`);
    } finally {
      this.state = "stopped";
      this.startedAt = null;
    }
  }

  async restart(): Promise<void> {
    await this.stop();
    this.restartCount = 0;
    this.recentRestarts = [];
    await this.start();
  }

  getStatus(): SessionStatus {
    const uptime_secs =
      this.startedAt
        ? Math.floor((Date.now() - this.startedAt.getTime()) / 1000)
        : null;

    return {
      state: this.state,
      uptime_secs,
      last_message_at: this.lastMessageAt?.toISOString() ?? null,
      message_count: this.messageCount,
      error_message: this.errorMessage,
      restart_count: this.restartCount,
    };
  }

  async sendMessage(
    text: string,
    context?: Record<string, unknown>,
  ): Promise<{ reply: string; processing_ms: number }> {
    if (this.state !== "active" && this.state !== "idle") {
      throw new Error(`Session is not ready (state=${this.state})`);
    }

    const started = Date.now();

    // Build stream-json input envelope
    const inputLine = JSON.stringify({
      type: "user",
      message: {
        role: "user",
        content: context
          ? `${text}\n\n<context>${JSON.stringify(context)}</context>`
          : text,
      },
    });

    let stdout = "";
    let stderr = "";

    await new Promise<void>((resolve, reject) => {
      const timer = setTimeout(() => {
        reject(new Error("sendMessage timed out after 120s"));
      }, MESSAGE_TIMEOUT_MS);

      const child = exec(
        `echo ${JSON.stringify(inputLine)} | docker exec -i ${CONTAINER_NAME} sh -c 'cat'`,
        { timeout: MESSAGE_TIMEOUT_MS },
        (err, out, errOut) => {
          clearTimeout(timer);
          if (err) {
            reject(err);
          } else {
            stdout = out;
            stderr = errOut;
            resolve();
          }
        },
      );

      void child; // suppress unused warning
      void stderr;
    });

    // Parse stream-json output — accumulate text blocks until result event
    const reply = this.parseStreamJsonReply(stdout);
    const processing_ms = Date.now() - started;

    this.lastMessageAt = new Date();
    this.messageCount++;
    this.state = "active";

    return { reply, processing_ms };
  }

  async getLogs(lines = 50): Promise<string[]> {
    try {
      const { stdout } = await execAsync(
        `docker logs --tail ${lines} ${CONTAINER_NAME} 2>&1`,
      );
      return stdout.split("\n").filter((l) => l.length > 0);
    } catch {
      return [];
    }
  }

  // -------------------------------------------------------------------------
  // Private helpers
  // -------------------------------------------------------------------------

  private async containerExists(): Promise<boolean> {
    try {
      execSync(`docker inspect ${CONTAINER_NAME}`, { stdio: "ignore" });
      return true;
    } catch {
      return false;
    }
  }

  private composePath(): string {
    // Resolve relative to this file's location at runtime
    const path = require("path") as typeof import("path");
    return path.resolve(
      path.dirname(__filename),
      "../docker/cc-session/docker-compose.yml",
    );
  }

  private async waitUntilRunning(timeoutMs: number): Promise<void> {
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
      try {
        const { stdout } = await execAsync(
          `docker inspect --format '{{.State.Status}}' ${CONTAINER_NAME}`,
        );
        if (stdout.trim() === "running") return;
      } catch {
        // container may not be visible yet — keep polling
      }
      await new Promise((r) => setTimeout(r, 500));
    }
    throw new Error(
      `Container ${CONTAINER_NAME} did not reach running state within ${timeoutMs}ms`,
    );
  }

  private async getDockerState(): Promise<string | null> {
    try {
      const { stdout } = await execAsync(
        `docker inspect --format '{{.State.Status}}' ${CONTAINER_NAME}`,
      );
      return stdout.trim();
    } catch {
      return null;
    }
  }

  private parseStreamJsonReply(raw: string): string {
    const lines = raw.split("\n").filter((l) => l.trim().length > 0);
    const textParts: string[] = [];

    for (const line of lines) {
      try {
        const event = JSON.parse(line) as Record<string, unknown>;
        // Accumulate assistant text blocks
        if (
          event.type === "content_block_delta" &&
          event.delta &&
          typeof (event.delta as Record<string, unknown>).text === "string"
        ) {
          textParts.push((event.delta as Record<string, unknown>).text as string);
        }
        // Stop at result event
        if (event.type === "result") break;
      } catch {
        // Skip non-JSON lines (e.g., init noise)
      }
    }

    return textParts.join("");
  }

  private startHealthPolling(): void {
    if (this.healthTimer) return;

    this.healthTimer = setInterval(() => {
      void this.runHealthCheck();
    }, HEALTH_POLL_INTERVAL_MS);

    // Don't keep the Node process alive just for this timer
    if (typeof this.healthTimer.unref === "function") {
      this.healthTimer.unref();
    }
  }

  private async runHealthCheck(): Promise<void> {
    // Only care when we expect the container to be running
    if (this.state === "stopping" || this.state === "stopped") return;

    const dockerState = await this.getDockerState();

    // Detect unexpected exit
    if (
      (this.state === "active" || this.state === "idle") &&
      dockerState !== "running"
    ) {
      await this.handleUnexpectedExit();
      return;
    }

    // Detect idle (no message activity for >30 min while "active")
    if (
      this.state === "active" &&
      this.lastMessageAt &&
      Date.now() - this.lastMessageAt.getTime() > IDLE_THRESHOLD_MS
    ) {
      this.state = "idle";
    }
  }

  private async handleUnexpectedExit(): Promise<void> {
    const now = Date.now();

    // Prune timestamps outside the rolling window
    this.recentRestarts = this.recentRestarts.filter(
      (ts) => now - ts < AUTO_RESTART_WINDOW_MS,
    );

    if (this.recentRestarts.length >= MAX_AUTO_RESTARTS) {
      this.state = "error";
      this.errorMessage = `Container exited unexpectedly. Auto-restart limit (${MAX_AUTO_RESTARTS} in ${AUTO_RESTART_WINDOW_MS / 60000} min) reached.`;
      this.startedAt = null;
      return;
    }

    // Attempt auto-restart
    this.recentRestarts.push(now);
    this.restartCount++;

    try {
      await execAsync(`docker start ${CONTAINER_NAME}`);
      await this.waitUntilRunning(START_TIMEOUT_MS);
      this.startedAt = new Date();
      this.state = "active";
      this.errorMessage = undefined;
    } catch (err) {
      this.state = "error";
      this.errorMessage = `Auto-restart failed: ${err instanceof Error ? err.message : String(err)}`;
      this.startedAt = null;
    }
  }
}

// Singleton export — initialized on module load
export const sessionManager = new SessionManager();
