---
name: audit:watchers
description: Audit proactive monitoring — watchers, alert rules, obligations
type: command
execution: foreground
---

# Audit: Watchers

Audit the background monitoring system: watchers evaluate alert rules and create obligations.

## Scope

| Watcher | File | Alert Type | Service |
|---------|------|-----------|---------|
| DeployWatcher | `crates/nv-daemon/src/watchers/deploy_watcher.rs` | `deploy_failure` | Vercel REST API |
| SentryWatcher | `crates/nv-daemon/src/watchers/sentry_watcher.rs` | `sentry_spike` | Sentry REST API |
| StaleTicketWatcher | `crates/nv-daemon/src/watchers/stale_ticket_watcher.rs` | `stale_ticket` | Local beads.jsonl |
| HaWatcher | `crates/nv-daemon/src/watchers/ha_watcher.rs` | `ha_anomaly` | Home Assistant API |
| Spawn/Cycle | `crates/nv-daemon/src/watchers/mod.rs` | — | Watcher lifecycle |
| Alert Rules | `crates/nv-daemon/src/alert_rules.rs` | — | Rule storage & evaluation |
| Obligations | `crates/nv-daemon/src/obligation_store.rs` | — | SQLite CRUD |
| Detector | `crates/nv-daemon/src/obligation_detector.rs` | — | Obligation detection from messages |

## Audit Checklist

### Watcher Lifecycle
- [ ] `spawn_watchers()` — all 4 watchers spawned correctly
- [ ] Interval timing (default 300s, configurable)
- [ ] `run_watcher_cycle()` — concurrent evaluation via `tokio::join!`
- [ ] Non-fatal error handling (warnings, not panics)
- [ ] Graceful shutdown on daemon stop

### Per-Watcher
- [ ] **Deploy**: Vercel API query window (window_minutes), error state detection
- [ ] **Sentry**: Spike count threshold evaluation, project filtering
- [ ] **StaleTicket**: Age threshold calculation, beads.jsonl parsing
- [ ] **HA**: Entity state anomaly detection logic, entity filtering

### Alert Rules
- [ ] Rule storage format and persistence
- [ ] `enabled` flag respected
- [ ] `last_triggered_at` updated correctly
- [ ] Cooldown logic (prevent repeated firings)

### Obligation Store
- [ ] SQLite table creation and migrations
- [ ] CRUD operations (create, get, update_status, list)
- [ ] Status transitions: Open → InProgress → Done/Dismissed
- [ ] Priority (0-4) correctly stored and filtered
- [ ] Owner assignment (Nova vs Leo)
- [ ] Concurrent access safety

### Obligation Detector
- [ ] Commitment detection from inbound messages
- [ ] False positive rate (over-detecting vs under-detecting)
- [ ] Source channel attribution

## Memory

Persist findings to: `.claude/audit/memory/watchers-memory.md`

## Findings

Log to: `~/.claude/scripts/state/nv-audit-findings.jsonl`
