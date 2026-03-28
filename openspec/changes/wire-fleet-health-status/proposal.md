# Proposal: Wire Fleet Health Status

## Change ID
`wire-fleet-health-status`

## Summary

Replace the hardcoded fleet status endpoint with live health data from meta-svc, add real channel
connection status from the daemon, per-channel message volume from DB aggregation, error rate
tracking from session events, and historical uptime sparklines on the Status page.

## Context
- Extends: `packages/api/src/routers/system.ts` (fleetStatus procedure), `packages/api/src/lib/fleet.ts` (fleet HTTP helper), `apps/dashboard/app/integrations/page.tsx` (Status page), `apps/dashboard/components/ServiceRow.tsx`, `apps/dashboard/components/ChannelRow.tsx`, `apps/dashboard/types/api.ts`
- Consumes: `packages/tools/meta-svc` (`GET /services` returns per-service health with latency and status), `packages/daemon/src/http.ts` (daemon HTTP server)
- Related: `redesign-integrations-status` (created the current static page -- this spec wires it to real data)
- DB schemas: `packages/db/src/schema/messages.ts` (channel volume), `packages/db/src/schema/session-events.ts` (error tracking), `packages/db/src/schema/sessions.ts` (daemon uptime)

## Motivation

The Status page was built during the `redesign-integrations-status` spec with the explicit caveat
that fleet services run on the host network and are unreachable from the Docker-hosted dashboard.
The solution was static registry data with `status: "unknown"` everywhere. Now that the tRPC API
layer has `fleetFetch()` (which resolves `host.docker.internal` URLs), the dashboard API can proxy
real health checks from meta-svc.

Problems with the current page:

1. **Fleet status is fake** -- every service shows "host only" with no latency, no health check
2. **Channel status is fake** -- hardcoded array says "configured" for Telegram/Discord/Teams with no connection verification
3. **No error visibility** -- no error rates, no failure counts, no way to spot degradation
4. **No volume metrics** -- no indication of channel activity (messages/hour)
5. **No history** -- impossible to tell if a service was down 2 hours ago and recovered

meta-svc already has `probeFleet()` which hits `/health` on all 9 fleet services and returns
per-service status, latency, and uptime. The daemon knows which channels are actually initialized.
The messages table has channel and timestamp columns for volume queries. Session events track
errors. All the data exists -- it just needs wiring.

## Requirements

### Req-1: Wire fleetStatus to meta-svc Health Probes

Replace the static `fleetStatus` tRPC procedure in `packages/api/src/routers/system.ts` with a
live call to meta-svc `GET /services` via `fleetFetch("meta-svc", "/services")`.

The meta-svc response shape (`{ services: ServiceHealthReport[], summary: FleetHealthSummary }`)
must be mapped to the existing `FleetHealthResponse` type contract:

```typescript
// meta-svc returns:
{ name, url, status: "healthy"|"unhealthy"|"unreachable", latency_ms, uptime_secs?, error? }

// Dashboard expects:
{ name, url, port, status: "healthy"|"unreachable"|"unknown", latency_ms, tools }
```

Mapping:
- Extract `port` from `url` (parse URL)
- Map `"unhealthy"` to `"unreachable"` (dashboard only has three states)
- Preserve `tools` from the existing static `FLEET_SERVICES` registry (meta-svc does not return tool lists)
- Add `last_checked` (ISO timestamp) and `uptime_secs` (nullable) to `FleetServiceStatus`

If meta-svc is unreachable (fleetFetch throws), fall back to the current static registry with
`status: "unknown"` so the page still renders.

### Req-2: Add Channel Connection Status from Daemon

Add `GET /channels/status` to the daemon HTTP server (`packages/daemon/src/http.ts`). Returns
actual connection state for each channel adapter:

```json
[
  { "name": "Telegram", "status": "connected", "direction": "bidirectional" },
  { "name": "Discord", "status": "connected", "direction": "bidirectional" },
  { "name": "Microsoft Teams", "status": "configured", "direction": "bidirectional" }
]
```

Status values:
- `"connected"` -- adapter initialized and polling/webhook active
- `"configured"` -- env var present but adapter not verified as connected
- `"disconnected"` -- adapter failed to initialize or was disabled
- `"unconfigured"` -- env var missing

The daemon has the adapter instances in scope (`telegram: TelegramAdapter | null` in `index.ts`).
Expose a channel registry that tracks which adapters were successfully created.

Add `"daemon"` to the `FLEET_URLS` map in `packages/api/src/lib/fleet.ts` (env var
`DAEMON_URL`, default `http://host.docker.internal:3443`). The `fleetStatus` tRPC procedure
calls `fleetFetch("daemon", "/channels/status")` and merges the result into the response,
replacing the hardcoded `KNOWN_CHANNELS` array.

Fallback: if the daemon is unreachable, return the current static `KNOWN_CHANNELS` with
`status: "configured"`.

### Req-3: Per-Channel Message Volume

Add a `channelVolume` tRPC procedure to the system router. Queries the `messages` table for
message counts grouped by channel over the last 24 hours, bucketed by hour:

```sql
SELECT channel, date_trunc('hour', created_at) AS hour, count(*)::int AS count
FROM messages
WHERE created_at >= NOW() - INTERVAL '24 hours'
GROUP BY channel, hour
ORDER BY channel, hour
```

Response shape:

```typescript
interface ChannelVolumeResponse {
  channels: {
    name: string;
    total_24h: number;
    hourly: { hour: string; count: number }[];
  }[];
}
```

The `fleetStatus` procedure merges volume data into the channel entries by matching `channel`
(lowercase) to channel name. Add `messages_24h: number` and `messages_per_hour: number` (current
hour count or average) to `ChannelStatus`.

### Req-4: Error Rate from Session Events

Add an `errorRates` tRPC procedure. Queries `session_events` for events with
`event_type = 'error'` or `event_type = 'tool_error'` in the last 24 hours, grouped by hour:

```typescript
interface ErrorRateResponse {
  total_24h: number;
  hourly: { hour: string; count: number }[];
  by_type: { event_type: string; count: number }[];
}
```

The Status page displays this as a compact summary line in the Infrastructure section:
"N errors in last 24h" with a subtle red/amber/green indicator (red if >10, amber if >0, green
if 0).

### Req-5: Update Dashboard Types and Components

Update `apps/dashboard/types/api.ts`:
- Add `last_checked: string | null` and `uptime_secs: number | null` to `FleetServiceStatus`
- Add `"connected" | "disconnected" | "unconfigured"` to `ChannelStatus["status"]` union
- Add `messages_24h: number | null` and `messages_per_hour: number | null` to `ChannelStatus`
- Add `ErrorRateResponse` type

Update `components/ServiceRow.tsx`:
- Show `uptime_secs` when available (formatted as "Xd Xh" or "Xh Xm")
- Show per-service `last_checked` timestamp in the expanded detail section
- Add visual transition: when status changes between polls, briefly flash the status dot
  (CSS `transition` on background-color, 300ms ease)

Update `components/ChannelRow.tsx`:
- Add status dot colors for new states: green=connected, gray=configured, red=disconnected, dim=unconfigured
- Show `messages_24h` count as a right-aligned metric (e.g., "142 msgs/24h")
- Show `messages_per_hour` as a secondary metric (e.g., "~6/hr")

### Req-6: Auto-Refresh with Visual Transition

The page already polls every 30s via `refetchInterval: POLL_INTERVAL_MS`. Enhance with:

- **Transition indicators**: When a service status changes between polls, apply a 1s highlight
  animation (subtle background flash) on the changed row. Track previous status in a ref map and
  compare on each data update.
- **Stale indicator**: If the last successful fetch was >60s ago (e.g., network issue preventing
  refresh), show an amber "Stale" badge next to the "Last checked" label.

### Req-7: Historical Uptime Sparklines

Add a `fleetHistory` tRPC procedure that stores and retrieves fleet health snapshots.

**Storage**: Create a new `fleet_health_snapshots` table in `packages/db/src/schema/`:

```typescript
// fleet-health-snapshots.ts
export const fleetHealthSnapshots = pgTable("fleet_health_snapshots", {
  id: uuid("id").primaryKey().defaultRandom(),
  serviceName: text("service_name").notNull(),
  status: text("status").notNull(), // "healthy" | "unhealthy" | "unreachable"
  latencyMs: integer("latency_ms"),
  checkedAt: timestamp("checked_at", { withTimezone: true }).notNull().defaultNow(),
});
```

**Write path**: The `fleetStatus` procedure, after fetching live health from meta-svc, inserts
one row per service into `fleet_health_snapshots`. This happens on every call (every 30s when
the Status page is open). Add a retention policy: delete snapshots older than 7 days (run as
part of the insert query or as a scheduled cleanup).

**Read path**: `fleetHistory` returns the last 24h of snapshots per service, downsampled to
one point per 15-minute bucket (take the worst status in each bucket):

```typescript
interface FleetHistoryResponse {
  services: {
    name: string;
    snapshots: { time: string; status: string; latency_ms: number | null }[];
    uptime_pct_24h: number; // percentage of 15min buckets where status was "healthy"
  }[];
}
```

**UI**: Render a tiny inline sparkline (48px wide, 16px tall) next to each service name in
`ServiceRow`. Use a `<canvas>` element or inline SVG with 96 points (one per 15min over 24h).
Color: green segments for healthy, red for unhealthy/unreachable, gray for missing data.
Show `uptime_pct_24h` as a percentage label on hover (title attribute).

## Scope
- **IN**: Live fleet health from meta-svc, daemon channel status endpoint, channel message volume aggregation, error rate tracking, type updates, component updates, sparkline rendering, fleet_health_snapshots schema + migration, visual transition indicators, stale state handling
- **OUT**: Alerting/notifications on status changes (future), fleet service restart/management from UI, per-service error breakdown (would need tool-level error tracking), Discord/Teams bot connection verification (would need SDK-level health checks in those services -- for now inferred from adapter initialization), custom polling intervals per service

## Impact
| Area | Change |
|------|--------|
| `packages/db/src/schema/fleet-health-snapshots.ts` | New: snapshot table for historical uptime |
| `packages/db/drizzle/` | New: generated migration for fleet_health_snapshots |
| `packages/api/src/routers/system.ts` | Modified: fleetStatus calls meta-svc + daemon, new channelVolume + errorRates + fleetHistory procedures |
| `packages/api/src/lib/fleet.ts` | Modified: add "daemon" to FLEET_URLS |
| `packages/daemon/src/http.ts` | Modified: add GET /channels/status |
| `packages/daemon/src/index.ts` | Modified: expose channel registry to HTTP server deps |
| `apps/dashboard/types/api.ts` | Modified: extend FleetServiceStatus, ChannelStatus; add ErrorRateResponse, ChannelVolumeResponse, FleetHistoryResponse |
| `apps/dashboard/components/ServiceRow.tsx` | Modified: uptime display, sparkline, last_checked, transition animation |
| `apps/dashboard/components/ChannelRow.tsx` | Modified: new status colors, message volume display |
| `apps/dashboard/app/integrations/page.tsx` | Modified: error rate section, stale indicator, transition tracking |

## Risks
| Risk | Mitigation |
|------|-----------|
| meta-svc unreachable (fleet services not running) | Fallback to static registry with "unknown" status -- same as current behavior, no regression |
| Daemon unreachable (channel status unavailable) | Fallback to static KNOWN_CHANNELS with "configured" status |
| fleet_health_snapshots grows unbounded | 7-day retention delete runs on each insert batch; at 10 services x 2880 checks/day = ~28,800 rows/day, 7 days = ~200K rows -- trivial for Postgres |
| Message volume query slow on large messages table | Query uses `created_at >= NOW() - INTERVAL '24 hours'` which hits the existing timestamp index; 24h window keeps the scan bounded |
| Sparkline rendering performance | Canvas/SVG with 96 points is lightweight; no external charting library needed |
| fleetFetch timeout adds latency to page load | meta-svc probe has 5s timeout; Status page renders immediately with skeleton, fleet data fills in async via React Query |
