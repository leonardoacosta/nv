# Proposal: Fix Infrastructure Health & Polish

## Change ID
`fix-infra-health`

## Summary

Bundle eleven audit findings into one spec: surface degraded channel status in health responses,
add TeamsCheck to the deep health endpoint, remove three hardcoded `/home/nyaptor` paths, implement
two CLI stubs, validate quiet-hours config at parse time, fix the pending-actions race condition,
verify systemd watchdog notification, guard Teams relay behind env presence, and correct CPU iowait
and disk reserved-block accounting.

## Context
- Extends: `crates/nv-daemon/src/health.rs` (to_health_response, to_deep_health_response)
- Extends: `crates/nv-daemon/src/health_poller.rs` (CPU jiffies, disk statvfs, hardcoded path)
- Extends: `crates/nv-cli/src/main.rs` (Config and Digest command stubs)
- Extends: `crates/nv-core/src/config.rs` (quiet_start / quiet_end fields)
- Extends: `crates/nv-daemon/src/state.rs` (pending-actions read-modify-write)
- Extends: `crates/nv-daemon/src/claude.rs` (hardcoded fallback path)
- Extends: `crates/nv-daemon/src/callbacks.rs` (hardcoded fallback path)
- Extends: `deploy/nv.service` (WatchdogSec)
- Extends: `deploy/install.sh` (Teams relay unconditional enable)
- Source: Audit 2026-03-23 (infra domain, ~76/C+ health)

## Motivation

The audit identified eleven issues across two priority levels that undermine operational
correctness and portability:

**P2 — Correctness gaps visible to callers:**

1. **Health status always "ok"** — `to_health_response()` hardcodes `status: "ok"` regardless of
   channel state. A `Disconnected` channel does not degrade the top-level status. Operators and
   automated monitors polling `/health` cannot distinguish a degraded system from a healthy one.

2. **CLI stubs in production** — `nv config` and `nv digest` (without `--now`) print
   "not implemented yet". These are dev artifacts shipped in the released CLI binary.

3. **TeamsCheck absent from /health?deep=true** — `check_services()` in the CLI includes
   `TeamsCheck`, but `to_deep_health_response()` in the daemon does not. The health endpoint and
   `nv check` show different service inventories to operators.

4. **Three hardcoded `/home/nyaptor` paths** — `health_poller.rs:176` (Nexus session path),
   `claude.rs:255` (HOME fallback), and `callbacks.rs:132` (project path fallback) embed the
   developer's username. The daemon fails silently or uses wrong paths on any other machine.

**P3 — Correctness gaps with lower blast radius:**

5. **quiet_start / quiet_end not validated at parse** — any string is accepted; invalid values
   like `"25:99"` only fail at the use site deep in the notification path.

6. **pending-actions.json race** — `state.rs` performs an unchecked read-modify-write on the
   JSON file. Two concurrent workers can produce a lost update.

7. **Watchdog signal not verified** — `deploy/nv.service` sets `WatchdogSec=60` but it is
   unclear whether the daemon emits `sd_notify(WATCHDOG=1)`. If not, systemd kills the daemon
   every 60 seconds.

8. **Teams relay unconditionally enabled** — `deploy/install.sh` enables the Teams relay service
   even when `TEAMS_WEBHOOK_SECRET` is absent, causing a startup loop-fail.

9. **CPU busy% understates iowait** — `read_cpu_jiffies()` uses only `fields[3]` (idle) as the
   idle counter, excluding iowait (`fields[4]`). Reported CPU busy% is lower than actual.

10. **Disk used% understates reserved blocks** — `read_disk_usage()` uses `f_bfree` (blocks free
    for root) instead of `f_bavail` (blocks available to unprivileged processes). Used% is
    understated by the root-reserved fraction (typically 5%).

11. **ServiceInstanceConfig empty marker struct** — dead abstraction carried through the
    `ServiceConfig<T>` generic with no fields or behaviour.

## Requirements

### Req-1: Health Status Degradation

`to_health_response()` must compute the top-level `status` field from the channel map. If any
channel has `ChannelStatus::Disconnected`, return `"degraded"`. If all channels are
`Connected` (or the map is empty), return `"ok"`. The same logic must apply inside
`to_deep_health_response()` (currently it delegates to `to_health_response()` then patches
`tools`, so fixing the base method is sufficient).

### Req-2: Remove CLI Stubs

Implement the two stub arms:

- `Commands::Config` — print the current resolved config as formatted JSON (call
  `nv_core::Config::load()` and `serde_json::to_string_pretty`).
- `Commands::Digest { now: false }` — fetch and print the last digest timestamp from the
  daemon's `/health` endpoint (already exposed as `last_digest_at` in `HealthResponse`).

### Req-3: Add TeamsCheck to Deep Health

`to_deep_health_response()` must include a Teams connectivity probe alongside the existing
service list. Construct `TeamsCheck` from env the same way the CLI's `check_services()` does,
and add it to the `owned` vec via the existing `push_env!` macro pattern.

### Req-4: Replace Hardcoded Paths

Replace all three hardcoded `/home/nyaptor` literals:

- `health_poller.rs:176` — use `std::env::var("HOME").unwrap_or_else(|_| "/home/nyaptor".into())`
  and build the project path from that.
- `claude.rs:255` — the fallback is already reading `REAL_HOME` then `HOME`; remove the
  hardcoded final fallback string, replacing it with an empty string or a documented panic with
  a clear error message.
- `callbacks.rs:132` — use `std::env::var("HOME").unwrap_or_default()` when constructing the
  fallback project path, not a literal.

### Req-5: Validate Quiet Hours at Parse Time

In `Config::load()`, after deserialisation, validate `quiet_start` and `quiet_end` when present.
Parse each with a `HH:MM` regex or `chrono::NaiveTime::parse_from_str`. Return a descriptive
`Err` if the format is invalid. Do not silently accept "25:99".

### Req-6: Fix Pending-Actions Race

Replace the unchecked read-modify-write in `state.rs` with a file-lock guard (use `fs2::FileExt`
or an equivalent `flock`-based approach). Hold the lock for the duration of the read, mutate,
and write cycle. Document that single-user daemon semantics make this low-probability but the
fix eliminates the class of bug entirely.

### Req-7: Verify and Emit Watchdog Notification

Add `sd_notify(0, "WATCHDOG=1\0")` via the `libsystemd` crate (or direct `libc::sd_notify` FFI)
to the main daemon poll loop, emitted once per tick. Confirm `deploy/nv.service` `WatchdogSec`
value is appropriate (60s is aggressive for a 60s poll loop — change to `WatchdogSec=120` to
allow one missed tick before kill).

### Req-8: Guard Teams Relay on Env Presence

In `deploy/install.sh`, wrap the `systemctl enable nv-teams-relay` call in a conditional that
checks whether `TEAMS_WEBHOOK_SECRET` is set (or a config key is present). If absent, skip
enablement and print an informational message.

### Req-9: Fix CPU iowait Accounting

In `read_cpu_jiffies()`, change the idle calculation to include iowait:

```rust
// Before
let idle = fields[3];

// After
let idle = fields[3] + fields.get(4).copied().unwrap_or(0); // idle + iowait
```

This matches the definition used by `top`, `mpstat`, and the Linux kernel documentation.

### Req-10: Fix Disk Reserved-Block Accounting

In `read_disk_usage()`, replace `f_bfree` with `f_bavail`:

```rust
// Before
let free_bytes = stat.f_bfree as u64 * block_size;

// After
let free_bytes = stat.f_bavail as u64 * block_size;
```

`f_bavail` is the correct field for "space available to non-root processes" and matches
the value reported by `df`.

### Req-11: Remove ServiceInstanceConfig

Delete the `ServiceInstanceConfig` empty marker struct and simplify or remove the
`ServiceConfig<T>` generic where `T = ServiceInstanceConfig` is the only instantiation. If the
generic serves no other purpose, collapse it to a concrete struct.

## Scope
- **IN**: health status degradation, CLI stubs, TeamsCheck in deep health, hardcoded paths,
  quiet-hours validation, pending-actions lock, watchdog notify, Teams relay guard, CPU iowait,
  disk f_bavail, ServiceInstanceConfig removal
- **OUT**: New health check services, new CLI subcommands beyond the two stubs, streaming health
  endpoints, UI changes

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/health.rs` | Compute degraded status from channel map |
| `crates/nv-cli/src/main.rs` | Implement Config and Digest stubs |
| `crates/nv-daemon/src/health.rs` | Add TeamsCheck to to_deep_health_response |
| `crates/nv-daemon/src/health_poller.rs` | Replace hardcoded path; fix CPU iowait; fix disk f_bavail |
| `crates/nv-daemon/src/claude.rs` | Replace hardcoded HOME fallback |
| `crates/nv-daemon/src/callbacks.rs` | Replace hardcoded path fallback |
| `crates/nv-core/src/config.rs` | Validate quiet_start/quiet_end at parse time |
| `crates/nv-daemon/src/state.rs` | Add file lock around read-modify-write |
| `deploy/nv.service` | Change WatchdogSec=120; confirm watchdog notify |
| `deploy/install.sh` | Guard Teams relay enable behind env check |
| `crates/nv-core/src/config.rs` (or `service_config.rs`) | Remove ServiceInstanceConfig |

## Risks
| Risk | Mitigation |
|------|-----------|
| Health status change breaks callers that check `status == "ok"` | Change is strictly additive — "ok" still returned when no channels disconnected; "degraded" is a new value callers should handle |
| File lock in state.rs adds latency on contended access | Lock is held for a single JSON read+write (microseconds); single-user daemon makes contention essentially impossible |
| sd_notify FFI call without libsystemd linked fails silently | Use `sd_notify` crate which handles the non-systemd case gracefully (no-op) |
| WatchdogSec=120 means a deadlocked daemon lives 120s before kill | Acceptable; 60s was too tight for a 60s tick loop and caused false kills |
