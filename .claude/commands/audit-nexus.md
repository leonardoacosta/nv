---
name: audit:nexus
description: Audit Nexus — gRPC client for remote Claude Code agent sessions
type: command
execution: foreground
---

# Audit: Nexus

Audit the Nexus module: gRPC client for querying and managing Claude Code sessions across machines.

## Scope

| Module | File | Purpose |
|--------|------|---------|
| Client | `crates/nv-daemon/src/nexus/client.rs` (26.8K) | Multi-agent connection manager, session queries |
| Connection | `crates/nv-daemon/src/nexus/connection.rs` (10.5K) | gRPC channel lifecycle, ConnectionStatus |
| Stream | `crates/nv-daemon/src/nexus/stream.rs` (11.8K) | Event streaming from sessions |
| Tools | `crates/nv-daemon/src/nexus/tools.rs` (12.1K) | Tool definitions for session introspection |
| Progress | `crates/nv-daemon/src/nexus/progress.rs` (9.4K) | Progress tracking |
| Notify | `crates/nv-daemon/src/nexus/notify.rs` (11.5K) | Session notifications |
| Watchdog | `crates/nv-daemon/src/nexus/watchdog.rs` (11.6K) | Heartbeat/health monitoring |
| Query | `crates/nv-daemon/src/query/` | Session data synthesis for agent + dashboard |
| Proto | `proto/` | gRPC service definitions |

## Key Types

- `NexusClient` — thread-safe `Vec<Arc<Mutex<NexusAgentConnection>>>`
- `NexusAgentConnection` — single gRPC connection
- `SessionSummary` — ID, project, status, agent_name, duration, branch, spec
- `SessionDetail` — extends summary with cwd, command, type, model, cost_usd
- `ConnectionStatus` enum — connection states

## Audit Checklist

### Client
- [ ] Multi-agent connect logic (named agents: homelab, macbook, etc.)
- [ ] Session filter queries (by project, status, agent)
- [ ] Thread safety (Arc<Mutex> usage patterns)
- [ ] Connection pooling and reuse

### Connection
- [ ] gRPC channel lifecycle (connect, reconnect, disconnect)
- [ ] ConnectionStatus transitions
- [ ] Timeout handling on gRPC calls
- [ ] TLS/auth configuration

### Stream
- [ ] Event streaming reliability (reconnect on drop)
- [ ] Backpressure handling
- [ ] Event deserialization correctness

### Watchdog
- [ ] Heartbeat interval and failure detection
- [ ] Reconnection strategy on agent unavailability
- [ ] Health status aggregation across agents

### Notifications
- [ ] Session start/complete/error notifications
- [ ] Notification routing (Telegram, dashboard, etc.)
- [ ] Notification deduplication

### Query Module
- [ ] `gather.rs` — session metadata collection correctness
- [ ] `synthesize.rs` — multi-session aggregation
- [ ] `format.rs` — display formatting
- [ ] `followup.rs` — follow-up suggestion quality

## Memory

Persist findings to: `.claude/audit/memory/nexus-memory.md`

## Findings

Log to: `~/.claude/scripts/state/nv-audit-findings.jsonl`
