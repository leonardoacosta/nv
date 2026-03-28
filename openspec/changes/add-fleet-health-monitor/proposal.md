# Proposal: Add Fleet Health Monitor

## Change ID
`add-fleet-health-monitor`

## Priority
P2

## Summary

Add a `FleetHealthMonitor` to the daemon that probes all fleet services at startup, rechecks
every 5 minutes (configurable), logs state transitions, exposes fleet health in the `/health`
HTTP endpoint, and optionally sends a Telegram notification when a critical service goes down.

## Context
- Extends: `packages/daemon/src/fleet-client.ts`, `packages/daemon/src/http.ts`,
  `packages/daemon/src/config.ts`, `packages/daemon/src/index.ts`
- Reuses: `probeFleetHealth()` in `packages/daemon/src/telegram/commands/health.ts` (service
  list + probe mechanics already established there — extract and centralise)
- Related: `add-fleet-client-retry` (completed — retry logic reduces false positives during
  deploys), `wire-fleet-health-status` (dashboard Status page — consumes the `/health` endpoint
  extended by this spec)
- Config section: `[fleet_health_monitor]` in `~/.nv/config/nv.toml`

## Motivation

Fleet services (10 Hono microservices on ports 4100–4109) can go down silently. When a service
fails, the daemon discovers it only when an incoming message triggers a fleet call — which then
hits the 5-second `fleetGet` timeout before erroring out. With `add-fleet-client-retry` in
place, that becomes a 10.5-second blocking penalty per message, plus potentially expensive Agent
SDK fallback.

The deploy script already probes fleet health post-deploy, proving the mechanic works. What is
missing is a live monitor inside the daemon that:

1. Surfaces unhealthy services at startup rather than at the first affected request
2. Logs state changes (healthy→unhealthy, unhealthy→healthy) so degradation is visible in
   structured logs without digging through request errors
3. Exposes the current fleet snapshot in `/health` so the dashboard Status page and ops tooling
   can consume it without a separate fleet call
4. Notifies via Telegram when a critical service transitions to unhealthy, giving enough lead
   time to intervene before messages start timing out

## Requirements

### Req-1: FleetHealthMonitor class (`src/features/fleet-health/monitor.ts`)

```typescript
export interface ServiceStatus {
  name: string;
  port: number;
  status: "healthy" | "unhealthy";
  latencyMs: number | null;
  lastCheckedAt: Date;
  error?: string;
}

export class FleetHealthMonitor {
  constructor(
    private config: FleetHealthMonitorConfig,
    private logger: Logger,
    private telegram?: TelegramAdapter,
    private telegramChatId?: string,
  ) {}

  start(): void;                    // probe immediately, then set interval
  stop(): void;                     // clear interval
  async probe(): Promise<void>;     // one full probe pass — exposed for testing
  getSnapshot(): ServiceStatus[];   // current state, safe to call any time
}
```

- `start()` fires `probe()` immediately (does not wait for the first interval), then schedules
  `setInterval(() => this.probe(), intervalMs)` where `intervalMs = config.intervalMs`
- `stop()` calls `clearInterval` on the stored handle; no-op if not running
- `getSnapshot()` returns a shallow copy of the internal state array — never a mutable reference

### Req-2: Service registry

The monitor owns the canonical service registry. Define it as a constant in the same module:

```typescript
const FLEET_SERVICES: readonly { name: string; port: number }[] = [
  { name: "tool-router",   port: 4100 },
  { name: "memory-svc",    port: 4101 },
  { name: "messages-svc",  port: 4102 },
  { name: "channels-svc",  port: 4103 },
  { name: "discord-svc",   port: 4104 },
  { name: "teams-svc",     port: 4105 },
  { name: "schedule-svc",  port: 4106 },
  { name: "graph-svc",     port: 4107 },
  { name: "meta-svc",      port: 4108 },
];
```

This replaces the duplicate array in `packages/daemon/src/telegram/commands/health.ts` —
`probeFleetHealth()` in that file should import `FLEET_SERVICES` from the monitor module instead
of defining its own. (The Telegram command continues to work unchanged; the list is just sourced
from one place.)

Note: port 4109 is intentionally absent — it is unassigned in the current v10 fleet.

### Req-3: Probe implementation

`probe()` calls `fleetGet(port, "/health", 3000)` for each service concurrently via
`Promise.allSettled`. The per-probe timeout is 3000ms (shorter than the 5000ms default, since
health checks should be fast and we do not want the monitor to block under heavy fleet load).

For each result:

| Outcome | `ServiceStatus.status` | `latencyMs` | `error` |
|---------|----------------------|-------------|---------|
| Resolved within timeout | `"healthy"` | elapsed ms | absent |
| Timeout (AbortError) | `"unhealthy"` | `null` | `"timeout after 3000ms"` |
| Non-2xx (FleetClientError) | `"unhealthy"` | `null` | HTTP status + message |
| Network error | `"unhealthy"` | `null` | error message |

After all probes settle, compare the new results against `this._state` to detect transitions and
update `lastCheckedAt` on every entry regardless of transition.

### Req-4: State change logging

On each `probe()` pass, after collecting results, log at the appropriate level for each service:

| Transition | Level | Message |
|-----------|-------|---------|
| healthy → unhealthy | `error` | `"Fleet service down: {name}:{port} — {error}"` |
| unhealthy → healthy | `info` | `"Fleet service recovered: {name}:{port} latency={latencyMs}ms"` |
| unhealthy → unhealthy (no change) | `debug` | (no log — avoid log spam on sustained outage) |
| healthy → healthy (no change) | (no log) | |

At startup (first probe, no prior state), log a single `info` summary line:

```
Fleet health check: N/M healthy — [unhealthy: name1:port1, name2:port2]
```

If all services are healthy, the summary is `Fleet health check: N/N healthy`.

### Req-5: `/health` endpoint extension

Extend the existing `GET /health` handler in `packages/daemon/src/http.ts` to include fleet
status when `FleetHealthMonitor` is wired in:

```jsonc
// Current response
{ "status": "ok", "service": "nova-daemon", "uptime_secs": 1234 }

// Extended response (when monitor is active)
{
  "status": "ok",               // "degraded" if any critical service is unhealthy
  "service": "nova-daemon",
  "uptime_secs": 1234,
  "fleet": {
    "summary": { "healthy": 8, "unhealthy": 1, "total": 9 },
    "last_checked_at": "2026-03-27T10:00:00.000Z",
    "services": [
      { "name": "tool-router",  "port": 4100, "status": "healthy",   "latency_ms": 2 },
      { "name": "memory-svc",   "port": 4101, "status": "unhealthy", "latency_ms": null, "error": "timeout after 3000ms" },
      ...
    ]
  }
}
```

The top-level `status` field changes from `"ok"` to `"degraded"` if any **critical** service is
unhealthy (see Req-7 for the critical service list). Non-critical unhealthy services do not
change the top-level status.

When `FleetHealthMonitor` is not yet initialised (first-probe not yet complete), `fleet` is
`null` and `status` remains `"ok"`.

### Req-6: HttpServerDeps extension

Add the monitor to `HttpServerDeps` in `packages/daemon/src/http.ts`:

```typescript
export interface HttpServerDeps {
  agent: NovaAgent;
  conversationManager: ConversationManager;
  config: Config;
  logger: Logger;
  briefingDeps?: BriefingDeps;
  fleetHealthMonitor?: FleetHealthMonitor;   // NEW — optional; absent = no fleet section in /health
}
```

Pass `getSnapshot()` data into the `/health` handler. The handler must never await a probe — it
reads only from the cached snapshot.

### Req-7: Telegram notification for critical service outages

When a critical service transitions healthy → unhealthy, send a Telegram message to
`config.telegramChatId` if:
- `config.fleetHealthMonitor.notifyOnCritical` is `true`
- `telegram` and `telegramChatId` are both provided in the constructor

Critical services:

| Service | Reason |
|---------|--------|
| `tool-router` | Gateway for all fleet tool calls |
| `memory-svc` | Long-term memory; loss degrades every conversation |
| `graph-svc` | Relationship graph; loss breaks contact/project context |

Message format (plain text):

```
Fleet alert: memory-svc:4101 is DOWN
Error: timeout after 3000ms
Daemon uptime: 2h 14m
```

Recovery notification (when service returns to healthy):

```
Fleet recovered: memory-svc:4101 is back (latency: 4ms)
```

Non-critical service outages are logged but do not generate Telegram notifications.

### Req-8: Config type (`src/features/fleet-health/types.ts`)

```typescript
export interface FleetHealthMonitorConfig {
  enabled: boolean;          // default: true
  intervalMs: number;        // default: 300_000 (5 minutes)
  probeTimeoutMs: number;    // default: 3000
  notifyOnCritical: boolean; // default: true
}

export const defaultFleetHealthMonitorConfig: FleetHealthMonitorConfig = {
  enabled: true,
  intervalMs: 300_000,
  probeTimeoutMs: 3000,
  notifyOnCritical: true,
};
```

### Req-9: Config loader integration

Extend `packages/daemon/src/config.ts`:

- Import `FleetHealthMonitorConfig`, `defaultFleetHealthMonitorConfig` from
  `./features/fleet-health/types.js`
- Add `fleetHealthMonitor: FleetHealthMonitorConfig` to the `Config` interface
- Add `fleetHealthMonitor` to the `Omit` list in `DEFAULTS` (it has its own default object)
- Add `TomlConfig.fleet_health_monitor` section:

```typescript
fleet_health_monitor?: {
  enabled?: boolean;
  interval_ms?: number;
  probe_timeout_ms?: number;
  notify_on_critical?: boolean;
};
```

- In `loadConfig()`, build `fleetHealthMonitor` from TOML + defaults (same pattern as
  `proactiveWatcher`):

```typescript
const fleetHealthMonitor: FleetHealthMonitorConfig = {
  enabled:           toml.fleet_health_monitor?.enabled           ?? defaultFleetHealthMonitorConfig.enabled,
  intervalMs:        toml.fleet_health_monitor?.interval_ms       ?? defaultFleetHealthMonitorConfig.intervalMs,
  probeTimeoutMs:    toml.fleet_health_monitor?.probe_timeout_ms  ?? defaultFleetHealthMonitorConfig.probeTimeoutMs,
  notifyOnCritical:  toml.fleet_health_monitor?.notify_on_critical ?? defaultFleetHealthMonitorConfig.notifyOnCritical,
};
```

- Add `fleetHealthMonitor` to the returned `Config` object.
- Export `FleetHealthMonitorConfig` from the `export type { ... }` line.

### Req-10: Daemon wiring (`src/index.ts`)

After `initFleetClient()` and after the Telegram adapter is constructed (monitor optionally
receives `telegram` and `telegramChatId` for Req-7):

```typescript
// ── Fleet health monitor ───────────────────────────────────────────────────
let fleetHealthMonitor: FleetHealthMonitor | null = null;

if (config.fleetHealthMonitor.enabled) {
  fleetHealthMonitor = new FleetHealthMonitor(
    config.fleetHealthMonitor,
    log,
    telegram ?? undefined,
    config.telegramChatId,
  );
  fleetHealthMonitor.start();
  log.info(
    { service: "nova-daemon", intervalMs: config.fleetHealthMonitor.intervalMs },
    "Fleet health monitor started",
  );
}
```

Pass `fleetHealthMonitor ?? undefined` into `createHttpApp` via `HttpServerDeps`.

In the `shutdown()` handler, call `fleetHealthMonitor?.stop()` before closing the HTTP server.

### Req-11: Barrel export (`src/features/fleet-health/index.ts`)

Re-export `FleetHealthMonitor`, `FleetHealthMonitorConfig`, `defaultFleetHealthMonitorConfig`,
`ServiceStatus`, and `FLEET_SERVICES` from the index barrel so `index.ts` imports cleanly:

```typescript
import { FleetHealthMonitor } from "./features/fleet-health/index.js";
```

## Scope

- **IN**: `FleetHealthMonitor` class, service registry constant, startup + periodic probe,
  state-change logging, `/health` endpoint extension, Telegram notification for critical
  services, `FleetHealthMonitorConfig` type, config loader integration, daemon wiring,
  deduplication of service list from `health.ts`
- **OUT**: Per-service health history / snapshots (separate `wire-fleet-health-status` spec),
  circuit-breaker or automatic fleet service restart, health check via tool-router aggregation
  endpoint (monitor probes directly to avoid depending on the router being healthy), alerting for
  non-critical services via Telegram, dashboard UI changes (covered by `wire-fleet-health-status`)

## Impact

| Area | Change |
|------|--------|
| `packages/daemon/src/features/fleet-health/types.ts` | New: `FleetHealthMonitorConfig`, `defaultFleetHealthMonitorConfig`, `ServiceStatus` |
| `packages/daemon/src/features/fleet-health/monitor.ts` | New: `FleetHealthMonitor` class, `FLEET_SERVICES` registry |
| `packages/daemon/src/features/fleet-health/index.ts` | New: barrel re-export |
| `packages/daemon/src/telegram/commands/health.ts` | Modified: import `FLEET_SERVICES` from monitor module; remove duplicate array |
| `packages/daemon/src/http.ts` | Modified: `HttpServerDeps.fleetHealthMonitor?`, extend `/health` response with fleet snapshot |
| `packages/daemon/src/config.ts` | Modified: add `FleetHealthMonitorConfig` import + export, `fleetHealthMonitor` field, TOML mapping |
| `packages/daemon/src/index.ts` | Modified: instantiate + start monitor, pass to `createHttpApp`, stop on shutdown |

No DB schema changes. No changes to fleet service implementations. No changes to `apps/`.

## Risks

| Risk | Mitigation |
|------|-----------|
| Probe interval adds CPU/network overhead | 9 services × 1 probe per 5 minutes = negligible; each probe resolves in <3s with concurrent `Promise.allSettled` |
| Startup probe delays daemon ready log | `probe()` is fire-and-forget from `start()` — startup log appears immediately; probe completes in the background |
| Telegram notification storm if many services go down simultaneously | Each service sends its own message; at most 3 critical services = 3 messages on a catastrophic failure; acceptable |
| False positive notifications during rolling redeploy | `add-fleet-client-retry` reduces transient failures; probe timeout is 3s — a deploy restart window is typically <2s; sustained outage threshold is one full probe cycle |
| `health.ts` Telegram command still works after service list refactor | `probeFleetHealth()` imports `FLEET_SERVICES` from the new module — same list, same probe logic; no behaviour change |
