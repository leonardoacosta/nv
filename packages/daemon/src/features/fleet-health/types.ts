// ─── ServiceStatus ────────────────────────────────────────────────────────────

export interface ServiceStatus {
  name: string;
  port: number;
  status: "healthy" | "unhealthy";
  latencyMs: number | null;
  lastCheckedAt: Date;
  error?: string;
}

// ─── FleetHealthMonitorConfig ─────────────────────────────────────────────────

export interface FleetHealthMonitorConfig {
  /** Whether the fleet health monitor is active. Default: true */
  enabled: boolean;
  /** How often to probe all fleet services (ms). Default: 300_000 (5 minutes) */
  intervalMs: number;
  /** Per-probe HTTP timeout (ms). Default: 3000 */
  probeTimeoutMs: number;
  /** Send Telegram notification when a critical service changes state. Default: true */
  notifyOnCritical: boolean;
}

export const defaultFleetHealthMonitorConfig: FleetHealthMonitorConfig = {
  enabled: true,
  intervalMs: 300_000,
  probeTimeoutMs: 3000,
  notifyOnCritical: true,
};
