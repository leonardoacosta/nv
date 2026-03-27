import { probeFleetHealth } from "./health.js";

const startTime = Date.now();

function formatUptime(): string {
  const ms = Date.now() - startTime;
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  const days = Math.floor(hours / 24);

  if (days > 0) return `${days}d ${hours % 24}h ${minutes % 60}m`;
  if (hours > 0) return `${hours}h ${minutes % 60}m`;
  if (minutes > 0) return `${minutes}m ${seconds % 60}s`;
  return `${seconds}s`;
}

/**
 * /status — daemon uptime + fleet health
 */
export async function buildStatusReply(): Promise<string> {
  const daemonInfo = [
    "Daemon Status",
    "─".repeat(32),
    `  Uptime: ${formatUptime()}`,
    `  PID: ${process.pid}`,
    `  Memory: ${Math.round(process.memoryUsage().heapUsed / 1024 / 1024)}MB`,
    "",
  ].join("\n");

  let fleetInfo: string;
  try {
    fleetInfo = await probeFleetHealth();
  } catch {
    fleetInfo = "Fleet health check failed.";
  }

  return daemonInfo + "\n" + fleetInfo;
}
