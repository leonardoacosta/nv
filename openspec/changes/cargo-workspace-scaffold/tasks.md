# Tasks: cargo-workspace-scaffold

## Dependencies

None

## Tasks

### Scaffold

- [ ] Create workspace `Cargo.toml` with `workspace.members` and `workspace.dependencies` (tokio, serde, serde_json, anyhow, thiserror, tracing, tracing-subscriber, chrono, reqwest, clap, toml, uuid, async-trait)
- [ ] Create `crates/nv-core/Cargo.toml` referencing workspace dependencies (serde, serde_json, anyhow, thiserror, chrono, uuid, async-trait, toml, tracing)
- [ ] Create `crates/nv-core/src/lib.rs` with module declarations (`pub mod config; pub mod types; pub mod channel;`)
- [ ] Create `crates/nv-core/src/config.rs` — empty module placeholder
- [ ] Create `crates/nv-core/src/types.rs` — empty module placeholder
- [ ] Create `crates/nv-core/src/channel.rs` — empty module placeholder
- [ ] Create `crates/nv-daemon/Cargo.toml` referencing nv-core (path) + workspace deps (tokio, tracing, tracing-subscriber, anyhow)
- [ ] Create `crates/nv-daemon/src/main.rs` — `#[tokio::main]`, tracing init with `EnvFilter`, "NV daemon starting" log, `ctrl_c()` shutdown
- [ ] Create `crates/nv-cli/Cargo.toml` referencing nv-core (path) + workspace deps (clap)
- [ ] Create `crates/nv-cli/src/main.rs` — clap derive with `Status`, `Ask { query }`, `Config`, `Digest { now }` subcommands, each printing placeholder
- [ ] Create `config/nv.example.toml` with all config sections (agent, telegram, jira, nexus, daemon) and env var comments for secrets
- [ ] Create `deploy/nv.service` systemd unit file (Type=simple, Restart=on-failure, RestartSec=5s, WatchdogSec=60)

### Verify

- [ ] `cargo build` passes for all workspace members
- [ ] `cargo clippy` passes with no warnings
- [ ] `./target/debug/nv-cli --help` prints subcommand help text
- [ ] `./target/debug/nv-daemon` starts, logs "NV daemon starting", shuts down on Ctrl+C
