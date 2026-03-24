# Infra Domain Audit — nv project

**Date:** 2026-03-23
**Scope:** health.rs, health_poller.rs, server_health_store.rs, config.rs, types.rs, memory.rs, state.rs, messages.rs, nv-cli/src/, deploy/

---

## Checklist Results

### Health System

- [PASS] HealthState tracks all subsystems — channels (HashMap<String, ChannelStatus>), last_digest_at, triggers_processed, version
- [PASS] Deep health probe queries all 14 external services concurrently via check_all()
- [PASS] Health poller runs on 60s interval; first tick skipped to let daemon settle; poll cycle errors are logged as warn, not fatal
- [CONCERN] HealthState.to_health_response() always returns status:"ok" regardless of disconnected channels — top-level status field does not reflect degraded state
- [CONCERN] CPU idle calculation omits iowait (field index 3 = idle only, not idle+iowait). Busy% is understated on I/O-heavy workloads
- [CONCERN] disk usage uses f_bfree instead of f_bavail — slightly under-reports used disk on Linux (root-reserved blocks counted as free)
- [CONCERN] ServerHealthStore opens a fresh SQLite connection per poll cycle (every 60s) — minor overhead, not a correctness issue
- [CONCERN] ServerHealthStore.previous() method appears to be dead code — production crash detection uses latest() before insert

### Configuration

- [PASS] TOML parsing covers all config sections: agent, telegram, discord, teams, email, imessage, jira, nexus, daemon, calendar, web, doppler, projects, alert_rules
- [PASS] Multi-instance configs implemented for Jira (JiraConfig flat/multi enum) and generically via ServiceConfig<T>
- [PASS] Default values are sensible (health_port=8400, digest_interval=60m, max_workers=3, weekly_budget_usd=50.0, timezone="America/Chicago")
- [PASS] Project paths resolved and validated on load (invalid paths silently dropped with warn log)
- [PASS] Secrets sourced exclusively from environment variables, never from config file
- [CONCERN] quiet_start/quiet_end are not validated as valid HH:MM strings at parse time — invalid values accepted silently
- [CONCERN] ServiceInstanceConfig is a marker type with no fields; dead abstraction that adds type noise

### Memory

- [PASS] Topic files: conversations.md, tasks.md, decisions.md, people.md (default set)
- [PASS] MAX_MEMORY_READ_CHARS (20K) enforced in read() — truncates to recent entries
- [PASS] SUMMARIZE_THRESHOLD (20 H2 entries) implemented in needs_summarization()
- [PASS] Auto-summarize via Claude calls implemented in summarize() with safe error handling (never corrupts original on failure)
- [PASS] Search across all .md files with MAX_SEARCH_RESULTS=10 enforced
- [CONCERN] Memory.get_context_summary() marked #[allow(dead_code)] — confirm it's intentionally reserved or remove it
- [CONCERN] write() does two file reads + two atomic writes per append (read existing, write with new entry, read again for frontmatter update, write again)

### State Persistence

- [PASS] last-digest.json read/write with atomic_write (tmp → rename pattern)
- [PASS] PendingAction lifecycle complete: AwaitingConfirmation → Approved/Rejected → Executed/Cancelled/Expired
- [PASS] ChannelState cursor persistence across restarts via channel-state.json
- [PASS] Atomic write pattern used for all JSON state files
- [CONCERN] save_pending_action() is a read-modify-write on the entire array — concurrent workers can create a lost-update race

### CLI

- [PASS] nv status — queries /health, integrates systemd status, graceful fallback when daemon not running
- [PASS] nv ask — POST /ask with 65s timeout, JSON output mode, connect/timeout error differentiation
- [PASS] nv check — concurrent service probes, JSON and terminal output modes, --service filter, --read-only flag
- [PASS] nv stats — budget tracking, tool usage breakdown, Claude API usage, daily bar chart
- [CONCERN] nv digest without --now prints "not implemented yet" — dev artifact in user-facing CLI
- [CONCERN] nv config prints "not implemented yet" — dev artifact in user-facing CLI
- [CONCERN] nv check includes TeamsCheck but health.rs deep probe does not — inventories out of sync

### Deployment

- [PASS] systemd unit: Type=notify, Restart=on-failure, RestartSec=5s, TimeoutStopSec=30, MemoryMax=2G, LimitNOFILE=4096
- [PASS] Doppler integration via `doppler run --fallback=true` — offline resilience built in
- [PASS] install.sh is idempotent — safe to re-run; does not delete ~/.nv data
- [PASS] SIGTERM and Ctrl+C handled via tokio::select in wait_for_shutdown_signal()
- [PASS] drain_with_timeout() provided for graceful in-flight task drain on shutdown
- [CONCERN] WatchdogSec=60 set but not confirmed nv-daemon sends sd_notify WATCHDOG=1 — if it doesn't, systemd will kill and restart every 60s
- [CONCERN] install.sh unconditionally enables nv-teams-relay.service even without Teams configuration (fallback path at line 115)
- [CONCERN] install.sh uses sleep 3 as a post-start health check gate — fragile

### SQLite

- [PASS] Versioned migration strategy via rusqlite_migration crate (4 versions tracked)
- [PASS] WAL mode enabled on every connection open (PRAGMA journal_mode=WAL)
- [PASS] FTS5 triggers maintain messages_fts index automatically; backfill on init
- [CONCERN] No explicit WAL checkpoint strategy — relies on SQLite's default auto-checkpoint at 1000 pages
- [CONCERN] No connection pool — MessageStore uses a single Connection; concurrent handlers serialize through Arc<Mutex<MessageStore>>

---

## Key Findings (by severity)

### Medium

1. **Hardcoded absolute paths to /home/nyaptor** in `health_poller.rs:176`, `claude.rs:255`, `callbacks.rs:132` — breaks portability and multi-user deployments
2. **pending-actions.json read-modify-write race** — concurrent workers (multiple message handlers completing simultaneously) can lose pending action updates

### Low (Debt-Inducing)

3. CPU busy% understated on I/O-heavy workloads (iowait not counted as busy)
4. Disk used% slightly under-reported (f_bfree vs f_bavail)
5. HealthState top-level status always "ok" — degraded channels not reflected
6. quiet_start/quiet_end not validated at parse time
7. nv digest / nv config have "not implemented yet" stubs in user-facing CLI
8. nv check and /health deep probe have divergent service inventories (Teams missing from deep probe)
9. WatchdogSec=60 in systemd unit — verify WATCHDOG=1 sd_notify is emitted
10. install.sh enables Teams relay unconditionally

---

## Overall Assessment

The infra domain is structurally sound with good test coverage across all core modules. The migration system, WAL mode, atomic writes, graceful shutdown, and config validation are all implemented correctly. The hardcoded `/home/nyaptor` paths are the most actionable items. The pending-actions race is low risk in practice (the daemon is single-user and workers are I/O-bound) but worth documenting.

No blocking issues found. All concerns are debt-inducing.
