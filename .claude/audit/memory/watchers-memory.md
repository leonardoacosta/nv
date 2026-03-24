# Watchers Domain Audit Memory

**Audited:** 2026-03-23
**Auditor:** codebase-health-analyst
**Scope:** nv-daemon watchers domain (deploy, sentry, stale_ticket, ha, mod, alert_rules, obligation_store, obligation_detector)

---

## Scores

| Axis         | Score | Notes |
|--------------|-------|-------|
| Structure    | 82    | Clean module layout, good test coverage on pure helpers, one dead utility (drain_with_timeout unused) |
| Quality      | 74    | Hardcoded HOME path, silent parse failure on Sentry count, cooldown logic absent |
| Architecture | 71    | No cooldown/dedup mechanism is the dominant design gap; N+1 HA queries; detector has no timeout |
| **Health**   | **75** | B — good foundation, clear actionable gaps |

```
health = (82 * 0.30) + (74 * 0.35) + (71 * 0.35) = 24.6 + 25.9 + 24.85 = 75.35
```

---

## Checklist Results

### Watcher Lifecycle

| Item | Result | Notes |
|------|--------|-------|
| spawn_watchers() — all 4 watchers spawned | PASS | DeployFailure, SentrySpike, StaleTicket, HaAnomaly dispatched via evaluate_rule |
| Interval timing (default 300s, configurable) | PASS | interval_secs.max(60) enforces minimum 60s floor |
| run_watcher_cycle() — concurrent evaluation | PASS | tokio::spawn per rule, futures joined via handle.await loop |
| Non-fatal error handling | PASS | All watcher errors logged as tracing::warn, never propagated |
| Graceful shutdown on daemon stop | CONCERN | JoinHandle returned by spawn_watchers is discarded in main.rs — no cancellation on SIGTERM; drain_with_timeout exists but is unused (#[allow(dead_code)]) |

### Per-Watcher

| Item | Result | Notes |
|------|--------|-------|
| Deploy: Vercel API query window | PASS | window_minutes parsed from config (default 10), cutoff_ms computed from chrono::Utc::now() |
| Deploy: error state detection | PASS | case-insensitive match on "ERROR" and "FAILED"; test coverage present |
| Deploy: empty projects guard | CONCERN | Returns None silently when projects list is empty (no warn-level log, doc says "checks all" but impl skips) |
| Sentry: spike count threshold | PASS | threshold parsed from config (default 10) |
| Sentry: project filtering | PASS | project slug required; warns and returns None if absent |
| Sentry: count parse failure | FAIL | issue.count is a String; parse::<u64>() failure is silently ignored via unwrap_or(false) — no tracing::warn |
| StaleTicket: age threshold calculation | PASS | chrono::Duration::days(stale_days), RFC3339 parse with debug logging on error |
| StaleTicket: beads.jsonl parsing | PASS | Line-by-line, malformed lines skipped with debug log |
| HA: entity state anomaly detection | PASS | Case-insensitive match against configurable anomaly_states list |
| HA: entity filtering | PASS | Requires entities list in config; warns if absent |
| HA: N+1 API calls | CONCERN | Sequential entity fetches; HA /api/states endpoint supports bulk fetch |

### Alert Rules

| Item | Result | Notes |
|------|--------|-------|
| Rule storage format and persistence | PASS | SQLite alert_rules table, versioned migration v3 |
| enabled flag respected | PASS | list_enabled() uses WHERE enabled = 1 |
| last_triggered_at updated correctly | PASS | touch_triggered() called after obligation stored |
| Cooldown logic (prevent repeated firings) | FAIL | last_triggered_at is written but never read during evaluation. No cooldown check. Persistent conditions create a new obligation every watcher cycle (every 5 min). |

### Obligation Store

| Item | Result | Notes |
|------|--------|-------|
| SQLite table creation and migrations | PASS | Migration v2 in messages.rs; IF NOT EXISTS on indexes |
| CRUD operations (create, get, update_status, list) | PASS | Full CRUD present with tests |
| Status transitions: Open -> InProgress -> Done/Dismissed | PASS | ObligationStatus::from_str handles all four values; update_status and update_status_and_owner both present |
| Priority (0-4) stored and filtered | PASS | ORDER BY priority ASC in all list queries |
| Owner assignment (Nova vs Leo) | PASS | ObligationOwner enum with as_str/from_str |
| Concurrent access safety | PASS | Arc<Mutex<ObligationStore>> — one connection per store instance, WAL mode enabled |

### Obligation Detector

| Item | Result | Notes |
|------|--------|-------|
| Commitment detection from inbound messages | PASS | claude CLI with structured JSON output, is_obligation flag |
| False positive handling | PASS | owner validation, detected_action empty check, priority clamped to 0-4 |
| Source channel attribution | PASS | channel passed through to tracing and included in prompt |
| Hardcoded HOME fallback | FAIL | "/home/nyaptor" hardcoded as final fallback — breaks in CI and other environments |
| Subprocess timeout | CONCERN | No tokio::time::timeout around wait_with_output — hung claude process blocks indefinitely |

---

## Findings Summary

### High Severity (1)

**No cooldown/dedup on rule firing**
- File: `crates/nv-daemon/src/watchers/mod.rs` (evaluate_rule)
- `last_triggered_at` is updated after firing but is never checked before the next evaluation. A Vercel deploy that stays in ERROR state, or a HA entity stuck as "unavailable", will create a new obligation every 5 minutes indefinitely.
- Fix: In `evaluate_rule`, read `rule.last_triggered_at`, parse it, and skip evaluation if now - last_triggered < cooldown_secs (configurable per rule or global).

### Medium Severity (3)

**JoinHandle discarded — no graceful watcher shutdown**
- File: `crates/nv-daemon/src/main.rs` line 723
- `spawn_watchers()` returns a `JoinHandle<()>` that is immediately dropped. On SIGTERM, in-flight watcher tasks are abandoned without cancellation. `drain_with_timeout` exists in shutdown.rs but is unused.
- Fix: Store the handle in the daemon state; on shutdown, `handle.abort()` and await.

**DeployWatcher silent skip when projects empty**
- File: `crates/nv-daemon/src/watchers/deploy_watcher.rs` line 56
- Returns `None` at `tracing::debug` level when the projects list is missing. Operators who misconfigure the rule (omitting the `projects` key) get no alert and no actionable warning.
- Fix: Elevate to `tracing::warn` and include a hint ("add 'projects' to rule config").

**Hardcoded fallback HOME path in obligation_detector**
- File: `crates/nv-daemon/src/obligation_detector.rs` line 134
- `"/home/nyaptor"` is baked in as the default when env vars are absent. Will silently break on any other machine, CI runner, or Docker container.
- Fix: Return an `Err` instead of using a hardcoded path, or read from `/etc/passwd` / `getpwuid`.

### Low Severity (4)

**Sentry issue.count parse failure is silent**
- `parse::<u64>()` returning `Err` maps to `unwrap_or(false)` — the issue is silently excluded. Log at `tracing::debug` at minimum.

**StaleTicketWatcher drops project_code**
- All stale ticket obligations have `project_code: None`. Cosmetic but reduces dashboard utility.

**HA watcher: N+1 sequential entity fetches**
- One HTTP call per watched entity. Use `/api/states` for bulk retrieval when entity count > 3.

**ObligationDetector: no subprocess timeout**
- `wait_with_output()` has no `tokio::time::timeout` wrapper. A hung `claude` process permanently blocks that message handler slot.

---

## Key Design Observations

1. **Cooldown gap is the dominant risk.** Without it, any persistent failure state will create a storm of duplicate obligations. This should be the first fix.

2. **The RuleEvaluator trait is clean.** The `evaluate(&self, rule: &AlertRule) -> impl Future<Output = Option<NewObligation>>` contract is simple and consistent across all four watchers. Testability of pure helpers (collect_failed_summaries, build_obligation_from_failures) is excellent.

3. **Schema migration ordering dependency is undocumented at call sites.** ObligationStore and AlertRuleStore both call `Connection::open` without running migrations, relying on MessageStore::init having been called first. This is documented in module docs but not enforced. An integration test opening AlertRuleStore first against a fresh DB would fail. The deploy_watcher.rs test gets the order right (ObligationStore before AlertRuleStore) but for the wrong-seeming reason (only noted in a comment).

4. **WAL mode is correctly applied** to every store connection. No concurrent access issues.

5. **All watcher errors are non-fatal.** Every external API failure degrades to `tracing::warn` and `return None`. This is the correct approach for proactive watchers.
