# Implementation Tasks

<!-- beads:epic:nv-9lu5 -->

## Core Types

- [ ] [1.1] [P-1] Add `CronEvent::WeeklySelfAssessment` variant to `CronEvent` enum in `crates/nv-core/src/types.rs` [owner:api-engineer]
- [ ] [1.2] [P-1] Define `LatencyTrend` enum (Improving, Stable, Degrading) in `crates/nv-daemon/src/self_assessment.rs` [owner:api-engineer]
- [ ] [1.3] [P-1] Define `LatencyAssessment` struct (p50_ms, p95_ms, p99_ms, sample_count, trend, prior_p95_ms) [owner:api-engineer]
- [ ] [1.4] [P-1] Define `ErrorAssessment` struct (total_invocations, error_rate_pct, top_failing_tools: Vec<(String, i64)>) [owner:api-engineer]
- [ ] [1.5] [P-1] Define `ToolUsageAssessment` struct (total_invocations, most_used: Vec<(String, i64)>, slowest: Vec<(String, f64)>) [owner:api-engineer]
- [ ] [1.6] [P-1] Define `FailurePattern` struct (pattern, occurrences, last_seen, example) [owner:api-engineer]
- [ ] [1.7] [P-1] Define `SelfAssessmentEntry` struct (id, generated_at, window_days, latency, errors, tool_usage, failure_patterns, suggestions: Vec<String>) with Serialize/Deserialize [owner:api-engineer]

## SelfAssessmentStore

- [ ] [2.1] [P-1] Implement `SelfAssessmentStore::new(path: &Path) -> Result<Self>` — init JSONL file at `~/.nv/state/self-assessment.jsonl`, create parent dirs if missing [owner:api-engineer]
- [ ] [2.2] [P-1] Implement `SelfAssessmentStore::append(entry: &SelfAssessmentEntry) -> Result<()>` — serialize to JSON line, append to file, prune to 52 entries if over cap [owner:api-engineer]
- [ ] [2.3] [P-2] Implement `SelfAssessmentStore::get_recent(limit: usize) -> Result<Vec<SelfAssessmentEntry>>` — read all lines, deserialize, return newest-first up to limit [owner:api-engineer]
- [ ] [2.4] [P-2] Implement `SelfAssessmentStore::get_latest() -> Result<Option<SelfAssessmentEntry>>` — returns the most recent entry (delegates to get_recent(1)) [owner:api-engineer]

## SelfAssessmentEngine

- [ ] [3.1] [P-1] Define `SelfAssessmentEngine` struct holding `Arc<Mutex<ColdStartStore>>`, `Arc<Mutex<MessageStore>>`, `diary_base: PathBuf` [owner:api-engineer]
- [ ] [3.2] [P-1] Implement latency analysis in `analyze()` — call `cold_start_store.get_percentiles(window_days * 24)`, compute LatencyTrend vs prior week p95 (1.2x degrading / 0.8x improving threshold) [owner:api-engineer]
- [ ] [3.3] [P-2] Implement error analysis in `analyze()` — call `message_store.get_tool_stats()`, compute error_rate_pct = (total - success) / total * 100, extract top 3 failing tools [owner:api-engineer]
- [ ] [3.4] [P-2] Implement tool usage analysis in `analyze()` — extract top 5 by invocation count, top 3 by avg_duration_ms from ToolStatsReport [owner:api-engineer]
- [ ] [3.5] [P-2] Implement diary scan in `analyze()` — list diary `.md` files from last window_days days in diary_base, read each (skip files > 1MB), scan `**Result:**` lines for failure signal phrases (failed, error, timeout, unavailable, retry, auth, 403, 401), group by normalized pattern, return top 3 by occurrence [owner:api-engineer]
- [ ] [3.6] [P-2] Implement suggestion generation in `analyze()` — deterministic rules: p95 > 30000ms, error_rate > 10%, tool avg_duration > 5000ms, insufficient data (< 7 events); produce up to 3 suggestion strings [owner:api-engineer]
- [ ] [3.7] [P-3] Wire `SelfAssessmentEngine::analyze()` to call store's `get_latest()` for prior week p95 comparison [owner:api-engineer]

## Scheduler Integration

- [ ] [4.1] [P-1] Add weekly self-assessment poll interval and `last_assessment_date: Option<chrono::NaiveDate>` tracking in `spawn_scheduler()` in `scheduler.rs` [owner:api-engineer]
- [ ] [4.2] [P-1] Add weekly fire condition to scheduler loop: Sunday (weekday == 0), hour 0–1, once per day via `last_assessment_date` guard; emit `Trigger::Cron(CronEvent::WeeklySelfAssessment)` [owner:api-engineer]

## Orchestrator Dispatch

- [ ] [5.1] [P-1] Handle `CronEvent::WeeklySelfAssessment` in `classify_trigger()` — return `TriggerClass::Digest` [owner:api-engineer]
- [ ] [5.2] [P-1] Dispatch `WeeklySelfAssessment` to a worker with action `"weekly_self_assessment"` in the orchestrator's cron branch [owner:api-engineer]
- [ ] [5.3] [P-2] Extend `/status` bot command handler in `orchestrator.rs` to call `SelfAssessmentStore::get_latest()` and append a "Self-Assessment" section (generated_at relative, latency trend, error rate, top suggestion; "No assessment yet. Will run Sunday." if None) [owner:api-engineer]

## Tool Registration

- [ ] [6.1] [P-1] Register `self_assessment_run` tool (no parameters) in `crates/nv-daemon/src/tools/mod.rs` — description: "Run a weekly self-assessment analyzing Nova's performance over the past 7 days. Returns a summary report." [owner:api-engineer]
- [ ] [6.2] [P-1] Implement `self_assessment_run` dispatch: call `SelfAssessmentEngine::analyze(7)`, append to `SelfAssessmentStore`, return formatted text summary (latency, errors, tool usage, failure patterns, suggestions) [owner:api-engineer]

## Morning Briefing Integration

- [ ] [7.1] [P-3] In `digest/gather.rs` (or `digest/synthesize.rs`), call `SelfAssessmentStore::get_latest()` after gather phase [owner:api-engineer]
- [ ] [7.2] [P-3] If latest entry exists and `generated_at > now - 7 days`, inject `[Self-Assessment]` section into briefing context: top 3 suggestions + latency trend + error rate [owner:api-engineer]

## Wiring

- [ ] [8.1] [P-1] Add `mod self_assessment;` declaration in `crates/nv-daemon/src/lib.rs` or `main.rs` [owner:api-engineer]
- [ ] [8.2] [P-1] Init `SelfAssessmentStore` and `SelfAssessmentEngine` in `main.rs`; pass engine via `SharedDeps` or as Arc to tool dispatch [owner:api-engineer]
- [ ] [8.3] [P-2] Pass `SelfAssessmentStore` (Arc<Mutex<>>) to orchestrator for `/status` handler [owner:api-engineer]

## Verify

- [ ] [9.1] cargo build passes [owner:api-engineer]
- [ ] [9.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [ ] [9.3] cargo test — SelfAssessmentStore: append + get_recent + get_latest + cap pruning at 52 entries [owner:api-engineer]
- [ ] [9.4] cargo test — SelfAssessmentEngine: analyze with mocked stores (sufficient data, insufficient data, p95 regression, high error rate) [owner:api-engineer]
- [ ] [9.5] cargo test — LatencyTrend computation: degrading (current > prior * 1.2), improving (current < prior * 0.8), stable (in range) [owner:api-engineer]
- [ ] [9.6] cargo test — diary scan: given a temp diary file with known failure phrases, verify patterns extracted correctly [owner:api-engineer]
- [ ] [9.7] cargo test — scheduler: WeeklySelfAssessment fires on Sunday hour 0, does not fire on non-Sunday, does not fire twice on same day [owner:api-engineer]
- [ ] [9.8] cargo test — suggestion generation: each rule triggers correct suggestion string [owner:api-engineer]
- [ ] [9.9] [user] Manual test: trigger `self_assessment_run` tool via Telegram, verify JSONL appended to `~/.nv/state/self-assessment.jsonl` [owner:api-engineer]
- [ ] [9.10] [user] Manual test: run `/status` via Telegram, verify self-assessment section appears in reply [owner:api-engineer]
- [ ] [9.11] [user] Manual test: wait for Sunday 00:00 (or manually emit trigger), verify morning briefing includes self-assessment section [owner:api-engineer]
