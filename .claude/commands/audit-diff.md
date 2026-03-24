---
name: audit:diff
description: Audit only domains affected by recent git changes
type: command
execution: foreground
---

# Audit Diff — nv

Run targeted audits only for domains affected by recent git changes.

## Arguments

| Argument | Default | Description |
|----------|---------|-------------|
| `--since <ref>` | `HEAD~5` | Git ref to diff against |
| `--dry-run` | off | Show affected domains without running audits |

## Domain Mapping

Map changed files to audit domains:

| File Pattern | Domain |
|-------------|--------|
| `crates/nv-daemon/src/orchestrator.rs` | agent |
| `crates/nv-daemon/src/worker.rs` | agent |
| `crates/nv-daemon/src/claude.rs` | agent |
| `crates/nv-daemon/src/conversation.rs` | agent |
| `crates/nv-daemon/src/agent.rs` | agent |
| `crates/nv-daemon/src/channels/*` | channels |
| `crates/nv-daemon/src/messages.rs` | channels |
| `relays/*` | channels |
| `crates/nv-daemon/src/tools/*` | tools |
| `crates/nv-daemon/src/dashboard.rs` | dashboard |
| `dashboard/*` | dashboard |
| `crates/nv-daemon/src/digest/*` | digest |
| `crates/nv-daemon/src/scheduler.rs` | digest |
| `crates/nv-daemon/src/watchers/*` | watchers |
| `crates/nv-daemon/src/alert_rules.rs` | watchers |
| `crates/nv-daemon/src/obligation_store.rs` | watchers |
| `crates/nv-daemon/src/obligation_detector.rs` | watchers |
| `crates/nv-daemon/src/nexus/*` | nexus |
| `crates/nv-daemon/src/query/*` | nexus |
| `crates/nv-daemon/src/health*.rs` | infra |
| `crates/nv-daemon/src/server_health_store.rs` | infra |
| `crates/nv-daemon/src/memory.rs` | infra |
| `crates/nv-daemon/src/state.rs` | infra |
| `crates/nv-daemon/src/shutdown.rs` | infra |
| `crates/nv-core/*` | infra |
| `crates/nv-cli/*` | infra |
| `deploy/*` | infra |
| `crates/nv-daemon/src/http.rs` | agent, dashboard |

## Execution

1. Run `git diff --name-only <since>..HEAD`
2. Map changed files to domains using table above
3. Deduplicate domain list
4. Run affected domain audits in parallel
5. Report findings

## Example

```bash
# Audit domains affected by last 5 commits
/audit:diff

# Audit domains affected since a specific commit
/audit:diff --since abc1234

# Preview which domains would be audited
/audit:diff --dry-run
```
