# health-polling Specification

## Purpose
TBD - created by archiving change add-fleet-health-monitor. Update Purpose after archive.
## Requirements
### Requirement: FleetHealthMonitor probes all services on start and at a configurable interval

`packages/daemon/src/features/fleet-health/monitor.ts` MUST export a `FleetHealthMonitor` class with `start()`, `stop()`, `probe()`, and `getSnapshot()` methods. `start()` MUST fire `probe()` immediately (without waiting for the first interval tick), then schedule `setInterval` at `config.intervalMs`. `stop()` MUST clear the interval. `getSnapshot()` MUST return a shallow copy of the internal state array — never a mutable reference. The `probe()` method MUST call `fleetGet(port, "/health", 3000)` for all 9 fleet services concurrently via `Promise.allSettled` using a 3000ms per-probe timeout.

#### Scenario: Startup probe fires before first interval

Given a `FleetHealthMonitor` constructed with `intervalMs: 300000`,
when `start()` is called,
then `probe()` executes immediately and `getSnapshot()` returns non-null state before the first interval elapses.

#### Scenario: Unhealthy service is recorded with error detail

Given a fleet service on port 4101 that times out,
when `probe()` completes,
then `getSnapshot()` contains an entry for `memory-svc` with `status: "unhealthy"`, `latencyMs: null`, and `error: "timeout after 3000ms"`.

### Requirement: State transitions are logged and critical outages trigger Telegram notification

On each `probe()` pass, the monitor MUST log `error` level when a service transitions from healthy to unhealthy and `info` level when it recovers. Sustained unhealthy services SHALL NOT produce repeat log entries. At startup (first probe), a single `info` summary line MUST be logged in the format `"Fleet health check: N/M healthy"`. When a critical service (`tool-router`, `memory-svc`, or `graph-svc`) transitions to unhealthy and `config.notifyOnCritical` is `true`, the monitor MUST send a Telegram message to `telegramChatId` if both `telegram` and `telegramChatId` are provided.

#### Scenario: Healthy-to-unhealthy transition logs error and notifies

Given `memory-svc` was healthy on the previous probe and is now unreachable,
when `probe()` completes,
then an `error` log entry is emitted for `memory-svc` and a Telegram notification is sent containing `"Fleet alert: memory-svc:4101 is DOWN"`.

#### Scenario: Sustained outage does not spam logs

Given `discord-svc` has been unhealthy for two consecutive probe cycles,
when the third probe completes and the service is still unhealthy,
then no additional log entry is emitted for that service (only transitions are logged).

### Requirement: /health endpoint exposes fleet snapshot and reflects degraded status

The `GET /health` handler in `packages/daemon/src/http.ts` MUST include a `fleet` field sourced from `FleetHealthMonitor.getSnapshot()` when the monitor is wired via `HttpServerDeps.fleetHealthMonitor`. The handler MUST set the top-level `status` field to `"degraded"` when any critical service is unhealthy. The handler MUST never await a probe — it SHALL read only from the cached snapshot. When the monitor is absent or the first probe has not yet completed, `fleet` SHALL be `null` and `status` SHALL remain `"ok"`.

#### Scenario: Degraded status when critical service is down

Given `tool-router` is unhealthy in the current snapshot,
when `GET /health` is called,
then the response contains `"status": "degraded"` and the `fleet.services` array includes `tool-router` with `"status": "unhealthy"`.

