import type { ServiceHealthReport, FleetHealthSummary } from "./types.js";

interface ServiceEntry {
  name: string;
  port: number;
}

const SERVICE_REGISTRY: readonly ServiceEntry[] = [
  { name: "tool-router", port: 4100 },
  { name: "memory-svc", port: 4101 },
  { name: "messages-svc", port: 4102 },
  { name: "channels-svc", port: 4103 },
  { name: "discord-svc", port: 4104 },
  { name: "teams-svc", port: 4105 },
  { name: "schedule-svc", port: 4106 },
  { name: "graph-svc", port: 4107 },
] as const;

const PROBE_TIMEOUT_MS = 3000;

async function probeService(
  name: string,
  url: string,
): Promise<ServiceHealthReport> {
  const start = performance.now();
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), PROBE_TIMEOUT_MS);

  try {
    const response = await fetch(url, { signal: controller.signal });
    const latency_ms = Math.round(performance.now() - start);

    if (response.ok) {
      try {
        const body = (await response.json()) as Record<string, unknown>;
        return {
          name,
          url,
          status: "healthy",
          uptime_secs:
            typeof body["uptime_secs"] === "number"
              ? body["uptime_secs"]
              : typeof body["uptime"] === "number"
                ? body["uptime"]
                : undefined,
          latency_ms,
        };
      } catch {
        return { name, url, status: "healthy", latency_ms };
      }
    }

    return {
      name,
      url,
      status: "unhealthy",
      latency_ms,
      error: `${response.status} ${response.statusText}`,
    };
  } catch (err) {
    const latency_ms = Math.round(performance.now() - start);
    const message =
      err instanceof Error ? err.message : "Unknown network error";
    return { name, url, status: "unreachable", latency_ms, error: message };
  } finally {
    clearTimeout(timeout);
  }
}

export async function probeFleet(): Promise<ServiceHealthReport[]> {
  const results = await Promise.allSettled(
    SERVICE_REGISTRY.map((svc) =>
      probeService(svc.name, `http://localhost:${svc.port}/health`),
    ),
  );

  return results.map((result, i) => {
    if (result.status === "fulfilled") {
      return result.value;
    }
    const svc = SERVICE_REGISTRY[i]!;
    return {
      name: svc.name,
      url: `http://localhost:${svc.port}/health`,
      status: "unreachable" as const,
      latency_ms: 0,
      error: result.reason instanceof Error ? result.reason.message : "Unknown error",
    };
  });
}

export function summarizeFleet(
  reports: ServiceHealthReport[],
): FleetHealthSummary {
  return {
    total: reports.length,
    healthy: reports.filter((r) => r.status === "healthy").length,
    unhealthy: reports.filter((r) => r.status === "unhealthy").length,
    unreachable: reports.filter((r) => r.status === "unreachable").length,
  };
}
