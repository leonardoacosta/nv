# Proposal: Self-Improvement Research

## Change ID
`self-improvement-research`

## Summary

Weekly scheduled task that reads Nova's own operational data (cold-start latency, message
error patterns, diary failure modes), compiles a structured self-assessment report appended
to `~/.nv/state/self-assessment.jsonl`, and surfaces the top findings in the morning briefing
digest and via `/status` command.

## Context
- New file: `crates/nv-daemon/src/self_assessment.rs` — SelfAssessmentStore + analysis engine
- Touches: `crates/nv-daemon/src/scheduler.rs` — add weekly CronEvent variant + poll loop
- Touches: `crates/nv-core/src/types.rs` — add `CronEvent::WeeklySelfAssessment` variant
- Touches: `crates/nv-daemon/src/orchestrator.rs` — dispatch WeeklySelfAssessment trigger
- Touches: `crates/nv-daemon/src/messages.rs` — expose tool stats query surface (if not already)
- Reads: `ColdStartStore` (latency), `MessageStore` (tool stats, error rates), `DiaryWriter` files (failure patterns)
- PRD ref: Phase 4 — Autonomy, Wave 10, Meta-cognition

## Motivation

Nova currently has no awareness of its own performance over time. Cold-start regressions,
repeated tool failures, and recurring error patterns go unnoticed unless Leo manually inspects
logs. A weekly self-assessment gives Nova:

1. **Latency awareness** — spot P95 regressions before they degrade UX
2. **Error pattern recognition** — surface tools that fail repeatedly so they can be fixed
3. **Failure mode learning** — parse diary for repeated failure phrases and classify them
4. **Suggested fixes** — translate findings into concrete configuration suggestions

The report is low-cost (reads local SQLite + markdown files), fires weekly (Sunday midnight),
and produces a compact summary. No new dependencies required.

## Requirements

### Req-1: SelfAssessmentEntry Type

New `SelfAssessmentEntry` struct (serializable to JSON):

```rust
pub struct SelfAssessmentEntry {
    pub id: String,                            // UUID v4
    pub generated_at: DateTime<Utc>,
    pub window_days: u32,                      // analysis window (default: 7)
    pub latency: LatencyAssessment,
    pub errors: ErrorAssessment,
    pub tool_usage: ToolUsageAssessment,
    pub failure_patterns: Vec<FailurePattern>,
    pub suggestions: Vec<String>,              // top 3 actionable config suggestions
}
```

Sub-structs:

```rust
pub struct LatencyAssessment {
    pub p50_ms: u64,
    pub p95_ms: u64,
    pub p99_ms: u64,
    pub sample_count: usize,
    pub trend: LatencyTrend,   // Improving | Stable | Degrading
    pub prior_p95_ms: Option<u64>,  // previous week's p95 for comparison
}

pub enum LatencyTrend { Improving, Stable, Degrading }

pub struct ErrorAssessment {
    pub total_invocations: i64,
    pub error_rate_pct: f64,         // failed / total * 100
    pub top_failing_tools: Vec<(String, i64)>,  // (tool_name, fail_count) top 3
}

pub struct ToolUsageAssessment {
    pub total_invocations: i64,
    pub most_used: Vec<(String, i64)>,   // top 5 by call count
    pub slowest: Vec<(String, f64)>,     // top 3 by avg duration_ms
}

pub struct FailurePattern {
    pub pattern: String,       // short label (e.g. "jira_auth_failure")
    pub occurrences: usize,
    pub last_seen: String,     // ISO 8601 date
    pub example: String,       // excerpt from diary
}
```

### Req-2: SelfAssessmentStore

New `SelfAssessmentStore` in `self_assessment.rs`:

- Stores entries as JSONL in `~/.nv/state/self-assessment.jsonl`
- `append(entry: &SelfAssessmentEntry) -> Result<()>` — appends one JSON line
- `get_recent(limit: usize) -> Result<Vec<SelfAssessmentEntry>>` — reads tail of file, newest-first
- `get_latest() -> Result<Option<SelfAssessmentEntry>>` — returns the most recent entry
- Cap at 52 entries (1 year of weekly reports); prune oldest on append when over cap

### Req-3: Analysis Engine

`SelfAssessmentEngine` in `self_assessment.rs`:

```rust
pub struct SelfAssessmentEngine {
    cold_start_store: Arc<Mutex<ColdStartStore>>,
    message_store: Arc<Mutex<MessageStore>>,
    diary_base: PathBuf,   // ~/.nv/diary/
}
```

Method: `pub fn analyze(&self, window_days: u32) -> Result<SelfAssessmentEntry>`

Steps:

1. **Latency analysis** — call `cold_start_store.get_percentiles(window_days * 24)`. Compare
   p95 to the prior week's p95 (fetched from the previous assessment entry in the store) to
   compute `LatencyTrend`. Degrading if current p95 > prior p95 * 1.2, improving if current
   p95 < prior p95 * 0.8, stable otherwise.

2. **Error analysis** — call `message_store.get_tool_stats()` (or equivalent). Compute error
   rate from `(total - success_count) / total`. Extract top 3 failing tools by fail count.

3. **Tool usage analysis** — top 5 by invocation count, top 3 by avg duration from
   `ToolStatsReport`.

4. **Diary scan** — read diary `.md` files from the last `window_days` days in `diary_base`.
   Scan for known failure signal phrases (configurable list, hardcoded initially):
   - `"failed"`, `"error"`, `"timeout"`, `"unavailable"`, `"retry"`, `"auth"`, `"403"`, `"401"`
   Parse surrounding context (the `**Result:**` line of the diary entry) to extract the failure
   mode. Group by normalized pattern. Return top 3 by occurrence count.

5. **Suggestion generation** — deterministic rule-based suggestions (no Claude call):
   - P95 > 30s → "Consider increasing Claude timeout or reducing context size"
   - error_rate > 10% → "Tool `{top_failing}` is failing frequently — check credentials/config"
   - Any tool avg_duration > 5000ms → "Tool `{slow_tool}` is slow — consider caching or timeout reduction"
   - If no data → "Insufficient data for assessment (< 7 events in window)"

### Req-4: Scheduler Integration

Add `CronEvent::WeeklySelfAssessment` to `nv_core::types::CronEvent`.

In `scheduler.rs`, add a weekly poll loop (alongside the existing morning briefing poll):

- Poll interval: 60 seconds (same as morning briefing)
- Fire condition: Sunday, any hour between 00:00–01:00 local time (once per week per day)
- Track `last_assessment_date: Option<chrono::NaiveDate>` to prevent duplicate fires on restart
- Emit `Trigger::Cron(CronEvent::WeeklySelfAssessment)`

### Req-5: Orchestrator Dispatch

In `orchestrator.rs`, handle `CronEvent::WeeklySelfAssessment`:

- Classify as `TriggerClass::Digest` (reuses existing priority/routing)
- Spawn a worker with action string `"weekly_self_assessment"`
- The worker's Claude context includes: "Run a weekly self-assessment. Use the
  `self_assessment_run` tool to generate and store this week's report, then summarize findings."

### Req-6: self_assessment_run Tool

Register a new tool `self_assessment_run` (no parameters):

- Calls `SelfAssessmentEngine::analyze(7)` internally
- Appends result to `SelfAssessmentStore`
- Returns formatted text summary of the assessment for Claude to include in its reply

Tool output format:
```
Self-Assessment (week of YYYY-MM-DD):
  Latency: P50=Xms P95=Xms P99=Xms (N samples) — Stable/Improving/Degrading
  Errors: X% error rate, top failing: tool_a (N), tool_b (N)
  Top tools: tool_a (N calls), tool_b (N calls), ...
  Failure patterns: pattern_a (N), pattern_b (N)
  Suggestions:
    1. ...
    2. ...
    3. ...
```

### Req-7: Morning Briefing Integration

In the morning briefing gather phase (`digest/gather.rs` or `digest/synthesize.rs`), include
the latest self-assessment entry if it was generated within the last 7 days:

- Call `SelfAssessmentStore::get_latest()`
- If entry exists and `generated_at > now - 7 days`: inject a `[Self-Assessment]` section
  into the briefing context
- Format: top 3 suggestions + latency trend + error rate

### Req-8: /status Command Extension

In `orchestrator.rs`, extend the `/status` bot command handler to include self-assessment data:

- Call `SelfAssessmentStore::get_latest()`
- Append a "Self-Assessment" section to the existing `/status` reply
- Show: generated_at (relative), latency trend, error rate, top suggestion
- If no assessment yet: "No self-assessment run yet. Will run Sunday."

## Scope
- **IN**: SelfAssessmentEntry types, SelfAssessmentStore (JSONL), SelfAssessmentEngine (analysis),
  WeeklySelfAssessment CronEvent, scheduler weekly poll, orchestrator dispatch, `self_assessment_run`
  tool, morning briefing injection, /status extension
- **OUT**: Claude-generated suggestions (rule-based only), UI dashboard, historical trend graphs,
  cross-daemon comparisons, email/push notifications for regressions

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/self_assessment.rs` | New: SelfAssessmentEntry, SelfAssessmentStore, SelfAssessmentEngine |
| `crates/nv-core/src/types.rs` | Add `CronEvent::WeeklySelfAssessment` variant |
| `crates/nv-daemon/src/scheduler.rs` | Add weekly poll loop, emit WeeklySelfAssessment trigger |
| `crates/nv-daemon/src/orchestrator.rs` | Dispatch WeeklySelfAssessment, extend /status handler |
| `crates/nv-daemon/src/digest/gather.rs` | Inject latest self-assessment into morning briefing context |
| `crates/nv-daemon/src/main.rs` | Init SelfAssessmentEngine + Store, wire into SharedDeps |
| `crates/nv-daemon/src/tools/mod.rs` | Register `self_assessment_run` tool |

## Risks
| Risk | Mitigation |
|------|-----------|
| Diary scan is slow on large files | Limit to last 7 days of diary files; skip files > 1MB |
| SQLite read contention | Use existing Arc<Mutex<>> pattern; reads are fast |
| Suggestion quality low (rule-based) | Rules are conservative and clearly labeled as suggestions, not commands |
| Weekly fire missed on restart | `last_assessment_date` check fires on next Sunday even if daemon was offline |
| JSONL grows unbounded | Hard cap at 52 entries with oldest-first pruning on append |
