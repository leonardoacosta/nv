# Implementation Tasks

<!-- beads:epic:TBD -->

## API Batch

- [x] [api-engineer] Compute health status from channel map in to_health_response(): return "degraded" if any ChannelStatus::Disconnected, "ok" otherwise — `crates/nv-daemon/src/health.rs`
- [x] [api-engineer] Implement Commands::Config stub: call Config::load() and print as serde_json::to_string_pretty — `crates/nv-cli/src/main.rs`
- [x] [api-engineer] Implement Commands::Digest { now: false } stub: fetch /health and print last_digest_at field — `crates/nv-cli/src/main.rs`
- [x] [api-engineer] Add TeamsCheck to to_deep_health_response() owned vec via push_env! macro, matching CLI check_services() inventory — `crates/nv-daemon/src/health.rs`
- [x] [api-engineer] Replace hardcoded /home/nyaptor in health_poller.rs:176 with std::env::var("HOME") when building the Nexus session project path — `crates/nv-daemon/src/health_poller.rs`
- [x] [api-engineer] Replace hardcoded /home/nyaptor fallback in claude.rs:255 SpawnConfig::new() with an Err or env-derived value — `crates/nv-daemon/src/claude.rs`
- [x] [api-engineer] Replace hardcoded /home/nyaptor fallback in callbacks.rs:132 with std::env::var("HOME").unwrap_or_default() — `crates/nv-daemon/src/callbacks.rs`
- [x] [api-engineer] Validate quiet_start and quiet_end in Config::load() after deserialisation — parse HH:MM, return Err on invalid format — `crates/nv-core/src/config.rs`
- [x] [api-engineer] Add file lock (fs2::FileExt or flock) around read-modify-write cycle in state.rs pending-actions handling — `crates/nv-daemon/src/state.rs`
- [x] [api-engineer] Emit sd_notify WATCHDOG=1 once per tick in the main daemon poll loop using the sd-notify crate — `crates/nv-daemon/src/health_poller.rs` (or daemon main loop)
- [x] [api-engineer] Change WatchdogSec from 60 to 120 in nv.service to allow one missed tick before kill — `deploy/nv.service`
- [x] [api-engineer] Wrap systemctl enable nv-teams-relay in deploy/install.sh behind a TEAMS_WEBHOOK_SECRET presence check — `deploy/install.sh`
- [x] [api-engineer] Fix CPU idle calculation in read_cpu_jiffies() to include iowait: idle = fields[3] + fields.get(4).copied().unwrap_or(0) — `crates/nv-daemon/src/health_poller.rs`
- [x] [api-engineer] Fix disk free calculation in read_disk_usage() to use f_bavail instead of f_bfree — `crates/nv-daemon/src/health_poller.rs`
- [x] [api-engineer] Remove ServiceInstanceConfig empty marker struct and simplify ServiceConfig<T> generic — `crates/nv-core/src/config.rs` or relevant service_config file

## Verify

- [x] [api-engineer] cargo build passes
- [x] [api-engineer] cargo clippy -- -D warnings passes
- [x] [api-engineer] Unit test: to_health_response() returns "degraded" when any channel is Disconnected — `crates/nv-daemon/src/health.rs`
- [x] [api-engineer] Unit test: to_health_response() returns "ok" when all channels Connected — `crates/nv-daemon/src/health.rs`
- [x] [api-engineer] Unit test: Config::load() returns Err for quiet_start = "25:99" — `crates/nv-core/src/config.rs`
- [x] [api-engineer] Unit test: Config::load() returns Err for quiet_end = "not-a-time" — `crates/nv-core/src/config.rs`
- [x] [api-engineer] Unit test: Config::load() accepts valid "23:00" and "07:00" quiet hours — `crates/nv-core/src/config.rs`
- [x] [api-engineer] Unit test: read_cpu_jiffies() idle includes iowait field — `crates/nv-daemon/src/health_poller.rs`
- [x] [api-engineer] Unit test: read_disk_usage() uses f_bavail (spot-check value matches df output) — `crates/nv-daemon/src/health_poller.rs`
- [x] [api-engineer] Existing tests pass
