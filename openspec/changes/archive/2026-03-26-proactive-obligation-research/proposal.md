# Proposal: Proactive Obligation Research

## Change ID
`proactive-obligation-research`

## Summary

When an obligation is created or updated, Nova schedules a background research task using
available tools (Jira, GitHub, calendar, email). Research results are stored as structured
notes on the obligation and surfaced in the next followup message or when Leo queries.
Configurable: which obligation types trigger research, max tools per session.

## Context
- Phase: 4 — Autonomy | Wave: 10
- Depends on: `proactive-followups` (Wave 9) — followup delivery infrastructure required
- New file: `crates/nv-daemon/src/obligation_research.rs`
- Touched files: `crates/nv-daemon/src/orchestrator.rs`, `crates/nv-daemon/src/obligation_store.rs`,
  `crates/nv-daemon/src/worker.rs`, `crates/nv-core/src/config.rs`

## Motivation

Obligation detection (Wave 7) captures commitments but leaves them as bare text fields:
a `detected_action` string, a priority, and an owner. Leo still has to manually investigate
context — what is the Jira status of the referenced ticket? Are there open PRs blocking it?
Is there a calendar event scheduled around the deadline?

Proactive research closes this gap: immediately after an obligation is stored, Nova fires a
low-priority background worker that fetches relevant context from connected tools and stores
it as structured notes. The notes then surface automatically in the next morning briefing or
when Leo asks about that obligation. Nova acts on its own initiative — this is the defining
behavior of Phase 4 Autonomy.

## Design

### New File: `obligation_research.rs`

This module owns the research lifecycle. It is intentionally separate from `obligation_store.rs`
(which owns persistence) and `obligation_detector.rs` (which owns classification).

```rust
/// Result of a proactive research session for one obligation.
pub struct ResearchResult {
    pub obligation_id: String,
    pub summary: String,           // 2-5 sentence prose for briefing
    pub raw_findings: Vec<Finding>,
    pub researched_at: DateTime<Utc>,
    pub tools_used: Vec<String>,
    pub error: Option<String>,     // set when research partially failed
}

pub struct Finding {
    pub tool: String,              // "jira", "github", "calendar", "web"
    pub label: String,             // e.g. "OO-42 status: In Progress"
    pub detail: Option<String>,    // optional longer description
}

/// Entry point called by the orchestrator after obligation creation/update.
///
/// Respects the `ObligationResearchConfig` from daemon config.
/// Returns immediately; spawns a background Tokio task.
pub fn schedule_research(
    obligation: Obligation,
    deps: Arc<SharedDeps>,
    config: ObligationResearchConfig,
);

/// Execute a research session for one obligation.
///
/// Spawns a low-priority `WorkerTask` into the existing pool via
/// `WorkerPool::dispatch`. The task's trigger is a synthetic
/// `Trigger::ObligationResearch` variant (added to `nv-core`).
async fn run_research(
    obligation: Obligation,
    deps: Arc<SharedDeps>,
    config: ObligationResearchConfig,
) -> Result<ResearchResult>;
```

### New Trigger Variant: `Trigger::ObligationResearch`

Add to `nv-core/src/types.rs`:

```rust
pub struct ObligationResearchTrigger {
    pub obligation_id: String,
    pub detected_action: String,
    pub project_code: Option<String>,
    pub source_channel: String,
    pub priority: i32,
}

// Trigger enum gains:
Trigger::ObligationResearch(ObligationResearchTrigger)
```

This keeps the research path inside the standard `WorkerPool` dispatch pipeline. Workers
already handle `Trigger` variants via `Worker::run` — no separate executor needed.

### Research Worker System Prompt Supplement

When a worker handles `Trigger::ObligationResearch`, it uses a focused research prompt
appended to the standard system context:

```
You are researching context for a tracked obligation. Do not respond conversationally.
Obligation: <detected_action>
Project: <project_code if present>
Channel: <source_channel>

Use available tools to gather relevant context:
- If a Jira ticket key is present, fetch its status, assignee, and recent comments.
- If a GitHub repo or PR is referenced, fetch open issues/PRs related to the obligation.
- If a deadline is mentioned, check the calendar for nearby events.
- If the obligation mentions an email thread, search recent email.

Return a JSON object with this shape:
{
  "summary": "<2-5 sentence prose summary of findings>",
  "findings": [
    { "tool": "<tool>", "label": "<short finding>", "detail": "<optional longer text>" }
  ],
  "tools_used": ["jira", "github"]
}

If no relevant context is found, return summary: "No additional context found." with empty findings.
Do not send a message to Leo. Do not use TTS. Just return the JSON.
```

### Storage: `obligation_notes` Table

New SQLite table in `messages.db` (migrated via `rusqlite_migration` at next startup):

```sql
CREATE TABLE IF NOT EXISTS obligation_notes (
    id            TEXT PRIMARY KEY,          -- UUID
    obligation_id TEXT NOT NULL,             -- FK to obligations.id
    summary       TEXT NOT NULL,
    findings_json TEXT NOT NULL DEFAULT '[]', -- JSON array of Finding
    tools_used    TEXT NOT NULL DEFAULT '[]', -- JSON array of strings
    error         TEXT,                       -- NULL on success
    researched_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_obligation_notes_obligation
    ON obligation_notes(obligation_id);
```

New `ObligationStore` methods:
- `save_research_result(result: &ResearchResult) -> Result<()>`
- `get_latest_research(obligation_id: &str) -> Result<Option<ResearchResult>>`
- `list_research_by_obligation(obligation_id: &str) -> Result<Vec<ResearchResult>>`

### Config: `ObligationResearchConfig`

Add to `nv-core/src/config.rs`:

```toml
[obligation_research]
enabled = true
# Obligation channels that trigger research (empty = all channels)
trigger_channels = []
# Max tool calls per research session (caps the worker's MAX_TOOL_LOOP_ITERATIONS)
max_tools = 5
# Minimum priority to trigger research (0 = all, 2 = P2+, 4 = never)
min_priority = 2
# Delay before research fires, in seconds (allows obligation to settle)
research_delay_secs = 10
```

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct ObligationResearchConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub trigger_channels: Vec<String>,
    #[serde(default = "default_max_tools")]
    pub max_tools: usize,          // default: 5
    #[serde(default = "default_min_priority")]
    pub min_priority: i32,         // default: 2 (P2 and above)
    #[serde(default = "default_research_delay_secs")]
    pub research_delay_secs: u64,  // default: 10
}
```

Add `obligation_research: Option<ObligationResearchConfig>` to `Config`.

### Orchestrator Integration

In `orchestrator.rs`, after the existing obligation detection block:

```rust
// After ObligationStore::create succeeds:
if let Some(research_cfg) = &deps.obligation_research_config {
    if research_cfg.enabled
        && obligation.priority <= research_cfg.min_priority
        && (research_cfg.trigger_channels.is_empty()
            || research_cfg.trigger_channels.contains(&obligation.source_channel))
    {
        obligation_research::schedule_research(
            obligation.clone(),
            Arc::clone(&deps),
            research_cfg.clone(),
        );
    }
}
```

`schedule_research` fires `tokio::spawn` with a `tokio::time::sleep(research_delay_secs)`
before dispatching the `WorkerTask`. Priority is always `Priority::Low` — research must not
displace interactive tasks.

### Surface: Followup and Query Integration

The followup mechanism (from `proactive-followups`) and the query path (`query/mod.rs`) gain
one new call:

```rust
// When building followup or query context for an obligation:
if let Some(notes) = obligation_store.get_latest_research(&obligation.id)? {
    context += &format!("\nResearch notes: {}", notes.summary);
    for f in &notes.findings {
        context += &format!("\n  - [{}] {}", f.tool, f.label);
    }
}
```

This means Leo automatically gets pre-fetched context without asking.

### Worker Changes

`Worker::run` gains a branch for `Trigger::ObligationResearch`:

```rust
Trigger::ObligationResearch(research_trigger) => {
    // Build research prompt supplement, inject into system context
    // Execute Claude with reduced MAX_TOOL_LOOP_ITERATIONS = config.max_tools
    // Parse JSON result from Claude response
    // Store via ObligationStore::save_research_result
    // No outbound message sent
}
```

`SharedDeps` gains:
```rust
pub obligation_research_config: Option<ObligationResearchConfig>,
```

## Scope

- **IN**: `obligation_notes` table, schema migration, `ObligationResearchConfig`, new
  trigger variant, `schedule_research` entry point, worker handling branch, notes surface
  in followup/query context
- **OUT**: Manual research trigger from Leo ("research OO-42"), re-research on obligation
  update (only on create), research for all obligation priorities (respects `min_priority`),
  dashboard visualization of research notes, Telegram notification when research completes

## Impact

| File | Change |
|------|--------|
| `crates/nv-daemon/src/obligation_research.rs` | New module — all research lifecycle logic |
| `crates/nv-daemon/src/obligation_store.rs` | `obligation_notes` migration + 3 new methods |
| `crates/nv-daemon/src/orchestrator.rs` | Trigger research after obligation create |
| `crates/nv-daemon/src/worker.rs` | `SharedDeps.obligation_research_config` + `ObligationResearch` branch |
| `crates/nv-core/src/types.rs` | `Trigger::ObligationResearch` + `ObligationResearchTrigger` |
| `crates/nv-core/src/config.rs` | `ObligationResearchConfig` + `Config.obligation_research` |
| `crates/nv-daemon/src/query/mod.rs` | Inject research notes into query context |

## Risks

| Risk | Mitigation |
|------|-----------|
| Research worker blocks interactive tasks | `Priority::Low` ensures interactive tasks always displace it in the queue |
| Claude returns malformed JSON from research prompt | Parse with `serde_json::from_str`; on error, store `error` field and `summary = "Research failed: ..."` |
| Jira/GitHub not configured → empty findings | Guard with `if deps.jira_registry.is_some()` etc. before including tool instructions |
| `research_delay_secs` allows obligation to be dismissed before research fires | Check obligation status before dispatching; skip if `status != Open` |
| `obligation_notes` table grows unbounded | Nightly cron prunes rows older than 30 days (piggyback on existing cleanup cron) |

## Verification

- `cargo build` passes for all workspace members
- `cargo test -p nv-daemon` — obligation_research module tests pass:
  - `schedule_research` respects `min_priority` gate
  - `schedule_research` respects `trigger_channels` filter
  - `save_research_result` and `get_latest_research` round-trip correctly
  - Worker branch parses valid and malformed JSON without panic
- `cargo clippy -- -D warnings` passes
- [user] Manual test: send a Telegram message with a Jira ticket reference containing an
  obligation; within ~15s a research note should be stored; next `/obligations` query
  should show the research summary inline
