import { fleetGet, FleetClientError } from "../../fleet-client.js";
import type { TelegramAdapter } from "../../channels/telegram.js";
import type { Logger } from "../../logger.js";
import type { FleetHealthMonitorConfig, ServiceStatus } from "./types.js";

// ─── Service registry ─────────────────────────────────────────────────────────

export const FLEET_SERVICES: readonly { name: string; port: number }[] = [
  { name: "tool-router",  port: 4100 },
  { name: "memory-svc",   port: 4101 },
  { name: "messages-svc", port: 4102 },
  { name: "channels-svc", port: 4103 },
  { name: "discord-svc",  port: 4104 },
  { name: "teams-svc",    port: 4105 },
  { name: "schedule-svc", port: 4106 },
  { name: "graph-svc",    port: 4107 },
  { name: "meta-svc",     port: 4108 },
];

// ─── Critical services (Telegram alert on state change) ───────────────────────

const CRITICAL_SERVICE_NAMES = new Set(["tool-router", "memory-svc", "graph-svc"]);

// ─── Uptime formatting helper ─────────────────────────────────────────────────

function formatUptime(startedAt: Date): string {
  const totalSecs = Math.floor((Date.now() - startedAt.getTime()) / 1000);
  const hours = Math.floor(totalSecs / 3600);
  const minutes = Math.floor((totalSecs % 3600) / 60);
  return hours > 0 ? `${hours}h ${minutes}m` : `${minutes}m`;
}

// ─── FleetHealthMonitor ───────────────────────────────────────────────────────

export class FleetHealthMonitor {
  private _state: ServiceStatus[] = [];
  private _timer: ReturnType<typeof setInterval> | null = null;
  private _isFirstProbe = true;
  private readonly _startedAt = new Date();

  constructor(
    private readonly config: FleetHealthMonitorConfig,
    private readonly logger: Logger,
    private readonly telegram?: TelegramAdapter,
    private readonly telegramChatId?: string,
  ) {}

  /**
   * Start the monitor: probe immediately (fire-and-forget), then schedule
   * repeating probes at the configured interval.
   */
  start(): void {
    void this.probe();
    this._timer = setInterval(() => {
      void this.probe();
    }, this.config.intervalMs);
  }

  /**
   * Stop the monitor by clearing the interval. No-op if not running.
   */
  stop(): void {
    if (this._timer !== null) {
      clearInterval(this._timer);
      this._timer = null;
    }
  }

  /**
   * Run one full probe pass against all fleet services concurrently.
   * Updates internal state and logs transitions.
   */
  async probe(): Promise<void> {
    const probeTimeoutMs = this.config.probeTimeoutMs;

    const results = await Promise.allSettled(
      FLEET_SERVICES.map(async (svc) => {
        const start = Date.now();
        try {
          await fleetGet(svc.port, "/health", probeTimeoutMs);
          return {
            name: svc.name,
            port: svc.port,
            status: "healthy" as const,
            latencyMs: Date.now() - start,
            lastCheckedAt: new Date(),
          };
        } catch (err) {
          let errorMessage: string;
          if (err instanceof FleetClientError && err.status === 504) {
            errorMessage = `timeout after ${probeTimeoutMs}ms`;
          } else if (err instanceof FleetClientError) {
            errorMessage = err.message;
          } else if (err instanceof Error) {
            errorMessage = err.message;
          } else {
            errorMessage = String(err);
          }

          return {
            name: svc.name,
            port: svc.port,
            status: "unhealthy" as const,
            latencyMs: null,
            lastCheckedAt: new Date(),
            error: errorMessage,
          };
        }
      }),
    );

    const newState: ServiceStatus[] = results.map((r) => {
      if (r.status === "fulfilled") return r.value;
      // Should never happen — inner try/catch always returns a value
      const idx = results.indexOf(r);
      const svc = FLEET_SERVICES[idx]!;
      return {
        name: svc.name,
        port: svc.port,
        status: "unhealthy" as const,
        latencyMs: null,
        lastCheckedAt: new Date(),
        error: "unexpected probe error",
      };
    });

    if (this._isFirstProbe) {
      this._isFirstProbe = false;
      this._state = newState;
      this._logStartupSummary(newState);
    } else {
      this._detectTransitions(this._state, newState);
      this._state = newState;
    }
  }

  /**
   * Return a shallow copy of the current fleet state snapshot.
   * Safe to call at any time (including before the first probe completes,
   * in which case an empty array is returned).
   */
  getSnapshot(): ServiceStatus[] {
    return [...this._state];
  }

  // ── Private helpers ─────────────────────────────────────────────────────────

  private _logStartupSummary(state: ServiceStatus[]): void {
    const healthy = state.filter((s) => s.status === "healthy");
    const unhealthy = state.filter((s) => s.status === "unhealthy");

    let summary = `Fleet health check: ${healthy.length}/${state.length} healthy`;
    if (unhealthy.length > 0) {
      const names = unhealthy.map((s) => `${s.name}:${s.port}`).join(", ");
      summary += ` — [unhealthy: ${names}]`;
    }

    this.logger.info({ service: "nova-daemon" }, summary);
  }

  private _detectTransitions(
    prev: ServiceStatus[],
    next: ServiceStatus[],
  ): void {
    const prevMap = new Map(prev.map((s) => [s.name, s]));

    for (const curr of next) {
      const before = prevMap.get(curr.name);

      if (!before) continue;

      if (before.status === "healthy" && curr.status === "unhealthy") {
        // Transition: healthy -> unhealthy
        this.logger.error(
          { service: "nova-daemon" },
          `Fleet service down: ${curr.name}:${curr.port} — ${curr.error ?? "unknown error"}`,
        );
        if (CRITICAL_SERVICE_NAMES.has(curr.name)) {
          this._notifyDown(curr);
        }
      } else if (before.status === "unhealthy" && curr.status === "healthy") {
        // Transition: unhealthy -> healthy
        this.logger.info(
          { service: "nova-daemon" },
          `Fleet service recovered: ${curr.name}:${curr.port} latency=${curr.latencyMs ?? 0}ms`,
        );
        if (CRITICAL_SERVICE_NAMES.has(curr.name)) {
          this._notifyRecovered(curr);
        }
      } else if (before.status === "unhealthy" && curr.status === "unhealthy") {
        // No change (sustained outage) — debug only to avoid log spam
        this.logger.debug(
          { service: "nova-daemon", name: curr.name, port: curr.port },
          `Fleet service still unhealthy: ${curr.name}:${curr.port}`,
        );
      }
      // healthy -> healthy: no log
    }
  }

  private _notifyDown(svc: ServiceStatus): void {
    if (
      !this.config.notifyOnCritical ||
      !this.telegram ||
      !this.telegramChatId
    ) {
      return;
    }

    const uptime = formatUptime(this._startedAt);
    const message = [
      `Fleet alert: ${svc.name}:${svc.port} is DOWN`,
      `Error: ${svc.error ?? "unknown"}`,
      `Daemon uptime: ${uptime}`,
    ].join("\n");

    void this.telegram
      .sendMessage(this.telegramChatId, message)
      .catch((err: unknown) => {
        this.logger.warn(
          { service: "nova-daemon", err },
          `Failed to send fleet-down Telegram alert for ${svc.name}`,
        );
      });
  }

  private _notifyRecovered(svc: ServiceStatus): void {
    if (
      !this.config.notifyOnCritical ||
      !this.telegram ||
      !this.telegramChatId
    ) {
      return;
    }

    const message = `Fleet recovered: ${svc.name}:${svc.port} is back (latency: ${svc.latencyMs ?? 0}ms)`;

    void this.telegram
      .sendMessage(this.telegramChatId, message)
      .catch((err: unknown) => {
        this.logger.warn(
          { service: "nova-daemon", err },
          `Failed to send fleet-recovered Telegram alert for ${svc.name}`,
        );
      });
  }
}
