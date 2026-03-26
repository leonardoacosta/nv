# Tasks: Proactive Obligation Research

## Spec
`proactive-obligation-research`

## Beads Epic
<!-- bd create "Proactive obligation research" -t epic -p 2 -->
<!-- EPIC_ID: nv-iz9e -->

---

## Phase 1 — Core Types and Config

- [x] Add `ObligationResearchTrigger` struct to `crates/nv-core/src/types.rs`
- [x] Add `Trigger::ObligationResearch(ObligationResearchTrigger)` variant to `Trigger` enum in `crates/nv-core/src/types.rs`
- [x] Add `ObligationResearchConfig` struct to `crates/nv-core/src/config.rs` with defaults (`enabled=true`, `max_tools=5`, `min_priority=2`, `research_delay_secs=10`)
- [x] Add `obligation_research: Option<ObligationResearchConfig>` field to `Config` struct in `crates/nv-core/src/config.rs`
- [x] Gate: `cargo build -p nv-core` passes

## Phase 2 — Storage Layer

- [x] Add `obligation_notes` SQLite migration to `crates/nv-daemon/src/messages.rs` (next migration version after current highest)
- [x] Add `Finding` and `ResearchResult` types to new `crates/nv-daemon/src/obligation_research.rs`
- [x] Add `ObligationStore::save_research_result(result: &ResearchResult) -> Result<()>` to `crates/nv-daemon/src/obligation_store.rs`
- [x] Add `ObligationStore::get_latest_research(obligation_id: &str) -> Result<Option<ResearchResult>>` to `crates/nv-daemon/src/obligation_store.rs`
- [x] Add `ObligationStore::list_research_by_obligation(obligation_id: &str) -> Result<Vec<ResearchResult>>` to `crates/nv-daemon/src/obligation_store.rs`
- [x] Unit tests for `save_research_result` / `get_latest_research` round-trip in `obligation_store.rs` test module
- [x] Gate: `cargo test -p nv-daemon obligation_store` passes

## Phase 3 — Research Module

- [x] Implement `schedule_research(obligation, deps, config)` in `obligation_research.rs` — spawns background Tokio task with `sleep(research_delay_secs)`, checks obligation still `Open` before dispatching, creates `WorkerTask` with `Priority::Low` and `Trigger::ObligationResearch`
- [x] Implement `min_priority` filter: skip if `obligation.priority > config.min_priority`
- [x] Implement `trigger_channels` filter: skip if non-empty and channel not in list
- [x] Implement `dismissed` guard: re-fetch obligation from store after delay; skip if status is not `Open`
- [x] Unit tests for filter logic (min_priority gate, channel gate, dismissed gate)
- [x] Gate: `cargo test -p nv-daemon obligation_research` passes

## Phase 4 — Worker Integration

- [x] Add `obligation_research_config: Option<ObligationResearchConfig>` to `SharedDeps` in `crates/nv-daemon/src/worker.rs`
- [x] Wire `obligation_research_config` in `main.rs` / `lib.rs` where `SharedDeps` is constructed
- [x] Add `Trigger::ObligationResearch` branch to `Worker::run` in `worker.rs`:
  - Build research system prompt supplement from trigger fields
  - Cap tool loop to `config.max_tools` iterations
  - Execute Claude, parse JSON result from response
  - Store result via `ObligationStore::save_research_result`
  - No outbound channel message
  - On JSON parse failure: store `ResearchResult` with `error` field set, `summary = "Research failed: <reason>"`
- [x] Unit test: `Worker` branch handles valid JSON research result without panic
- [x] Unit test: `Worker` branch handles malformed JSON without panic (stores error result)
- [x] Gate: `cargo build -p nv-daemon` passes

## Phase 5 — Orchestrator Wiring

- [x] In `orchestrator.rs`, after `ObligationStore::create` succeeds in the obligation detection block, call `obligation_research::schedule_research` when config gates pass
- [x] Add `obligation_research_config` pass-through from `deps` to the call site
- [x] Gate: `cargo build -p nv-daemon` passes

## Phase 6 — Context Surface

- [x] In `crates/nv-daemon/src/query/mod.rs`, inject research notes into query context when `get_latest_research` returns `Some`
- [x] Ensure the proactive-followups path (followup context builder) also injects research notes when available
- [x] Gate: `cargo build -p nv-daemon` passes

## Phase 7 — Quality Gates

- [x] `cargo build` — all workspace members
- [x] `cargo test -p nv-daemon` — all tests pass (2 pre-existing failures: reminders::parse_tomorrow timing, proactive_watcher::channel_close async)
- [x] `cargo test -p nv-core` — all tests pass (1 pre-existing failure: secrets_from_env when ANTHROPIC_API_KEY is set in test env)
- [x] `cargo clippy -- -D warnings` — no warnings

## Deferred

- [ ] [deferred] Manual /obligations query smoke test (requires live Jira/GitHub config)
- [ ] [deferred] Dashboard visualization of research notes (separate spec)
- [ ] [deferred] Re-research on obligation update (create only for now)
- [ ] [deferred] Telegram notification when research completes
- [ ] [deferred] Manual research trigger from Leo ("research OO-42")
