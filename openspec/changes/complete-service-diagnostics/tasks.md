# Implementation Tasks

<!-- beads:epic:nv-ekt -->

## DB Batch: Module Restructuring & Type Definitions

- [ ] [1.1] [nv-v6j] Move 16 `*_tools.rs` files into `tools/` module directories (ado, calendar, cloudflare, docker, doppler, github, ha, neon, plaid, posthog, resend, schedule, sentry, stripe, teams, upstash, vercel, web) — each becomes `tools/<name>/mod.rs` or stays flat where no sub-modules needed [owner:api-engineer]
- [ ] [1.2] [nv-5dn] Create `tools/mod.rs` — extract `register_tools()` and `execute_tool()` into `tools/dispatch.rs`; slim `mod.rs` to re-exports, `Checkable` trait, `CheckResult` enum, `ServiceRegistry<T>` [owner:api-engineer]
- [ ] [1.3] [nv-9hp] Create `channels/mod.rs` — update re-exports for channel modules (discord, email, imessage, teams, telegram, util) [owner:api-engineer]
- [ ] [1.4] [nv-lfx] Move `jira/` module into `tools/jira/` — already a directory; verify sub-module imports remain valid [owner:api-engineer]
- [ ] [1.5] [nv-axo] Update `main.rs` module declarations for `tools/` and `channels/` restructured paths [owner:api-engineer]
- [ ] [1.6] [nv-r6t] Update all `use crate::` imports for restructured modules across daemon crate [owner:api-engineer]
- [ ] [1.7] [nv-a5l] Add generic `ServiceConfig` to `nv-core/config.rs` — already exists (`ServiceConfig<T>` with Flat/Multi variants); verify it covers all service types and add any missing per-instance config structs [owner:api-engineer]
- [ ] [1.8] [nv-tqe] Define `Checkable` trait with `check_read`/`check_write` and `CheckResult` enum — already exists in `tools/mod.rs`; remove `#[allow(dead_code)]` annotations, verify trait is public and re-exported [owner:api-engineer]
- [ ] [1.9] [nv-w06] Define `ServiceRegistry<T: Checkable>` with `resolve`/`get`/`default`/`iter` — already exists in `tools/mod.rs`; remove `#[allow(dead_code)]`, verify public API surface [owner:api-engineer]
- [ ] [1.10] [nv-rrc] Create `tools/check.rs` — `CheckReport`, `check_all`, formatters (terminal, JSON, Telegram), `MissingService`, `timed()` helper — already implemented; verify re-exports after module restructure, remove `#[allow(dead_code)]` [owner:api-engineer]
- [ ] [1.11] [nv-6nd] Verify `cargo check` passes after restructure — gate before proceeding to API batch [owner:api-engineer]

## API Batch: Checkable Trait, ServiceRegistry, Tool Registration

- [ ] [2.1] [nv-stw] Implement `Checkable` for all 14 service clients that don't already have it — audit existing 17 impls (stripe, vercel, sentry, resend, ha, upstash, ado, cloudflare, doppler, neon, posthog, github, docker, plaid, teams, jira, calendar); remove `#[allow(dead_code)]` from each impl block [owner:api-engineer]
- [ ] [2.2] [nv-6ki] Convert `SharedDeps` to use `ServiceRegistry<T>` for all services — currently has registries for stripe, vercel, sentry, resend, ha, upstash, ado, cloudflare, doppler; add registries for neon, posthog, github, docker, plaid, teams, calendar [owner:api-engineer]
- [ ] [2.3] [nv-6m0] Update `execute_tool` to resolve from registries — replace direct client usage with `registry.resolve(project)` or `registry.default()` for each tool dispatch case [owner:api-engineer]
- [ ] [2.4] [nv-ur8] Extend `Secrets` for instance-qualified env var loading — wire `collect_instance_secrets(prefix)` into registry construction path in `main.rs` for services that support multi-instance [owner:api-engineer]
- [ ] [2.5] [nv-yog] Register `check_services` tool definition — already registered in `register_tools()`; update dispatch in `execute_tool()` to use registry-based `check_all()` instead of ad-hoc client construction [owner:api-engineer]

## UI Batch: nv check CLI & Health Endpoint

- [ ] [3.1] [nv-ro6] Add `nv check` clap subcommand with `--json`/`--read-only`/`--service` flags — already scaffolded in `nv-cli/src/main.rs` and implemented in `nv-cli/src/commands/check.rs`; verify alignment with registry-based probe path [owner:api-engineer]
- [ ] [3.2] [nv-1j5] Extend `HealthResponse` with `deep=true` tool probes — replace ad-hoc client construction in `to_deep_health_response()` with registry iteration from `SharedDeps`; pass registries to health state or health endpoint handler [owner:api-engineer]
- [ ] [3.3] [nv-1ef] Update `nv.toml` with multi-instance config examples — add documented examples for flat and multi-instance configurations (stripe, jira, sentry as examples), including `project_map` usage [owner:api-engineer]

## Verify Batch: Tests & Build Gate

- [ ] [4.1] [nv-5p6] Unit tests: `ServiceRegistry` — `new`, `single`, `resolve`, `get`, `default`, `iter`, `is_empty`, `len` with various instance/project_map configs; `ServiceConfig` flat/multi deserialization; `check_all` with mock services; all three formatters (terminal, JSON, Telegram) [owner:api-engineer]
- [ ] [4.2] [nv-me1] Integration test: `nv check --json` with mocked services — verify JSON output structure matches `CheckReport` schema, exit code 0 when all healthy, exit code 1 when any unhealthy [owner:api-engineer]
- [ ] [4.3] `cargo build` — full workspace build passes with no dead_code warnings on diagnostics types [owner:api-engineer]
- [ ] [4.4] `cargo clippy -- -D warnings` — no new warnings introduced [owner:api-engineer]
- [ ] [4.5] `cargo test` — all existing tests pass plus new unit and integration tests [owner:api-engineer]
