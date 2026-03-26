# Implementation Tasks

<!-- beads:epic:nv-a3ey -->

## DB Migration

- [x] [1.1] [P-1] Add `last_attempt_at TEXT` column to `obligations` table via migration in `obligation_store.rs`; nullable, no default [owner:api-engineer]
- [x] [1.2] [P-1] Add `proposed_done` to `ObligationStatus` enum in `obligation_store.rs`; update `from_str` and display impls; update `list_by_status` to handle the new variant [owner:api-engineer]
- [x] [1.3] [P-2] Add `ObligationStore::update_last_attempt_at(id, timestamp)` method [owner:api-engineer]
- [x] [1.4] [P-2] Add `ObligationStore::list_ready_for_execution() -> Vec<Obligation>` method — filters `owner=nova`, `status IN (open, in_progress)`, `last_attempt_at IS NULL OR last_attempt_at < now - cooldown_hours`, ordered by priority ASC then created_at ASC [owner:api-engineer]

## Config

- [x] [2.1] [P-1] Add `AutonomyConfig` struct to `crates/nv-core/src/config.rs` — `enabled: bool`, `timeout_secs: u64` (default 300), `cooldown_hours: u32` (default 2), `idle_debounce_secs: u64` (default 60); add `autonomy: Option<AutonomyConfig>` to `NvConfig`; NO tool count cap — timeout is the only bound [owner:api-engineer]
- [x] [2.2] [P-2] Add `[autonomy]` section to `config/nv.toml` with `enabled = true` and commented defaults [owner:api-engineer]

## Obligation Executor

- [x] [3.1] [P-1] Create `crates/nv-daemon/src/obligation_executor.rs` — define `ObligationResult` enum: `Completed { summary: String }`, `Failed { error: String }`, `Timeout`, `BudgetExhausted { partial: String }` [owner:api-engineer]
- [x] [3.2] [P-1] Implement `build_obligation_context(obligation, notes) -> String` — system prompt instructing Nova to fulfill the obligation using available tools; include detected_action, source_message, priority, project_code, existing research notes [owner:api-engineer]
- [x] [3.3] [P-1] Implement `execute_obligation(obligation, deps, config) -> ObligationResult` — builds context, sends Claude turn via `ClaudeClient::send_messages` (or `AnthropicClient::send_message`), runs tool loop with NO tool count cap (unlimited tools), wraps in `tokio::time::timeout(Duration::from_secs(config.timeout_secs))` — 5-minute timeout is the only bound [owner:api-engineer]
- [x] [3.4] [P-2] After execution: call `obligation_store.update_last_attempt_at(id, Utc::now())` regardless of result [owner:api-engineer]
- [x] [3.5] [P-2] On `Completed`: store summary in obligation_notes, set status to `proposed_done`, send Telegram summary (truncated to 500 chars) with `[Confirm Done] [Reopen]` inline keyboard [owner:api-engineer]
- [x] [3.6] [P-2] On `Failed`/`Timeout`/`BudgetExhausted`: store error in obligation_notes, keep status as `in_progress`, send Telegram error message [owner:api-engineer]
- [x] [3.7] [P-2] Add `mod obligation_executor;` to `lib.rs` and `main.rs` [owner:api-engineer]

## Idle Detection + Dispatch

- [x] [4.1] [P-1] Add `last_interactive_at: Arc<AtomicU64>` to `SharedDeps` — updated to `Utc::now().timestamp()` on every interactive trigger dispatch in `orchestrator.rs` [owner:api-engineer]
- [x] [4.2] [P-1] Add `active_worker_count: Arc<AtomicU32>` to `SharedDeps` — incremented on worker dispatch, decremented on worker completion (already exists as `WorkerPool` internal state — expose it) [owner:api-engineer]
- [x] [4.3] [P-1] Add idle detection loop to orchestrator: every 30 seconds, check `active_worker_count == 0 && now - last_interactive_at > idle_debounce_secs`; if idle and autonomy enabled, call `try_execute_next_obligation()` [owner:api-engineer]
- [x] [4.4] [P-2] Implement `try_execute_next_obligation()` — calls `obligation_store.list_ready_for_execution()`, picks first, calls `obligation_executor::execute_obligation()`, handles result [owner:api-engineer]
- [x] [4.5] [P-2] Set `executing_obligation: Option<String>` flag on orchestrator to prevent double-dispatch; cleared when execution completes [owner:api-engineer]

## Callbacks

- [x] [5.1] [P-1] Add `confirm_done:{id}` callback handler in `callbacks.rs` — transitions obligation from `proposed_done` to `done`; edits Telegram message to show "Confirmed" [owner:api-engineer]
- [x] [5.2] [P-1] Add `reopen:{id}` callback handler in `callbacks.rs` — transitions obligation from `proposed_done` to `open`; edits Telegram message to show "Reopened — will retry" [owner:api-engineer]
- [x] [5.3] [P-2] Add `confirm_done:` and `reopen:` to `callback_label()` in `channels/telegram/mod.rs` [owner:api-engineer]
- [x] [5.4] [P-2] Update proactive watcher to skip obligations with `status = proposed_done` [owner:api-engineer]

## Verify

- [x] [6.1] `cargo build -p nv-daemon` passes [owner:api-engineer]
- [x] [6.2] `cargo clippy -p nv-daemon -- -D warnings` passes [owner:api-engineer]
- [x] [6.3] Unit test: `list_ready_for_execution` returns only Nova-owned open obligations outside cooldown window [owner:api-engineer]
- [x] [6.4] Unit test: `proposed_done` status round-trips through store correctly [owner:api-engineer]
- [ ] [6.5] Unit test: `execute_obligation` respects timeout (mock slow tool execution, verify it aborts after timeout_secs) [owner:api-engineer]
- [ ] [6.6] [user] Manual test: create a Nova obligation via Telegram ("check Jira status for project X"), wait for idle cycle, verify Nova executes it and sends Telegram summary
- [ ] [6.7] [user] Manual test: receive proposed_done notification, tap "Confirm Done", verify obligation transitions to done
- [ ] [6.8] [user] Manual test: receive proposed_done notification, tap "Reopen", verify obligation goes back to open and re-executes on next idle cycle
