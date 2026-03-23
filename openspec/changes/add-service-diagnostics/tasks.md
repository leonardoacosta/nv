# Implementation Tasks

<!-- beads:epic:nv-ekt -->

## Restructure Batch

- [x] [1.1] [P-1] Create `tools/mod.rs` — move `register_tools()` and `execute_tool()` from `tools.rs`, update module declarations [owner:general-purpose]
- [x] [1.2] [P-1] Move 16 `*_tools.rs` files into `tools/` (stripe, vercel, sentry, neon, posthog, upstash, resend, ado, ha, docker, plaid, github, web, cloudflare, doppler, calendar, schedule) — rename dropping `_tools` suffix [owner:general-purpose]
- [x] [1.3] [P-1] Move `jira/` module into `tools/jira/` [owner:general-purpose]
- [x] [1.4] [P-1] Create `channels/mod.rs` — re-export telegram, discord, teams, email, imessage modules [owner:general-purpose]
- [x] [1.5] [P-1] Update `main.rs` module declarations — replace 20+ flat `mod` statements with `mod tools; mod channels;` [owner:general-purpose]
- [x] [1.6] [P-2] Update all `use crate::xxx_tools` imports across agent.rs, worker.rs, callbacks.rs, aggregation.rs, orchestrator.rs [owner:general-purpose]
- [x] [1.7] [P-2] Verify `cargo check` passes with zero logic changes [owner:general-purpose]

## Core Batch

- [ ] [2.1] [P-1] Define `Checkable` trait in `tools/mod.rs` — `name()`, `check_read()`, `check_write()` with `CheckResult` enum [owner:general-purpose]
- [ ] [2.2] [P-1] Define `ServiceRegistry<T: Checkable>` in `tools/mod.rs` — `resolve()`, `get()`, `default()`, `iter()` [owner:general-purpose]
- [ ] [2.3] [P-1] Create `tools/check.rs` — `CheckReport` struct, `check_all()` orchestrator using FuturesUnordered, terminal formatter, JSON formatter [owner:general-purpose]
- [ ] [2.4] [P-1] Add generic `ServiceConfig` enum to `nv-core/config.rs` — flat vs instances deserialization with `#[serde(untagged)]` [owner:general-purpose]
- [ ] [2.5] [P-2] Extend `Secrets` in `nv-core/config.rs` — instance-qualified env var loading for all services (`SERVICE_VAR_INSTANCENAME` pattern) [owner:general-purpose]

## Implementation Batch

- [ ] [3.1] [P-1] Implement `Checkable` for StripeClient — read: `GET /v1/balance`, write: `POST /v1/invoices` empty body [owner:general-purpose]
- [ ] [3.2] [P-1] Implement `Checkable` for VercelClient — read: `GET /v13/user`, write: returns None [owner:general-purpose]
- [ ] [3.3] [P-1] Implement `Checkable` for SentryClient — read: `GET /api/0/` org info, write: returns None [owner:general-purpose]
- [ ] [3.4] [P-1] Implement `Checkable` for NeonClient — read: `SELECT 1`, write: returns None [owner:general-purpose]
- [ ] [3.5] [P-1] Implement `Checkable` for PosthogClient — read: `GET /api/projects/`, write: returns None [owner:general-purpose]
- [ ] [3.6] [P-1] Implement `Checkable` for UpstashClient — read: `INFO` command, write: returns None [owner:general-purpose]
- [ ] [3.7] [P-1] Implement `Checkable` for ResendClient — read: `GET /domains`, write: returns None [owner:general-purpose]
- [ ] [3.8] [P-1] Implement `Checkable` for AdoClient — read: `GET /_apis/projects`, write: returns None [owner:general-purpose]
- [ ] [3.9] [P-1] Implement `Checkable` for HaClient — read: `GET /api/`, write: `POST /api/services/light/turn_on` empty [owner:general-purpose]
- [ ] [3.10] [P-1] Implement `Checkable` for DockerClient — read: `docker info`, write: returns None [owner:general-purpose]
- [ ] [3.11] [P-1] Implement `Checkable` for PlaidClient — read: `SELECT 1`, write: returns None [owner:general-purpose]
- [ ] [3.12] [P-1] Implement `Checkable` for GithubClient — read: `gh auth status`, write: returns None [owner:general-purpose]
- [ ] [3.13] [P-1] Implement `Checkable` for CloudflareClient — read: `GET /user/tokens/verify`, write: returns None [owner:general-purpose]
- [ ] [3.14] [P-1] Implement `Checkable` for DopplerClient — read: `GET /v3/me`, write: returns None [owner:general-purpose]
- [ ] [3.15] [P-2] Convert all `Option<XClient>` in SharedDeps to `Option<ServiceRegistry<XClient>>` [owner:general-purpose]
- [ ] [3.16] [P-2] Update `main.rs` — build `ServiceRegistry` for each service from config + env vars [owner:general-purpose]
- [ ] [3.17] [P-2] Update `execute_tool()` — resolve client from registry by project param where applicable [owner:general-purpose]
- [ ] [3.18] [P-2] Update multi-instance config in `config/nv.toml` with commented examples for all services [owner:general-purpose]

## CLI Batch

- [ ] [4.1] [P-1] Add `nv check` clap subcommand — parse args (--json, --read-only, --service) [owner:general-purpose]
- [ ] [4.2] [P-1] Wire `nv check` to `check_all()` — load config, build registries, run probes, format output [owner:general-purpose]
- [ ] [4.3] [P-1] Register `check_services` tool definition in `register_tools()` [owner:general-purpose]
- [ ] [4.4] [P-1] Implement `check_services` tool handler in `execute_tool()` — delegates to `check_all()`, returns JSON [owner:general-purpose]
- [ ] [4.5] [P-2] Extend `HealthResponse` with optional `tools: HashMap<String, CheckResult>` for `?deep=true` [owner:general-purpose]
- [ ] [4.6] [P-2] Update `/health` HTTP handler to run probes when `deep=true` query param present [owner:general-purpose]

## Test Batch

- [ ] [5.1] Unit tests for `ServiceRegistry` — resolve, get, default, iter, empty registry [owner:general-purpose]
- [ ] [5.2] Unit tests for `ServiceConfig` deserialization — flat, multi-instance, backward compat [owner:general-purpose]
- [ ] [5.3] Unit tests for `check_all` — mock Checkable impls, verify concurrent execution and report format [owner:general-purpose]
- [ ] [5.4] Unit tests for terminal and JSON formatters [owner:general-purpose]
- [ ] [5.5] Integration test: `nv check --json` with mocked services returns valid JSON [owner:general-purpose]
