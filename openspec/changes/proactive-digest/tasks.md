# Implementation Tasks

## Phase 1: Cron Scheduler

- [x] [1.1] Create `crates/nv-daemon/src/scheduler.rs` — tokio task with `tokio::time::interval(Duration::from_secs(config.agent.digest_interval_minutes * 60))`, pushes `Trigger::Cron(CronEvent::Digest)` to `mpsc::Sender<Trigger>`, minimum interval floor of 5 minutes, reads `last-digest.json` on startup to calculate initial delay [owner:api-engineer]
- [x] [1.2] Add `CronEvent` enum to `crates/nv-core/src/types.rs` — `CronEvent::Digest` variant (with optional `force: bool` field to bypass dedup), extend `Trigger::Cron(CronEvent)` if not already parameterized [owner:api-engineer]
- [x] [1.3] Wire scheduler into `crates/nv-daemon/src/main.rs` — spawn scheduler task with cloned `mpsc::Sender<Trigger>` and config reference, alongside existing channel listener spawns [owner:api-engineer]

## Phase 2: Digest State

- [x] [2.1] Create `crates/nv-daemon/src/digest/state.rs` — `DigestState` struct with `last_sent_at: Option<DateTime<Utc>>`, `content_hash: Option<String>`, `suggested_actions: Vec<SuggestedAction>`, `sources_status: HashMap<String, String>`. Read/write to `~/.nv/state/last-digest.json`. SHA-256 content hash for dedup. `should_send()` method checks interval + hash (depends: 1.2) [owner:api-engineer]
- [x] [2.2] Create `crates/nv-daemon/src/digest/mod.rs` — module declaration for `state`, `gather`, `synthesize`, `format`, `actions` submodules [owner:api-engineer]

## Phase 3: Context Gathering

- [x] [3.1] Create `crates/nv-daemon/src/digest/gather.rs` — `DigestContext` struct containing `jira_issues: Vec<JiraIssue>`, `nexus_sessions: Vec<SessionSummary>`, `memory_entries: Vec<MemoryEntry>`, `errors: Vec<String>`. `gather_context()` async fn using `tokio::join!` for parallel fetches (depends: 2.1) [owner:api-engineer]
- [x] [3.2] Implement Jira gather — `jira_search("assignee = currentUser() AND resolution = Unresolved ORDER BY priority ASC, updated DESC")` with 30s timeout, returns issues grouped by project. On failure, pushes "Jira unavailable" to errors vec, returns empty issues (depends: 3.1) [owner:api-engineer]
- [x] [3.3] Implement memory gather — `search_memory("*")` filtered to entries since `last_sent_at` from `DigestState`. Returns recent decisions, tasks, conversation summaries. On failure, pushes "Memory unavailable" to errors (depends: 3.1) [owner:api-engineer]
- [x] [3.4] Add Nexus gather stub — returns `nexus_sessions: vec![]` with "Nexus not connected" in errors vec. Placeholder until spec-9 provides `NexusClient` (depends: 3.1) [owner:api-engineer]

## Phase 4: Digest Synthesis

- [x] [4.1] Create `crates/nv-daemon/src/digest/synthesize.rs` — `synthesize_digest(context: DigestContext)` async fn. Builds Claude API request with system prompt defining digest format: sections for Jira (grouped by priority, staleness warnings >3d), Sessions (placeholder), Memory (recent entries), Suggested Actions (3-5 items). Returns `DigestResult` with `sections: Vec<DigestSection>` and `suggested_actions: Vec<SuggestedAction>` (depends: 3.1) [owner:api-engineer]
- [x] [4.2] Define `SuggestedAction` struct in nv-core — `id: String`, `label: String`, `action_type: ActionType` enum (JiraTransition, MemoryWrite, FollowUpQuery), `payload: serde_json::Value`. Serializable for both Claude response parsing and Telegram callback data [owner:api-engineer]
- [x] [4.3] Define digest system prompt — Claude instruction template: "You are NV generating a periodic digest. Given the following context, produce a structured summary..." with JSON output schema, section ordering rules, and action suggestion guidelines [owner:api-engineer]

## Phase 5: Telegram Formatting

- [x] [5.1] Create `crates/nv-daemon/src/digest/format.rs` — `format_digest(result: DigestResult)` fn. Converts sections to Telegram-compatible text with section headers, bullet items, priority indicators for P0/P1 items. Respects 4096-char Telegram limit by truncating lower-priority items first (depends: 4.1) [owner:api-engineer]
- [x] [5.2] Build inline keyboard for suggested actions — each `SuggestedAction` becomes an `InlineKeyboardButton` with `callback_data` set to `"digest_act:{action_id}"`. Add "Dismiss All" button as final row. Limit to 5 action buttons (Telegram max 8 per message) (depends: 5.1) [owner:api-engineer]
- [x] [5.3] Send formatted digest to Telegram — call `telegram.send_message(OutboundMessage { content: formatted_text, keyboard: Some(action_keyboard), .. })` targeting `config.telegram.chat_id` (depends: 5.2) [owner:api-engineer]

## Phase 6: Action Execution

- [x] [6.1] Create `crates/nv-daemon/src/digest/actions.rs` — `handle_digest_action(action_id: &str)` async fn. Loads current `DigestState`, finds matching `SuggestedAction` by id, executes based on `action_type` (Jira transition via JiraClient, memory write via memory system, follow-up query by pushing new Trigger). Updates action status to "completed" in state file. Sends confirmation message to Telegram (depends: 2.1, 5.2) [owner:api-engineer]
- [x] [6.2] Wire callback routing — in Telegram callback_query handler (spec-3), match `callback_data` prefix `"digest_act:"` and route to `handle_digest_action`. Match `"digest_dismiss"` to mark all actions dismissed and send acknowledgment (depends: 6.1) [owner:api-engineer]

## Phase 7: Agent Loop Integration

- [x] [7.1] Add digest trigger handling to `crates/nv-daemon/src/agent_loop.rs` — match `Trigger::Cron(CronEvent::Digest)` in the main loop. Check `DigestState::should_send()` (unless `force: true`). Call gather -> synthesize -> format -> send pipeline. Update state after send (depends: 4.1, 5.3, 6.1) [owner:api-engineer]
- [x] [7.2] Add `POST /digest` endpoint to `crates/nv-daemon/src/http.rs` — handler pushes `Trigger::Cron(CronEvent::Digest { force: true })` to mpsc sender. Returns 202 Accepted immediately. Used by `nv digest --now` CLI (depends: 7.1) [owner:api-engineer]

## Phase 8: CLI Command

- [x] [8.1] Create `crates/nv-cli/src/commands/digest.rs` — `nv digest --now` subcommand. Sends HTTP POST to `http://localhost:{config.daemon.port}/digest`. Prints "Digest triggered" on 202, error message on failure. Add to clap subcommand registry (depends: 7.2) [owner:api-engineer]

---

## Validation Gates

| Phase | Gate |
|-------|------|
| 1 Scheduler | `cargo build -p nv-daemon` -- scheduler compiles with correct types |
| 2 State | `cargo test -p nv-daemon` -- unit tests for state read/write, dedup logic, interval check |
| 3 Gathering | `cargo test -p nv-daemon` -- unit tests for gather with mock Jira/memory responses, timeout handling |
| 4 Synthesis | `cargo build -p nv-daemon` -- synthesis compiles, prompt template renders |
| 5 Formatting | `cargo test -p nv-daemon` -- unit tests for message truncation at 4096 chars, keyboard construction |
| 6 Actions | `cargo build -p nv-daemon` -- action handler compiles with callback routing |
| 7 Integration | `cargo build -p nv-daemon` -- full digest pipeline wired in agent loop |
| 8 CLI | Manual: `nv digest --now` -> digest arrives on Telegram with Jira data + action buttons |
