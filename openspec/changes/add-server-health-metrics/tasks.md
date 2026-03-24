# Implementation Tasks

<!-- beads:epic:TBD -->

## Migration

- [ ] [1.1] [P-1] Add migration v3 to messages_migrations() in messages.rs -- CREATE TABLE server_health with columns (id, timestamp, cpu_percent, memory_used_mb, memory_total_mb, disk_used_gb, disk_total_gb, uptime_seconds, load_avg_1m, load_avg_5m) + CREATE INDEX idx_server_health_timestamp [owner:api-engineer]

## Types

- [ ] [2.1] [P-1] Create crates/nv-daemon/src/server_health.rs -- ServerHealthSnapshot struct (id, timestamp, cpu_percent, memory_used_mb, memory_total_mb, disk_used_gb, disk_total_gb, uptime_seconds, load_avg_1m, load_avg_5m) with Serialize/Deserialize [owner:api-engineer]
- [ ] [2.2] [P-1] Add NewServerHealth struct -- insert values without id/timestamp (all metric fields as Option) [owner:api-engineer]
- [ ] [2.3] [P-2] Add HealthStatus enum (Healthy, Degraded, Critical) with from_metrics() classifier -- Critical: CPU >= 90% or mem >= 95%, Degraded: CPU >= 70% or mem >= 80% [owner:api-engineer]

## Storage

- [ ] [3.1] [P-1] Create crates/nv-daemon/src/server_health_store.rs -- ServerHealthStore with new(db_path), WAL pragma [owner:api-engineer]
- [ ] [3.2] [P-1] Add insert(&NewServerHealth) method -- INSERT INTO server_health with datetime('now') timestamp, return row id [owner:api-engineer]
- [ ] [3.3] [P-2] Add latest() method -- SELECT ... ORDER BY id DESC LIMIT 1, return Option<ServerHealthSnapshot> [owner:api-engineer]
- [ ] [3.4] [P-2] Add previous() method -- SELECT ... ORDER BY id DESC LIMIT 1 OFFSET 1 for uptime comparison [owner:api-engineer]
- [ ] [3.5] [P-2] Add history_24h(limit) method -- SELECT ... WHERE timestamp >= datetime('now', '-24 hours') ORDER BY id ASC [owner:api-engineer]
- [ ] [3.6] [P-3] Add prune_older_than_days(days) method -- DELETE WHERE timestamp < datetime('now', '-N days') [owner:api-engineer]

## Poll Loop

- [ ] [4.1] [P-1] Implement spawn_health_poller(db_path, obligation_store, nexus_client) in health_poller.rs -- 60s interval, skip first tick [owner:api-engineer]
- [ ] [4.2] [P-1] Implement collect_metrics() -- read CPU (/proc/stat delta with 200ms sleep), memory (/proc/meminfo), disk (statvfs /), uptime (/proc/uptime), loadavg (/proc/loadavg) [owner:api-engineer]
- [ ] [4.3] [P-2] Implement run_poll_cycle() -- collect, store via ServerHealthStore, compare uptime to previous for crash detection [owner:api-engineer]
- [ ] [4.4] [P-2] Implement handle_crash_detected() -- create P1 obligation via ObligationStore when uptime decreases between polls [owner:api-engineer]
- [ ] [4.5] [P-3] Add 7-day prune call after each insert in run_poll_cycle() [owner:api-engineer]

## API Endpoint

- [ ] [5.1] [P-1] Add GET /api/server-health handler in dashboard.rs -- return JSON with daemon (HealthResponse), latest (ServerHealthSnapshot), status (HealthStatus), history (Vec, 24h, limit 1440) [owner:api-engineer]
- [ ] [5.2] [P-2] Wire route into build_dashboard_router() [owner:api-engineer]
- [ ] [5.3] [P-2] Add DashboardState.messages_db_path field for server health store access [owner:api-engineer]

## Verify

- [ ] [6.1] Unit tests for ServerHealthStore -- insert_and_latest, previous_returns_second_row, health_status_classification, prune_older_than_days [owner:api-engineer]
- [ ] [6.2] Unit tests for health_poller -- cpu_idle_includes_iowait, disk_bavail_calculation, collect_metrics structure [owner:api-engineer]
- [ ] [6.3] Unit test for migration -- verify PRAGMA user_version advances correctly after migration [owner:api-engineer]
- [ ] [6.4] cargo build -- full project compiles cleanly [owner:api-engineer]
- [ ] [6.5] cargo clippy -- -D warnings passes [owner:api-engineer]
- [ ] [6.6] cargo test -- all existing + new tests pass [owner:api-engineer]
