# Proposal: Multi-Instance Service Configuration

## Change ID
`add-multi-instance-services`

## Summary

Generic multi-instance configuration pattern that allows a single service (e.g. Jira) to have
multiple named instances, each with separate credentials and base URLs. Projects map to instances
via a `project_map`, so tools automatically route to the correct backend based on the project
code in the request. Implemented for Jira now; designed so Stripe, Sentry, Vercel, etc. can
adopt the same pattern later.

## Context
- Extends: `crates/nv-core/src/config.rs` (JiraConfig), `crates/nv-daemon/src/jira/client.rs` (JiraClient), `crates/nv-daemon/src/jira/tools.rs` (tool definitions + dispatch), `crates/nv-daemon/src/main.rs` (client init), `crates/nv-daemon/src/tools.rs` (tool execution), `crates/nv-daemon/src/worker.rs` (SharedDeps)
- Related: `config/nv.toml` (current single `[jira]` section), `crates/nv-daemon/src/aggregation.rs` (project_health uses jira_client), `crates/nv-daemon/src/callbacks.rs` (pending action approval uses jira_client)

## Motivation

Leo manages multiple organizations: personal projects (OO, TC) on `leonardoacosta.atlassian.net`
and LLC projects (CT) on `civalent.atlassian.net`. Today, `[jira]` in `nv.toml` only supports one
instance URL and one set of credentials. When Nova creates an issue for CT, it hits the wrong
Jira instance.

Multi-instance config solves this by:
1. **Named instances** per service with separate URLs and credentials
2. **Project-to-instance mapping** so tools auto-route based on the project code
3. **Backward compatibility** so existing single-instance configs keep working unchanged
4. **Extensible pattern** other services can adopt without reimplementation

## Requirements

### Req-1: Multi-Instance Config Schema

Support named instances under each service section. New TOML structure for Jira:

```toml
[jira.instances.personal]
instance = "leonardoacosta.atlassian.net"
default_project = "OO"

[jira.instances.llc]
instance = "civalent.atlassian.net"
default_project = "CT"

[jira.project_map]
OO = "personal"
TC = "personal"
CT = "llc"
```

The `JiraConfig` struct must support both the new multi-instance format and the existing flat
format for backward compatibility.

### Req-2: Backward-Compatible Flat Config

A single `[jira]` section without `.instances` continues to work:

```toml
[jira]
instance = "leonardoacosta.atlassian.net"
default_project = "OO"
```

When parsed, this is treated as a single unnamed "default" instance. No `project_map` is needed;
all projects route to this instance.

### Req-3: Instance-Qualified Credential Routing

Environment variables keyed by uppercase instance name:

| Instance name | API token env var | Username env var |
|---------------|-------------------|------------------|
| `personal` | `JIRA_API_TOKEN_PERSONAL` | `JIRA_USERNAME_PERSONAL` |
| `llc` | `JIRA_API_TOKEN_LLC` | `JIRA_USERNAME_LLC` |
| (default/flat) | `JIRA_API_TOKEN` | `JIRA_USERNAME` |

Fallback: if instance-qualified vars are missing, fall back to unqualified `JIRA_API_TOKEN` /
`JIRA_USERNAME`. This means upgrading from flat to multi-instance config does not break if you
only have one set of credentials.

### Req-4: Multi-Client Registry

Replace the single `Option<JiraClient>` with a `JiraRegistry` that holds a
`HashMap<String, JiraClient>` (instance name to client). For backward-compatible flat config,
the map has one entry keyed `"default"`.

### Req-5: Tool Routing by Project

Jira tools that accept a `project` parameter (e.g. `jira_create`, `jira_search`) resolve the
instance via:
1. Look up `project` in `project_map` to get instance name
2. If no mapping, use the instance whose `default_project` matches
3. If still no match, use `"default"` instance (or first instance if only one exists)

Tools that accept `issue_key` (e.g. `jira_get`, `jira_transition`, `jira_assign`, `jira_comment`)
extract the project prefix from the key (e.g. `OO-123` -> `OO`) and apply the same resolution.

### Req-6: Extensible Design

The config pattern (`instances` map + `project_map` + credential routing) should be documented
as a reusable convention. The `JiraRegistry` pattern (HashMap of clients resolved by project)
should be extractable for other services. No trait abstraction required yet -- just a consistent
naming convention and documented pattern that Stripe, Sentry, etc. can follow.

## Scope
- **IN**: Multi-instance JiraConfig parsing, backward-compatible flat config, instance-qualified env var loading, JiraRegistry with HashMap<String, JiraClient>, project-to-instance routing in all Jira tools, update all call sites (tools.rs, worker.rs, agent.rs, callbacks.rs, aggregation.rs, main.rs), config tests, documentation in nv.toml comments
- **OUT**: Multi-instance implementation for Stripe/Sentry/Vercel/etc. (pattern only), per-instance webhook secrets (use single webhook_secret for now), runtime instance switching via Telegram command, GUI/TUI for instance management

## Impact
| Area | Change |
|------|--------|
| `nv-core/src/config.rs` | Refactor `JiraConfig` to support both flat and multi-instance; add `JiraInstanceConfig`, `project_map` |
| `nv-core/src/config.rs` | Extend `Secrets` to load instance-qualified Jira env vars |
| `nv-daemon/src/jira/client.rs` | No structural change -- `JiraClient::new()` stays the same |
| `nv-daemon/src/jira/mod.rs` | Add `JiraRegistry` struct with `resolve(project) -> Option<&JiraClient>` |
| `nv-daemon/src/jira/tools.rs` | Update tool schemas to document project-based routing; no schema changes needed (project param already exists) |
| `nv-daemon/src/tools.rs` | Change `jira_client: Option<&JiraClient>` to `jira_registry: Option<&JiraRegistry>` in execute_tool signatures |
| `nv-daemon/src/worker.rs` | Change `SharedDeps.jira_client` from `Option<JiraClient>` to `Option<JiraRegistry>` |
| `nv-daemon/src/main.rs` | Build `JiraRegistry` from config + env vars instead of single client |
| `nv-daemon/src/agent.rs` | Update field type and all call sites |
| `nv-daemon/src/callbacks.rs` | Update `handle_approve` to use registry |
| `nv-daemon/src/aggregation.rs` | Update `project_health` to resolve client from registry by project code |
| `config/nv.toml` | Add commented example of multi-instance config |

## Risks
| Risk | Mitigation |
|------|-----------|
| TOML deserialization ambiguity between flat and nested format | Use `#[serde(untagged)]` enum or custom deserializer to try nested first, then flat |
| Instance-qualified env vars proliferate | Clear naming convention (`SERVICE_VAR_INSTANCENAME`), documented in config comments |
| Project code not found in any mapping | Graceful fallback chain: project_map -> default_project match -> "default" instance -> first instance -> error |
| Breaking change if existing code expects `Option<&JiraClient>` | Mechanical refactor -- all call sites updated in one pass, no behavioral change for single-instance users |
