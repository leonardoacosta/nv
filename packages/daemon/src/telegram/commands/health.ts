import { fleetGet, FleetClientError } from "../../fleet-client.js";

const TOOL_ROUTER_PORT = 4100;

interface ServiceHealth {
  name: string;
  status: string;
  port?: number;
  latency_ms?: number;
}

/**
 * /health — fleet service health from tool-router
 */
export async function buildHealthReply(): Promise<string> {
  const data = await fleetGet(TOOL_ROUTER_PORT, "/health");

  // tool-router might return a flat status or a services array
  const services = (
    Array.isArray(data)
      ? data
      : Array.isArray((data as { services?: unknown }).services)
        ? (data as { services: unknown[] }).services
        : null
  ) as ServiceHealth[] | null;

  if (!services) {
    // Simple health check — just report the router itself
    const status =
      (data as { status?: string } | null)?.status ?? "ok";
    return `Fleet Health\n${"─".repeat(32)}\n  tool-router: ${status}`;
  }

  const header = `Fleet Health (${services.length} services)\n${"─".repeat(32)}\n`;
  const lines = services.map((s) => {
    const icon = s.status === "ok" || s.status === "healthy" ? "+" : "-";
    const latency = s.latency_ms !== undefined ? ` (${s.latency_ms}ms)` : "";
    const port = s.port !== undefined ? `:${s.port}` : "";
    return `  [${icon}] ${s.name}${port} ${s.status}${latency}`;
  });

  return header + lines.join("\n");
}

/**
 * Probe individual fleet services and return a health table.
 * Used as fallback when tool-router /health doesn't return per-service detail.
 */
export async function probeFleetHealth(): Promise<string> {
  const services = [
    { name: "tool-router", port: 4100 },
    { name: "memory-svc", port: 4101 },
    { name: "messages-svc", port: 4102 },
    { name: "channels-svc", port: 4103 },
    { name: "discord-svc", port: 4104 },
    { name: "teams-svc", port: 4105 },
    { name: "schedule-svc", port: 4106 },
    { name: "graph-svc", port: 4107 },
    { name: "meta-svc", port: 4108 },
  ];

  const results = await Promise.allSettled(
    services.map(async (svc) => {
      const start = Date.now();
      await fleetGet(svc.port, "/health");
      return { ...svc, status: "ok", latency: Date.now() - start };
    }),
  );

  const header = `Fleet Health (${services.length} services)\n${"─".repeat(32)}\n`;
  const lines = results.map((r, i) => {
    const svc = services[i]!;
    if (r.status === "fulfilled") {
      return `  [+] ${svc.name}:${svc.port} ok (${r.value.latency}ms)`;
    }
    const reason = r.reason;
    const code =
      reason instanceof FleetClientError ? reason.status : 503;
    return `  [-] ${svc.name}:${svc.port} unavailable (${code})`;
  });

  return header + lines.join("\n");
}
