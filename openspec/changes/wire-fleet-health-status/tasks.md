# Implementation Tasks

<!-- beads:epic:TBD -->

## DB Batch

- [x] [1.1] [P-1] Create `packages/db/src/schema/fleet-health-snapshots.ts` with `fleet_health_snapshots` table (id uuid PK, service_name text, status text, latency_ms integer, checked_at timestamptz) and index on (service_name, checked_at) [owner:db-engineer]
- [x] [1.2] [P-1] Export new schema from `packages/db/src/schema/index.ts` barrel and run `pnpm drizzle-kit generate` to produce migration [owner:db-engineer]

## API Batch

- [ ] [2.1] [P-1] Add `"daemon"` entry to `FLEET_URLS` in `packages/api/src/lib/fleet.ts` (env var `DAEMON_URL`, default `http://host.docker.internal:3443`) [owner:api-engineer]
- [ ] [2.2] [P-1] Replace static `fleetStatus` procedure in `packages/api/src/routers/system.ts` -- call `fleetFetch("meta-svc", "/services")`, map `ServiceHealthReport[]` to `FleetServiceStatus[]` (extract port from URL, map "unhealthy" to "unreachable", merge tools from static registry, add last_checked and uptime_secs), fall back to static registry on error [owner:api-engineer]
- [ ] [2.3] [P-1] Wire channel status in `fleetStatus` -- call `fleetFetch("daemon", "/channels/status")`, replace hardcoded `KNOWN_CHANNELS`, fall back to static array on error [owner:api-engineer]
- [ ] [2.4] [P-2] Add `channelVolume` tRPC procedure -- query messages table grouped by channel and hour for last 24h, merge totals into fleetStatus channel entries as `messages_24h` and `messages_per_hour` [owner:api-engineer]
- [ ] [2.5] [P-2] Add `errorRates` tRPC procedure -- query session_events for error/tool_error events in last 24h, return total, hourly buckets, and by_type breakdown [owner:api-engineer]
- [ ] [2.6] [P-2] Add `fleetHistory` tRPC procedure -- read: query fleet_health_snapshots for last 24h downsampled to 15min buckets (worst status per bucket), compute uptime_pct_24h per service; write: insert snapshot rows after each live meta-svc fetch in fleetStatus, delete rows older than 7 days [owner:api-engineer]

## UI Batch

- [ ] [3.1] [P-1] Update `FleetServiceStatus` in `apps/dashboard/types/api.ts` -- add `last_checked: string | null`, `uptime_secs: number | null` [owner:ui-engineer]
- [ ] [3.2] [P-1] Update `ChannelStatus` in `apps/dashboard/types/api.ts` -- add `"connected" | "disconnected" | "unconfigured"` to status union, add `messages_24h: number | null`, `messages_per_hour: number | null` [owner:ui-engineer]
- [ ] [3.3] [P-1] Add `ErrorRateResponse`, `ChannelVolumeResponse`, `FleetHistoryResponse` types to `apps/dashboard/types/api.ts` [owner:ui-engineer]
- [ ] [3.4] [P-1] Update `components/ServiceRow.tsx` -- show uptime_secs when available, show last_checked in expanded detail, add CSS transition on status dot background-color (300ms ease) [owner:ui-engineer]
- [ ] [3.5] [P-1] Update `components/ChannelRow.tsx` -- add status dot colors for connected/disconnected/unconfigured, show messages_24h count and messages_per_hour as right-aligned metrics [owner:ui-engineer]
- [ ] [3.6] [P-2] Update `app/integrations/page.tsx` -- add error rate summary line in Infrastructure section (query errorRates), add stale indicator when last fetch >60s ago, track previous status in ref map for transition detection [owner:ui-engineer]
- [ ] [3.7] [P-2] Add sparkline component to `ServiceRow` -- inline `<canvas>` or SVG (48x16px), 96 points from fleetHistory, green=healthy/red=unhealthy/gray=missing, uptime_pct_24h on hover via title attr [owner:ui-engineer]

## Daemon Batch

- [ ] [4.1] [P-1] Add channel registry to daemon -- track adapter initialization state (Telegram, Discord, Teams) with status enum (connected/configured/disconnected/unconfigured), expose via `HttpServerDeps` [owner:api-engineer]
- [ ] [4.2] [P-1] Add `GET /channels/status` to `packages/daemon/src/http.ts` -- return array of `{ name, status, direction }` from channel registry [owner:api-engineer]

## E2E Batch

(no E2E tasks -- dashboard has no E2E test suite)

## Verify

- [ ] [5.1] `pnpm drizzle-kit generate` produces clean migration for fleet_health_snapshots [owner:db-engineer]
- [ ] [5.2] `pnpm --filter @nova/api exec tsc --noEmit` passes [owner:api-engineer]
- [ ] [5.3] `pnpm --filter nova-dashboard exec tsc --noEmit` passes [owner:ui-engineer]
- [ ] [5.4] `pnpm --filter @nova/daemon exec tsc --noEmit` passes [owner:api-engineer]
- [ ] [5.5] fleetStatus returns live data when meta-svc is running, falls back gracefully when meta-svc is down [owner:api-engineer]
- [ ] [5.6] Channel status shows "connected" for Telegram when daemon is running with TELEGRAM_BOT_TOKEN set [owner:api-engineer]
