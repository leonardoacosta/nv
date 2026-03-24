# Proposal: Fix Watcher Reliability

## Change ID
`fix-watcher-reliability`

## Summary

Seven reliability defects in the proactive watcher subsystem, spanning obligation flooding,
missing shutdown coordination, a hardcoded home directory path, and several silent failure
modes. All are contained within `nv-daemon`.

## Context
- Extends: `crates/nv-daemon/src/watchers/mod.rs` (evaluate_rule, spawn_watchers)
- Extends: `crates/nv-daemon/src/watchers/deploy_watcher.rs` (missing projects log level)
- Extends: `crates/nv-daemon/src/watchers/sentry_watcher.rs` (silent parse failure)
- Extends: `crates/nv-daemon/src/watchers/ha_watcher.rs` (N+1 sequential HTTP)
- Extends: `crates/nv-daemon/src/obligation_detector.rs` (hardcoded path, no timeout)
- Extends: `crates/nv-daemon/src/main.rs` (JoinHandle discarded)
- Extends: `crates/nv-daemon/src/shutdown.rs` (dead code drain_with_timeout)
- Related: Audit 2026-03-23 (watchers domain, 75/B- health score)

## Motivation

The watcher subsystem has a P1 correctness defect and three P2 reliability defects that
combine to produce noisy behaviour in production:

1. **Obligation flooding (P1)** — `evaluate_rule` writes `last_triggered_at` after every
   fire but never reads it before the next evaluation. A persistent external failure (e.g.
   a Vercel deploy stuck in ERROR state) creates one new duplicate obligation every 5 minutes
   indefinitely. The obligation store accumulates unbounded duplicates with no deduplication.

2. **Watchers not cancelled on shutdown (P2)** — `spawn_watchers()` returns a
   `JoinHandle<()>` that is immediately discarded at the call site in `main.rs:723`. When
   the daemon receives SIGTERM, in-flight watcher tasks are abandoned. There is no graceful
   cancellation.

3. **Hardcoded fallback path in obligation detector (P2)** — `obligation_detector.rs:134`
   falls back to `"/home/nyaptor"` when the `HOME` environment variable is absent. This is a
   developer machine path baked into production code. Running under a service account or
   Docker container with no `HOME` silently misconfigures the subprocess environment.

4. **DeployWatcher silently skips on missing config (P2)** — when the `projects` key is
   absent from a rule's config JSON, the watcher returns `None` with a `debug!` log. A
   misconfigured rule produces no alert and no actionable warning in production logs.

Three P3 issues complete the reliability picture:

5. **Sentry count parse failure is silent (P3)** — `issue.count.parse::<u64>()` uses
   `.unwrap_or(false)` with no log. When Sentry returns an unexpected count format, affected
   issues are silently excluded from threshold evaluation.

6. **ObligationDetector subprocess has no timeout (P3)** — `wait_with_output()` is
   unbounded. A hung Claude CLI process blocks the detection path indefinitely.

7. **HA watcher makes N+1 sequential HTTP calls (P3)** — one `client.entity(id)` call per
   configured entity in a serial loop. `HAClient::states()` (bulk `/api/states` endpoint)
   already exists in the codebase. Switching to it eliminates the N+1 pattern.

One P4 dead code item is included as a housekeeping task:

8. **`drain_with_timeout` dead code (P4)** — `shutdown.rs:37` has `#[allow(dead_code)]`.
   Wire it into the shutdown path or remove it.

## Requirements

### Req-1: Cooldown guard in evaluate_rule

Before calling the watcher's `evaluate()` method, read `rule.last_triggered_at`. If the
elapsed time since last trigger is less than the watcher cycle interval (`interval_secs`),
skip evaluation and log at `debug!`. This prevents re-firing on every cycle while a
condition remains active.

The cooldown period should equal the watcher interval (already available as the loop
`Duration`). Pass `interval_secs` into `evaluate_rule` so the guard can compare:

```
now - last_triggered_at < interval_secs  →  skip
```

`last_triggered_at` is stored as an SQLite `datetime('now')` string (UTC, RFC 3339-ish).
Parse it with `chrono::NaiveDateTime::parse_from_str` or `DateTime::parse_from_rfc3339`.

### Req-2: Bind and abort watcher JoinHandle on shutdown

In `main.rs`, bind the return value of `spawn_watchers(...)` to a named variable. In the
shutdown `select!` arm (after `wait_for_shutdown_signal()` resolves), call
`watcher_handle.abort()` before the process exits. This ensures in-flight watcher tasks
are cancelled on SIGTERM/Ctrl+C.

### Req-3: Remove hardcoded /home/nyaptor fallback

In `obligation_detector.rs`, replace the `unwrap_or_else(|_| "/home/nyaptor".into())`
fallback with a hard error. If neither `REAL_HOME` nor `HOME` is set, return
`Err(anyhow::anyhow!("HOME env var not set — cannot spawn obligation detector"))`.
The caller already handles `Err` gracefully and logs a warning.

### Req-4: Upgrade DeployWatcher missing-projects log to warn

In `deploy_watcher.rs:56-61`, change `tracing::debug!` to `tracing::warn!` and add a
config hint in the message:

```
"deploy_watcher: no 'projects' configured in rule config — add {\"projects\": [\"my-project\"]} to rule config"
```

### Req-5: Log Sentry count parse failures

In `sentry_watcher.rs`, replace the silent `.unwrap_or(false)` on the count parse with an
explicit match that emits `tracing::debug!` when parse fails, including the raw count string
and the issue title. Continue treating the parse failure as non-spiked (return `false`).

### Req-6: Add subprocess timeout to ObligationDetector

Wrap the `child.wait_with_output().await?` call in
`tokio::time::timeout(Duration::from_secs(30), ...)`. On timeout expiry, kill the child
process and return `Err(anyhow::anyhow!("obligation detector subprocess timed out after 30s"))`.

### Req-7: Replace HA watcher N+1 with bulk states call

Replace the sequential `for entity_id in &entities { client.entity(entity_id).await }`
loop with a single `client.states().await` call. Filter the returned `Vec<HAEntity>` to
only the entity IDs in the configured list, then apply the anomaly-state check. Warn if
a configured entity ID is absent from the bulk response.

### Req-8: Wire or remove drain_with_timeout

Either:
- Wire `drain_with_timeout` into the shutdown sequence after `wait_for_shutdown_signal()`
  returns, draining the inbound trigger channel with a 2-second timeout, and remove the
  `#[allow(dead_code)]` attribute; or
- Delete the function entirely if the shutdown path has no channel to drain.

Prefer wiring if there is a `trigger_rx` in scope at shutdown; prefer deletion otherwise.

## Scope
- **IN**: cooldown guard, JoinHandle abort, HOME error, deploy log level, Sentry parse log,
  detector timeout, HA bulk states, drain_with_timeout resolution
- **OUT**: obligation deduplication in the store, watcher interval configurability per-rule,
  alert rule UI changes

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/watchers/mod.rs` | Cooldown guard in `evaluate_rule`; pass `interval_secs` |
| `crates/nv-daemon/src/watchers/deploy_watcher.rs` | `debug!` → `warn!` with config hint |
| `crates/nv-daemon/src/watchers/sentry_watcher.rs` | Log parse failures at `debug!` |
| `crates/nv-daemon/src/watchers/ha_watcher.rs` | Replace N+1 loop with `client.states()` |
| `crates/nv-daemon/src/obligation_detector.rs` | Remove hardcoded path; add 30s timeout |
| `crates/nv-daemon/src/main.rs` | Bind `spawn_watchers` handle; abort on shutdown |
| `crates/nv-daemon/src/shutdown.rs` | Wire or remove `drain_with_timeout` |

## Risks
| Risk | Mitigation |
|------|-----------|
| Cooldown skips a real new failure that coincides with the interval boundary | Cooldown only applies when `last_triggered_at` is set — first fire always goes through |
| `client.states()` returns a large payload with many entities | Filter immediately after fetch; no behavioural change for the caller |
| Aborting the watcher JoinHandle mid-cycle drops an in-progress obligation write | SQLite write is atomic; partial cycle leaves no corrupt state |
| 30s detector timeout too tight for cold-start Claude CLI on slow hardware | Value is a constant — easy to tune; 30s is already used for tool timeouts elsewhere |
