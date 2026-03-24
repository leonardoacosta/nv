# Proposal: Add Server Health Metrics

## Change ID
`add-server-health-metrics`

## Depends On
`add-sqlite-migrations`

## Summary

Add persistent server health metrics collection via a `server_health` SQLite table (migration v3),
a 60-second poll loop that reads system metrics from `/proc` and stores snapshots, and an API
endpoint `/api/server-health` returning the latest snapshot plus historical data.

## Context
- Extends: `crates/nv-daemon/src/messages.rs` (migration), `crates/nv-daemon/src/health_poller.rs`, `crates/nv-daemon/src/http.rs`
- Related: Existing `HealthState` (in-memory daemon health), `ServerHealthStore` (already scaffolded), scope-lock Phase 5 (Monitoring)
- Depends on: `add-sqlite-migrations` (versioned migration framework must land first)

## Motivation

Nova currently tracks daemon health (uptime, channel connectivity) in memory via `HealthState`,
but has no persistent record of server-level metrics. Without historical CPU, memory, disk, and
uptime data:

1. **No crash detection** -- if uptime drops between polls, a restart happened silently
2. **No trend visibility** -- cannot spot gradual memory leaks or disk filling
3. **No dashboard data source** -- the dashboard health cards have no backend to query

Polling Nexus health every 60 seconds and storing snapshots in SQLite solves all three. The
`server_health` table provides the data layer that the dashboard monitoring spec builds on.

## Requirements

### Req-1: Migration (v3) -- server_health Table

Add a new migration to `messages_migrations()` creating the `server_health` table:

| Column | Type | Notes |
|--------|------|-------|
| `id` | INTEGER PK AUTOINCREMENT | Row identifier |
| `timestamp` | TEXT NOT NULL DEFAULT datetime('now') | ISO-8601 snapshot time |
| `cpu_percent` | REAL | CPU usage percentage (0-100) |
| `memory_used_mb` | INTEGER | Used memory in MB |
| `memory_total_mb` | INTEGER | Total memory in MB |
| `disk_used_gb` | REAL | Used disk in GB |
| `disk_total_gb` | REAL | Total disk in GB |
| `uptime_seconds` | INTEGER | Server uptime from /proc/uptime |
| `load_avg_1m` | REAL | 1-minute load average |
| `load_avg_5m` | REAL | 5-minute load average |

Index on `timestamp` for efficient history queries. All metric columns nullable to handle
partial collection failures gracefully.

### Req-2: Type Definitions

New file `crates/nv-daemon/src/server_health.rs` with:

- `ServerHealthSnapshot` -- serializable struct matching the table schema (id, timestamp, all metrics)
- `NewServerHealth` -- insert struct (no id/timestamp, those are auto-generated)
- `HealthStatus` enum (`Healthy`, `Degraded`, `Critical`) with threshold-based classification:
  - Critical: CPU >= 90% or memory >= 95%
  - Degraded: CPU >= 70% or memory >= 80%
  - Healthy: below both thresholds

### Req-3: Storage Layer (ServerHealthStore)

`ServerHealthStore` in `server_health.rs` (or separate `server_health_store.rs`):

- `new(db_path)` -- open SQLite connection with WAL mode
- `insert(health: &NewServerHealth)` -- insert snapshot, return row id
- `latest()` -- return most recent snapshot or None
- `previous()` -- return second-most-recent snapshot (for uptime comparison)
- `history_24h(limit)` -- return up to `limit` snapshots from last 24 hours, oldest first
- `prune_older_than_days(days)` -- delete old rows to prevent unbounded growth

### Req-4: Poll Loop

In `health_poller.rs`, implement `spawn_health_poller()`:

1. Runs on a 60-second `tokio::time::interval`
2. Skips the first immediate tick (let daemon settle)
3. Each tick: collect metrics from `/proc` (Linux-specific)
   - CPU: two `/proc/stat` reads 200ms apart for delta calculation
   - Memory: `/proc/meminfo` (MemTotal, MemAvailable)
   - Disk: `statvfs("/")` via libc (use `f_bavail` not `f_bfree`)
   - Uptime: `/proc/uptime`
   - Load: `/proc/loadavg`
4. Store snapshot via `ServerHealthStore::insert()`
5. Compare current uptime to previous -- if current < previous, server restarted (crash detection)
6. On crash: create P1 obligation via `ObligationStore`
7. Prune rows older than 7 days after each insert

### Req-5: API Endpoint

`GET /api/server-health` in `dashboard.rs`:

Response JSON:
```json
{
  "daemon": { /* existing HealthResponse */ },
  "latest": { /* ServerHealthSnapshot or null */ },
  "status": "healthy" | "degraded" | "critical",
  "history": [ /* up to 1440 snapshots from last 24h */ ]
}
```

The endpoint opens a read-only `ServerHealthStore`, fetches the latest snapshot, classifies
status via `HealthStatus::from_metrics()`, and loads 24h history.

## Scope
- **IN**: Migration, types, store, poll loop with crash detection, API endpoint, unit tests
- **OUT**: Dashboard UI (separate spec), Telegram alerting on degraded status, multi-host support

## Impact
| Area | Change |
|------|--------|
| `messages.rs` | Add migration v3 (server_health table + index) |
| `server_health.rs` (new) | Types (ServerHealthSnapshot, NewServerHealth, HealthStatus) |
| `server_health_store.rs` (new) | SQLite store (insert, latest, previous, history, prune) |
| `health_poller.rs` | Poll loop (collect, store, crash detection, prune) |
| `dashboard.rs` | GET /api/server-health endpoint |
| `main.rs` | Wire `spawn_health_poller()` into daemon startup |

## Risks
| Risk | Mitigation |
|------|-----------|
| /proc not available (containers, macOS) | All metric reads return Option; partial failures are fine |
| SQLite write contention with message store | WAL mode + separate Connection per store instance |
| Unbounded table growth | 7-day auto-prune on every poll cycle |
| CPU sample adds 200ms latency | Runs in background task, does not block any request path |
