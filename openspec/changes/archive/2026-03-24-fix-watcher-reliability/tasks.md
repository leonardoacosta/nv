# Implementation Tasks

<!-- beads:epic:TBD -->

## API Batch

- [x] [api-engineer] Add `interval_secs: u64` parameter to `evaluate_rule` signature and thread it through from `run_watcher_cycle` — `crates/nv-daemon/src/watchers/mod.rs`
- [x] [api-engineer] In `evaluate_rule`, parse `rule.last_triggered_at` and skip evaluation (with `tracing::debug!`) if elapsed time is less than `interval_secs` — `crates/nv-daemon/src/watchers/mod.rs`
- [x] [api-engineer] Bind the return value of `spawn_watchers(...)` to a named `watcher_handle` variable in `main.rs` — `crates/nv-daemon/src/main.rs`
- [x] [api-engineer] Call `watcher_handle.abort()` in the shutdown `select!` arm after `wait_for_shutdown_signal()` resolves — `crates/nv-daemon/src/main.rs`
- [x] [api-engineer] Replace `unwrap_or_else(|_| "/home/nyaptor".into())` in `detect_obligation` with `return Err(...)` when both `REAL_HOME` and `HOME` are absent — `crates/nv-daemon/src/obligation_detector.rs`
- [x] [api-engineer] Wrap `child.wait_with_output().await` in `tokio::time::timeout(Duration::from_secs(30), ...)` and kill child + return `Err` on timeout — `crates/nv-daemon/src/obligation_detector.rs`
- [x] [api-engineer] Change `tracing::debug!` to `tracing::warn!` on the empty-projects branch in `DeployWatcher::evaluate` and include a config hint in the message — `crates/nv-daemon/src/watchers/deploy_watcher.rs`
- [x] [api-engineer] Replace silent `.unwrap_or(false)` on `issue.count.parse::<u64>()` with an explicit match that emits `tracing::debug!` on parse error — `crates/nv-daemon/src/watchers/sentry_watcher.rs`
- [x] [api-engineer] Replace sequential `for entity_id` loop in `HaWatcher::evaluate` with a single `client.states().await` call, filter by configured entity IDs, warn on missing IDs — `crates/nv-daemon/src/watchers/ha_watcher.rs`
- [x] [api-engineer] Wire `drain_with_timeout` into the shutdown path using the inbound trigger channel, or delete the function and its `#[allow(dead_code)]` if no channel is in scope — `crates/nv-daemon/src/shutdown.rs`

## Verify

- [x] [api-engineer] `cargo build` passes with no errors
- [x] [api-engineer] `cargo clippy -- -D warnings` passes
- [x] [api-engineer] Unit test: cooldown guard skips evaluation when `last_triggered_at` is within `interval_secs` of now
- [x] [api-engineer] Unit test: cooldown guard allows evaluation when `last_triggered_at` is older than `interval_secs`
- [x] [api-engineer] Unit test: cooldown guard allows evaluation when `last_triggered_at` is `None` (first fire)
- [x] [api-engineer] Unit test: `detect_obligation` returns `Err` when both `HOME` and `REAL_HOME` are unset
- [x] [api-engineer] Unit test: Sentry watcher count parse failure emits debug log and treats issue as non-spiked
- [x] [api-engineer] Existing tests pass (`cargo test`)
