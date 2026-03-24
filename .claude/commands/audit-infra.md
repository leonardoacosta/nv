---
name: audit:infra
description: Audit infrastructure — health, CLI, config, memory, state, deployment
type: command
execution: foreground
---

# Audit: Infrastructure

Audit cross-cutting infrastructure: health system, CLI, configuration, memory, state persistence, deployment.

## Scope

| Module | File | Purpose |
|--------|------|---------|
| Health | `crates/nv-daemon/src/health.rs` | HealthState tracking |
| Health Poller | `crates/nv-daemon/src/health_poller.rs` | Periodic health checks |
| Server Health | `crates/nv-daemon/src/server_health_store.rs` | CPU/mem/disk metrics |
| Config | `crates/nv-core/src/config.rs` | TOML config parsing |
| Types | `crates/nv-core/src/types.rs` | Shared types across crates |
| Memory | `crates/nv-daemon/src/memory.rs` | Markdown-native memory (20K char limit) |
| State | `crates/nv-daemon/src/state.rs` | JSON state persistence (~/.nv/state/) |
| Messages DB | `crates/nv-daemon/src/messages.rs` | SQLite store (messages + obligations) |
| CLI | `crates/nv-cli/src/` | `nv status`, `nv ask`, `nv check`, `nv digest`, `nv stats` |
| Deploy | `deploy/` | systemd service, install script |
| Shutdown | `crates/nv-daemon/src/shutdown.rs` | Graceful shutdown handling |

## Routes

| # | Method | Path | What to check |
|---|--------|------|---------------|
| 1 | GET | `/health` | Deep probe option, channel status, response format |
| 2 | GET | `/stats` | Token counts, daily stats, tool usage breakdown |

## Audit Checklist

### Health System
- [ ] `HealthState` tracks all subsystems (channels, watchers, Nexus)
- [ ] Deep health probe queries external services
- [ ] Health poller interval and failure tracking
- [ ] Server health metrics accuracy (CPU, memory, disk)

### Configuration
- [ ] TOML parsing covers all config sections (agent, telegram, jira, nexus, daemon, alert_rules)
- [ ] Multi-instance configs (jira.instances, stripe.instances)
- [ ] Default values sensible
- [ ] Config validation on load (invalid values caught early)
- [ ] Secret redaction in config API responses

### Memory
- [ ] Topic file management (conversations.md, tasks.md, decisions.md, people.md)
- [ ] MAX_MEMORY_READ_CHARS (20K) enforcement
- [ ] Auto-summarize at SUMMARIZE_THRESHOLD (20 H2 entries)
- [ ] Search across memory files (MAX_SEARCH_RESULTS: 10)

### State Persistence
- [ ] `last-digest.json` read/write correctness
- [ ] `PendingAction` lifecycle (Pending → Approved/Rejected → Executed/Failed)
- [ ] `ChannelState` cursor persistence across restarts
- [ ] File locking / concurrent access

### CLI
- [ ] `nv status` — health display, systemd integration
- [ ] `nv ask` — question routing, JSON output mode
- [ ] `nv check` — concurrent service probes, output formatting
- [ ] `nv digest` — trigger and display modes
- [ ] `nv stats` — budget tracking accuracy

### Deployment
- [ ] systemd unit file correctness (restart policy, env vars, dependencies)
- [ ] Install script safety (idempotent, no data loss)
- [ ] Graceful shutdown (signal handling, in-flight task completion)

### SQLite
- [ ] Migration strategy (messages.db schema evolution)
- [ ] WAL mode for concurrent reads
- [ ] Connection pooling strategy

## Memory

Persist findings to: `.claude/audit/memory/infra-memory.md`

## Findings

Log to: `~/.claude/scripts/state/nv-audit-findings.jsonl`
