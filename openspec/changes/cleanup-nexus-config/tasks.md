# Implementation Tasks

<!-- beads:epic:TBD -->

## Batch 1: nv-core Config Types

- [ ] [1.1] [P-1] Delete `NexusAgent` struct from `crates/nv-core/src/config.rs` (the `#[derive(Debug, Clone, Deserialize)] pub struct NexusAgent { ... }` block) [owner:api-engineer]
- [ ] [1.2] [P-1] Delete `NexusConfig` struct from `crates/nv-core/src/config.rs` (the `#[derive(Debug, Clone, Deserialize)] pub struct NexusConfig { ... }` block including the `watchdog_interval_secs` field) [owner:api-engineer]
- [ ] [1.3] [P-1] Delete `default_watchdog_interval` default-value function from `crates/nv-core/src/config.rs` — it is only used by the now-deleted `NexusConfig` [owner:api-engineer]
- [ ] [1.4] [P-1] Remove `pub nexus: Option<NexusConfig>` field from the `Config` struct in `crates/nv-core/src/config.rs` [owner:api-engineer]

## Batch 2: Config Tests

- [ ] [2.1] [P-1] Remove the `[nexus]` table and `[[nexus.agents]]` entry from the TOML fixture string in the `parse_full_config` test in `crates/nv-core/src/config.rs` [owner:api-engineer]
- [ ] [2.2] [P-1] Remove the `config.nexus.unwrap()` assertion block (asserting `agents.len()`, `agents[0].name`, `agents[0].port`) from `parse_full_config` [owner:api-engineer]
- [ ] [2.3] [P-2] Verify `parse_minimal_config` no longer asserts `config.nexus.is_none()` — remove that assertion line since the field no longer exists [owner:api-engineer]

## Batch 3: nv.toml

- [ ] [3.1] [P-1] Remove the `[nexus]` section and both `[[nexus.agents]]` entries (homelab and macbook) from `config/nv.toml` [owner:api-engineer]

## Batch 4: Config Docs

- [ ] [4.1] [P-2] In `config/system-prompt.md`, remove `query_nexus` from the "Reads (immediate)" custom tools list [owner:api-engineer]
- [ ] [4.2] [P-2] In `config/system-prompt.md`, remove "Nexus events" from the triggers list in the Context section [owner:api-engineer]
- [ ] [4.3] [P-2] In `config/system-prompt.md`, remove the `[Nexus: homelab]` example from Response Rule 2 (Cite sources) [owner:api-engineer]
- [ ] [4.4] [P-2] In `config/system-prompt.md`, remove "Nexus: offline" and "If Nexus is offline" references from Response Rules 3 and 4 [owner:api-engineer]
- [ ] [4.5] [P-2] In `config/bootstrap.md`, remove `query_nexus` from the prohibited tools list (line referencing "No jira_search, no query_nexus, no read_memory") [owner:api-engineer]

## Verify

- [ ] [5.1] `cargo build` passes with no errors or dead-code warnings related to nexus [owner:api-engineer]
- [ ] [5.2] `cargo clippy -- -D warnings` passes [owner:api-engineer]
- [ ] [5.3] `cargo test -p nv-core` passes — all config tests green [owner:api-engineer]
- [ ] [5.4] `grep -ri nexus crates/nv-core/src/config.rs` returns no matches [owner:api-engineer]
- [ ] [5.5] `grep -i '\[nexus\]' config/nv.toml` returns no matches [owner:api-engineer]
- [ ] [5.6] [user] Restart daemon with updated `nv.toml` — confirm clean startup in systemd journal with no unknown-field warnings [owner:api-engineer]
