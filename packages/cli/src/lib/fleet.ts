/** Fleet service health check helpers. */

export interface ServiceDef {
  name: string;
  port: number;
  url: string;
}

export interface HealthResult {
  name: string;
  port: number;
  healthy: boolean;
  latencyMs: number | null;
  error?: string;
}

export interface ChannelInfo {
  name: string;
  status: string;
}

/** All 10 fleet services (router + 9 downstream). */
export const FLEET_SERVICES: ServiceDef[] = [
  { name: "tool-router", port: 4100, url: "http://127.0.0.1:4100" },
  { name: "memory-svc", port: 4101, url: "http://127.0.0.1:4101" },
  { name: "messages-svc", port: 4102, url: "http://127.0.0.1:4102" },
  { name: "channels-svc", port: 4103, url: "http://127.0.0.1:4103" },
  { name: "discord-svc", port: 4104, url: "http://127.0.0.1:4104" },
  { name: "teams-svc", port: 4105, url: "http://127.0.0.1:4105" },
  { name: "schedule-svc", port: 4106, url: "http://127.0.0.1:4106" },
  { name: "graph-svc", port: 4107, url: "http://127.0.0.1:4107" },
  { name: "meta-svc", port: 4108, url: "http://127.0.0.1:4108" },
  { name: "azure-svc", port: 4109, url: "http://127.0.0.1:4109" },
];

/** Check a single service's /health endpoint. */
async function checkService(svc: ServiceDef, timeoutMs: number): Promise<HealthResult> {
  const start = performance.now();
  try {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), timeoutMs);
    const res = await fetch(`${svc.url}/health`, { signal: controller.signal });
    clearTimeout(timer);
    const latencyMs = Math.round(performance.now() - start);
    return {
      name: svc.name,
      port: svc.port,
      healthy: res.ok,
      latencyMs,
    };
  } catch (err) {
    return {
      name: svc.name,
      port: svc.port,
      healthy: false,
      latencyMs: null,
      error: err instanceof Error ? err.message : String(err),
    };
  }
}

/** Check all fleet services in parallel. */
export async function checkFleet(timeoutMs = 3000): Promise<HealthResult[]> {
  return Promise.all(FLEET_SERVICES.map((svc) => checkService(svc, timeoutMs)));
}

/** Fetch channel statuses from channels-svc. */
export async function getChannels(timeoutMs = 3000): Promise<ChannelInfo[]> {
  try {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), timeoutMs);
    const res = await fetch("http://127.0.0.1:4103/channels", {
      signal: controller.signal,
    });
    clearTimeout(timer);
    if (!res.ok) return [];
    const data = (await res.json()) as { channels?: ChannelInfo[] };
    return data.channels ?? [];
  } catch {
    return [];
  }
}
