# Implementation Tasks

<!-- beads:epic:TBD -->

## API Batch

- [x] [api-engineer] Remove all 5x `#[allow(dead_code)]` attributes from `digest/mod.rs` and resolve any resulting compiler errors — `crates/nv-daemon/src/digest/mod.rs`
- [x] [api-engineer] Add `pub claude_client: ClaudeClient` and `pub health_port: u16` fields to `SharedDeps` struct — `crates/nv-daemon/src/worker.rs`
- [x] [api-engineer] Populate `claude_client` (clone of existing `client`) and `health_port` (from `config.daemon.health_port`) in the `SharedDeps { ... }` initializer — `crates/nv-daemon/src/main.rs`
- [x] [api-engineer] Replace `TriggerClass::Digest` fall-through in orchestrator with inline async handler: call `gather_context()` → `synthesize_digest()` (fallback on error) → `state.should_send()` check → send to Telegram → `state.record_sent()` — `crates/nv-daemon/src/orchestrator.rs`
- [x] [api-engineer] Wire `inject_budget_warning()` into the Digest handler — call after `synthesize_digest()` succeeds and threshold is exceeded; format: `"[Budget] ${spent:.2} / ${limit:.2} this week ({pct}%)"` — `crates/nv-daemon/src/orchestrator.rs`
- [x] [api-engineer] Replace `let port = 8400;` in `cmd_digest()` with `let port = self.deps.health_port;` — `crates/nv-daemon/src/orchestrator.rs` (already done in Wave 1 — verified at line 1034)
- [x] [api-engineer] Add `LIMIT 20` to the JQL string in `gather_jira()` to cap Jira results — `crates/nv-daemon/src/digest/gather.rs`
- [x] [api-engineer] Add `max_tokens: 1024` to the Claude API call in `synthesize_digest()` — added `send_messages_with_options()` to `ClaudeClient` — `crates/nv-daemon/src/digest/synthesize.rs`
- [x] [api-engineer] Expose `DigestStateManager::save()` as `pub` if it is not already — already `pub` — `crates/nv-daemon/src/digest/state.rs`
- [x] [api-engineer] Replace N-cycle loop in `dismiss_all_actions()` with single load/mutate/save: load state once, iterate `iter_mut()` to flip `Pending` → `Dismissed`, then call `state_mgr.save()` once — `crates/nv-daemon/src/digest/actions.rs`
- [x] [api-engineer] Change morning briefing condition from `current_hour >= MORNING_BRIEFING_HOUR` to `current_hour == MORNING_BRIEFING_HOUR` — `crates/nv-daemon/src/scheduler.rs`

## Verify

- [x] [api-engineer] `cargo build` passes with no dead_code warnings in the digest module
- [x] [api-engineer] `cargo clippy -- -D warnings` passes
- [x] [api-engineer] Unit test: `dismiss_all_actions()` with 2 pending actions results in 1 `save()` call and both statuses set to `Dismissed` (refactor existing test to assert single-save behavior)
- [x] [api-engineer] Unit test: `gather_jira()` JQL string contains `LIMIT 20`
- [x] [api-engineer] Unit test: morning briefing does not fire when `current_hour == MORNING_BRIEFING_HOUR + 1` (e.g., hour 8) with stale `last_briefing_date`
- [x] [api-engineer] Existing digest tests pass (`cargo test -p nv-daemon digest`)
