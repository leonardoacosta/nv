# Proposal: Complete Service Diagnostics

## Change ID
`complete-service-diagnostics`

## Summary

Wire up the remaining service diagnostics infrastructure: restructure flat tool/channel files into
proper module directories, integrate `ServiceRegistry<T>` into `SharedDeps` for all services,
implement the `nv check` CLI subcommand end-to-end, extend the health endpoint with `deep=true`
tool probes, and add comprehensive test coverage. Most foundational types (`Checkable` trait,
`CheckResult` enum, `ServiceRegistry<T>`, `ServiceConfig<T>`, `check_all()`, formatters) already
exist but are gated behind `#[allow(dead_code)]` and not yet called from production paths.

## Context
- Extends: `crates/nv-daemon/src/tools/` (16 flat `*_tools.rs` files + `mod.rs` with `register_tools`/`execute_tool` + `check.rs`), `crates/nv-daemon/src/channels/` (5 channel subdirs + `mod.rs`), `crates/nv-daemon/src/health.rs` (health endpoint)
- Depends on: `crates/nv-core/src/config.rs` (`ServiceConfig<T>`, `Secrets`)
- Related: `crates/nv-cli/src/commands/check.rs` (CLI `nv check` implementation, already scaffolded), `crates/nv-daemon/src/worker.rs` (`SharedDeps` struct with existing registries)
- Beads epic: `nv-ekt` (add-service-diagnostics)

## Motivation

The codebase has 17 `Checkable` trait implementations across service clients (stripe, vercel,
sentry, resend, ha, upstash, ado, cloudflare, doppler, neon, posthog, github, docker, plaid,
teams, jira, calendar), a full `check_all()` orchestrator with terminal/JSON/Telegram formatters,
and a `ServiceRegistry<T>` generic container --- all marked `#[allow(dead_code)]`. The CLI
`nv check` subcommand exists with clap flags (`--json`, `--read-only`, `--service`) and a working
implementation in `nv-cli/src/commands/check.rs`, but the module structure in `nv-daemon` is still
flat (16 `*.rs` files directly in `tools/`), `execute_tool` does not resolve clients from
registries for most services, and the health endpoint's `to_deep_health_response()` constructs
clients ad-hoc from env vars instead of using the registry pattern.

This spec completes the circuit:
1. Restructure `tools/` and `channels/` into proper module directories
2. Convert `SharedDeps` to use `ServiceRegistry<T>` for all services
3. Make `execute_tool` resolve from registries
4. Wire `nv check` end-to-end through the daemon
5. Extend `GET /health?deep=true` to use registries instead of ad-hoc client construction
6. Add unit and integration tests

## Requirements

### Req-1: Module Restructuring

Move the 16 flat `*_tools.rs` files in `tools/` into proper module directories (already has
`tools/jira/` as a precedent). Move `register_tools()` and `execute_tool()` out of `tools/mod.rs`
into a dedicated file so `mod.rs` only contains re-exports, the `Checkable` trait, `CheckResult`
enum, and `ServiceRegistry<T>`. Update `channels/mod.rs` re-exports. Update `main.rs` module
declarations and all `use crate::` import paths.

### Req-2: ServiceRegistry Integration

Convert `SharedDeps` to hold `ServiceRegistry<T>` for all 14 service clients that currently use
direct `Option<Client>` fields or ad-hoc construction. The Jira registry already uses this pattern
(`JiraRegistry`). Each registry is populated from `ServiceConfig<T>` deserialized from `nv.toml`
or from `Secrets::collect_instance_secrets()` for env-var-based configs.

### Req-3: execute_tool Registry Resolution

Update `execute_tool()` dispatch to resolve the correct client instance from the registry using
the project context from the tool call input, instead of using a single flat client.

### Req-4: nv check CLI End-to-End

The CLI scaffolding exists (`nv-cli/src/commands/check.rs`). Complete the integration:
- Respect `--service` filter (partial match on service name)
- Respect `--read-only` flag (skip write probes)
- Respect `--json` flag (JSON output vs terminal)
- Exit code 1 if any service is unhealthy

### Req-5: Health Endpoint Deep Probes

Replace the ad-hoc client construction in `HealthState::to_deep_health_response()` with registry
iteration. The daemon already holds all registries in `SharedDeps`; the health endpoint should
iterate all registries, collect `&dyn Checkable` refs, and call `check_all()`.

### Req-6: Secrets Instance-Qualified Loading

Extend `Secrets` to support instance-qualified env var loading (e.g. `STRIPE_SECRET_KEY_PERSONAL`,
`STRIPE_SECRET_KEY_LLC`) using the existing `collect_instance_secrets(prefix)` method. Wire this
into the registry construction path so multi-instance configs can source credentials from env vars
without requiring `nv.toml` entries.

### Req-7: check_services Tool Registration

Register the `check_services` tool definition in `register_tools()` (already done) and wire its
dispatch in `execute_tool()` to use the registry-based probe path instead of ad-hoc client
construction.

### Req-8: nv.toml Multi-Instance Config Examples

Update `config/nv.toml` with documented examples showing both flat (single-instance) and
multi-instance configurations for services that support it (stripe, jira, sentry, etc.).

## Scope
- **IN**: Module restructuring, ServiceRegistry wiring, execute_tool registry resolution, nv check CLI, health endpoint deep probes, instance-qualified secrets, check_services tool, nv.toml examples, unit tests, integration tests
- **OUT**: New Checkable implementations (all 17 already exist), new service integrations, UI/dashboard for health, automated health alerting, Prometheus/metrics export

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/tools/mod.rs` | Slim down to re-exports, trait, enum, registry; move `register_tools`/`execute_tool` to separate file |
| `crates/nv-daemon/src/tools/*.rs` | No content changes, only path moves into module dirs |
| `crates/nv-daemon/src/channels/mod.rs` | Update re-exports after restructure |
| `crates/nv-daemon/src/main.rs` | Update module declarations, registry construction |
| `crates/nv-daemon/src/worker.rs` | Update `SharedDeps` to use registries for all services |
| `crates/nv-daemon/src/health.rs` | Replace ad-hoc client construction with registry iteration |
| `crates/nv-cli/src/commands/check.rs` | Already implemented; may need minor updates for registry alignment |
| `crates/nv-core/src/config.rs` | Instance-qualified secret loading (method already exists) |
| `config/nv.toml` | Add multi-instance config examples |

## Risks
| Risk | Mitigation |
|------|-----------|
| Module restructuring breaks imports across crate | Batch 1 ends with `cargo check` gate; fix all `use crate::` paths before proceeding |
| `tools/mod.rs` is 138KB with register/execute mixed in | Extract to `tools/dispatch.rs`; mod.rs becomes re-exports only |
| Ad-hoc client construction in health.rs duplicates CLI check | Both paths converge to registry-based `check_all()` after this spec |
| 17 Checkable impls already exist but are dead code | Remove `#[allow(dead_code)]` as each gets wired into live paths |
| Env var naming for multi-instance may conflict | Follow existing `collect_instance_secrets` convention: `PREFIX_SUFFIX` |
