# Proposal: Service Diagnostics & Module Restructure

## Change ID
`add-service-diagnostics`

## Summary

Restructure the daemon's flat tool/channel files into `tools/` and `channels/` modules, introduce
a `Checkable` trait for unified auth/connectivity validation, expand multi-instance support to all
services, and add an `nv check` CLI subcommand plus a `check_services` Nova tool for real-time
service health diagnostics.

## Context
- Extends: `crates/nv-daemon/src/main.rs` (module declarations, client init), `crates/nv-daemon/src/tools.rs` (register_tools, execute_tool), `crates/nv-daemon/src/worker.rs` (SharedDeps), `crates/nv-daemon/src/health.rs` (HealthState/HealthResponse), `crates/nv-core/src/config.rs` (service configs, Secrets)
- Related: `openspec/changes/add-multi-instance-services/` (Jira multi-instance pattern — this spec extends it to all services), `crates/nv-daemon/src/jira/registry.rs` (existing JiraRegistry pattern to generalize)
- Depends on: `add-multi-instance-services` (Jira pattern must land first or be absorbed)

## Motivation

Nova integrates 18 services across 6 auth patterns (Bearer, Basic, OAuth, connection string, CLI,
webhook). Today there is no way to validate whether credentials are correct, expired, or missing
without triggering an actual tool call and watching it fail. Services are sprawled as 12 `*_tools.rs`
files in the daemon src root alongside core files like `agent.rs` and `orchestrator.rs`, making the
codebase hard to navigate.

Additionally, Leo manages multiple organizations (personal + LLC) with separate credentials for
Stripe, Sentry, PostHog, etc. Only Jira currently supports multi-instance config. Extending the
registry pattern to all services eliminates credential juggling.

This spec solves three problems in one pass:
1. **Navigability** — `tools/` and `channels/` modules with clear boundaries
2. **Diagnosability** — `nv check` validates all credentials in seconds
3. **Multi-org support** — any service can have named instances with project routing

## Requirements

### Req-1: Module Restructure

Move all `*_tools.rs` files into a `tools/` module directory and formalize `channels/` for all
messaging channel modules. Update `main.rs` module declarations and all internal imports.

### Req-2: Checkable Trait

Define a `Checkable` trait that each service implements:

```rust
#[async_trait]
pub trait Checkable: Send + Sync {
    /// Human-readable service name (e.g., "stripe", "jira/personal")
    fn name(&self) -> &str;

    /// Check read connectivity — lightweight GET or equivalent
    async fn check_read(&self) -> CheckResult;

    /// Check write permissions — dry-run probe (expect 400, not 2xx)
    /// Returns None if service has no write endpoints
    async fn check_write(&self) -> Option<CheckResult> {
        None // default: no write check
    }
}
```

`CheckResult` is an enum: `Healthy { latency_ms: u64, detail: String }`, `Degraded { message }`,
`Unhealthy { error }`, `Missing { env_var: String }`.

### Req-3: Multi-Instance Expansion

Generalize the JiraRegistry pattern into a `ServiceRegistry<T: Checkable>` that all services use.
Each service config supports both flat (single instance) and named-instance TOML:

```toml
# Flat — backward compatible
[stripe]
# uses STRIPE_SECRET_KEY env var

# Multi-instance
[stripe.instances.personal]
# uses STRIPE_SECRET_KEY_PERSONAL env var

[stripe.instances.llc]
# uses STRIPE_SECRET_KEY_LLC env var

[stripe.project_map]
OO = "personal"
CT = "llc"
```

Services that don't need multi-instance (Docker, GitHub CLI) keep flat config only.

### Req-4: `nv check` CLI Subcommand

A clap subcommand that instantiates all configured services and runs their `Checkable` probes:

```
$ nv check

 Channels
  ✓ telegram       Bot @nova_bot connected              12ms
  ✓ discord        Gateway reachable, 2 servers          45ms
  ✗ teams          OAuth token expired                   --
  ○ imessage       Disabled in config

 Tools (read)
  ✓ jira/personal  leonardoacosta.atlassian.net          89ms
  ✓ jira/llc       civalent.atlassian.net                102ms
  ✓ stripe/personal sk_live_...abc                       67ms
  ✗ stripe/llc     STRIPE_SECRET_KEY_LLC missing          --
  ✓ sentry         org: leo                              54ms
  ...

 Tools (write)
  ✓ jira/personal  create_issue dry-run: 400 valid       91ms
  ✓ stripe/personal invoice dry-run: 400 valid           72ms
  ✓ ha             service_call dry-run: 400 valid       8ms
  ...

 Summary: 16/20 healthy, 2 missing, 1 expired, 1 disabled
```

Flags: `--json` for machine-readable output, `--read-only` to skip write probes, `--service <name>`
to check a single service.

### Req-5: `check_services` Nova Tool

Register a `check_services` tool definition so Nova can self-diagnose when a tool call fails.
Returns the same structured output as `nv check --json`. Nova can use this proactively or when
a tool returns an auth error.

## Scope
- **IN**: Module restructure (tools/, channels/), Checkable trait with read+write probes, generic ServiceRegistry replacing per-service client fields, multi-instance config for all services, `nv check` CLI subcommand, `check_services` Nova tool, HealthResponse extended with tool status, unit tests for Checkable implementations
- **OUT**: OAuth token refresh automation (Teams/Email — separate concern), runtime instance hot-swap without restart, per-instance webhook secrets (defer to existing spec), GUI/TUI for instance management, actual resource creation in write checks (dry-run only)

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/` | Move 12 `*_tools.rs` → `tools/`, 5 channel dirs → `channels/` |
| `crates/nv-daemon/src/tools/mod.rs` | New module root with `Checkable` trait, `ServiceRegistry<T>`, `register_tools()` |
| `crates/nv-daemon/src/tools/check.rs` | `CheckResult` enum, `check_all()` orchestrator |
| `crates/nv-daemon/src/channels/mod.rs` | New module root re-exporting channel types |
| `crates/nv-core/src/config.rs` | Generic `InstanceConfig` pattern for all services, `ServiceConfig` enum (flat vs instances) |
| `crates/nv-daemon/src/worker.rs` | `SharedDeps` fields change from `Option<XClient>` to `Option<ServiceRegistry<XClient>>` |
| `crates/nv-daemon/src/main.rs` | Module declarations updated, registry construction for all services |
| `crates/nv-daemon/src/health.rs` | `HealthResponse` gains `tools: HashMap<String, CheckResult>` |
| `config/nv.toml` | Multi-instance examples for all services |

## Risks
| Risk | Mitigation |
|------|-----------|
| Massive file-move diff obscures logic changes | Restructure batch is pure moves with zero logic changes; logic changes in separate batch |
| Generic `ServiceRegistry<T>` over-abstraction | Keep it simple — HashMap + resolve() method, no trait hierarchies |
| Dry-run write probes may not work for all APIs | Each service's `check_write()` is optional; services where dry-run isn't feasible return None |
| Multi-instance config breaks existing nv.toml | Backward-compatible flat config always works; multi-instance is additive |
| Large number of env vars for multi-instance | Clear naming convention `SERVICE_VAR_INSTANCENAME`, documented in config comments |
