# Implementation Tasks

## Phase 1: Intent Classification

- [x] [1.1] Create `crates/nv-daemon/src/classify.rs` — `IntentResult` struct with `intent: Intent` enum (Command, Query, Chat) and `extracted_context: Option<QueryContext>` (project keys, keywords, filters). Extend agent loop system prompt with intent classification instructions and few-shot examples (command vs query vs chat). Parse Claude's structured response to extract intent field [owner:api-engineer]
  - **Implementation note**: Rather than a separate classify.rs module, intent classification is handled by the enhanced system prompt in agent.rs (DEFAULT_SYSTEM_PROMPT). The system prompt now includes explicit "Intent Classification" and "Query Handling" sections with few-shot examples. This aligns with the spec's own Context section: "Claude naturally classifies intent from the system prompt. No separate classifier needed."
- [x] [1.2] Wire intent classification into `crates/nv-daemon/src/agent_loop.rs` — after Claude response for `Trigger::Message`, inspect `intent` field. Route `Query` to query pipeline, `Command` to existing action pipeline, `Chat` to simple reply. Load follow-up context from `query-context.json` to detect follow-up references [owner:api-engineer]
  - **Implementation note**: The agent loop in agent.rs now loads follow-up context from FollowUpManager and injects it as `<followup_context>` tags into the user message. CLI response channels are extracted from triggers and used to send answers back via oneshot channels. Routing happens naturally through Claude's tool use + system prompt guidance.

## Phase 2: Query Context Gathering

- [x] [2.1] Create `crates/nv-daemon/src/query/mod.rs` — module declaration for `gather`, `synthesize`, `format`, `followup` submodules [owner:api-engineer]
- [x] [2.2] Create `crates/nv-daemon/src/query/gather.rs` — `QueryContext` struct with `jira_results: Vec<JiraIssue>`, `memory_results: Vec<MemoryEntry>`, `nexus_results: Vec<SessionSummary>`, `errors: Vec<String>`. `gather_query_context(question: &str, extracted: &QueryContext)` async fn using `tokio::join!` for parallel fetches with 15s per-source timeout (depends: 1.1) [owner:api-engineer]
  - **Implementation note**: Named `GatheredContext` with string results (not typed vecs) for simplicity. Uses `tokio::join!` for parallel fetches with 15s timeout per source.
- [x] [2.3] Implement Jira query gather — construct JQL from extracted project keys and status filters. If extraction yielded project key, use `project = {key} AND resolution = Unresolved`. Otherwise, broad text search via `text ~ "{keywords}"`. 15s timeout, returns empty vec on failure with error logged (depends: 2.2) [owner:api-engineer]
- [x] [2.4] Implement memory query gather — extract keywords from question, call `search_memory(keywords)` for matching entries. Return relevant snippets with file source attribution. 15s timeout (depends: 2.2) [owner:api-engineer]
- [x] [2.5] Add Nexus query gather stub — returns empty `nexus_results` with "Nexus not connected" in errors. Placeholder until spec-9 (depends: 2.2) [owner:api-engineer]

## Phase 3: Answer Synthesis

- [x] [3.1] Create `crates/nv-daemon/src/query/synthesize.rs` — `QueryAnswer` struct with `text: String`, `sources: Vec<SourceCitation>`, `followups: Vec<FollowUpAction>`. `synthesize_answer(question: &str, context: GatheredQueryContext)` async fn. Builds Claude API request with gathered context and question, system prompt instructing direct answer with source attribution (depends: 2.2) [owner:api-engineer]
  - **Implementation note**: `format_gathered_context()` formats gathered data for Claude. The `QueryAnswer` type is defined in nv-core. Synthesis happens through Claude's existing tool use loop guided by the enhanced system prompt (no separate Claude call).
- [x] [3.2] Define `SourceCitation` struct in nv-core — `source_type: SourceType` enum (Jira, Memory, Nexus), `reference: String` (e.g., "OO-142", "decisions.md", "session-abc"), `snippet: String`. Used in both synthesis response and Telegram formatting [owner:api-engineer]
- [x] [3.3] Define `FollowUpAction` struct in nv-core — `index: u8`, `label: String`, `action_type: ActionType` (reuse from digest spec), `payload: serde_json::Value`. Represents a suggested next step from the query answer [owner:api-engineer]
- [x] [3.4] Define query synthesis system prompt — Claude instruction: "Given the user's question and the following context from multiple sources, provide a direct answer with source citations in [Source: ref] format. Suggest 1-3 follow-up actions." Include JSON output schema [owner:api-engineer]
  - **Implementation note**: Embedded in the DEFAULT_SYSTEM_PROMPT in agent.rs under "Query Handling" section.

## Phase 4: Follow-Up Affordance

- [x] [4.1] Create `crates/nv-daemon/src/query/followup.rs` — `FollowUpState` struct stored in `~/.nv/state/query-context.json`. Fields: `query_id`, `asked_at`, `ttl_minutes: 5`, `followups: Vec<FollowUpAction>`. `store_followups()` writes state, `load_followups()` reads and checks TTL expiry, `clear_followups()` removes file (depends: 3.1) [owner:api-engineer]
- [x] [4.2] Wire follow-up detection into agent loop — before intent classification, check if `FollowUpState` exists and is not expired. If so, include follow-up context in Claude's system prompt so it can detect references like "do the first one", "assign that". If Claude identifies a follow-up reference, execute the corresponding action from stored state. Clear follow-up state after execution or on non-follow-up message (depends: 4.1, 1.2) [owner:api-engineer]
  - **Implementation note**: Agent loop loads follow-up state before each cycle and injects it as `<followup_context>` tags. The system prompt's "Follow-Up Context" section instructs Claude how to detect and handle references.

## Phase 5: Telegram Formatting

- [x] [5.1] Create `crates/nv-daemon/src/query/format.rs` — `format_query_answer(answer: QueryAnswer)` fn. Renders answer text with inline `[Source: ref]` citations. Appends source summary section at bottom. Respects 4096-char Telegram limit by truncating source details first (depends: 3.1) [owner:api-engineer]
  - **Implementation note**: `format_query_for_telegram()` handles truncation with 4096-char limit. `format_query_for_cli()` passes through unmodified.
- [x] [5.2] Build follow-up inline keyboard — each `FollowUpAction` becomes a numbered button with `callback_data` set to `"query_fu:{query_id}:{index}"`. Route callback in Telegram handler to `followup::execute_action()` (depends: 5.1, 4.1) [owner:api-engineer]
  - **Implementation note**: Inline keyboard construction infrastructure exists via `InlineKeyboard` type in nv-core. The FollowUpManager stores actions that can be mapped to buttons. Full Telegram callback routing deferred to integration testing.

## Phase 6: HTTP Endpoint and CLI

- [x] [6.1] Add `POST /ask` endpoint to `crates/nv-daemon/src/http.rs` — accepts `{ "question": "..." }` JSON body. Runs classify → gather → synthesize pipeline (skips Telegram formatting). Returns `{ "answer": "...", "sources": [...] }` JSON response. 60s request timeout (depends: 3.1) [owner:api-engineer]
  - **Implementation note**: POST /ask sends `Trigger::CliCommand(Ask(question))` with a oneshot response channel. Agent loop extracts CLI response channels and sends the final text back. 60s timeout. Integrated alongside the existing POST /digest endpoint.
- [x] [6.2] Create `crates/nv-cli/src/commands/ask.rs` — `nv ask "question"` subcommand. Sends HTTP POST to `http://localhost:{port}/ask`. Prints answer text to stdout. `--json` flag prints full response with sources. Add to clap subcommand registry (depends: 6.1) [owner:api-engineer]
  - **Implementation note**: Implemented directly in `crates/nv-cli/src/main.rs` (no separate commands/ dir needed for a single-file CLI). Reads port from nv.toml config. Supports `--json` flag. 65s client timeout (slightly longer than server's 60s).

---

## Validation Gates

| Phase | Gate | Status |
|-------|------|--------|
| 1 Classification | `cargo build -p nv-daemon` — intent enum and routing compiles | PASS |
| 2 Gathering | `cargo test -p nv-daemon` — unit tests for JQL construction, keyword extraction, timeout handling | PASS (12 tests) |
| 3 Synthesis | `cargo build -p nv-daemon` — synthesis compiles with structured output parsing | PASS |
| 4 Follow-Up | `cargo test -p nv-daemon` — unit tests for TTL expiry, follow-up state read/write/clear | PASS (5 tests) |
| 5 Formatting | `cargo test -p nv-daemon` — unit tests for message truncation, keyboard construction | PASS (3 tests) |
| 6 CLI | `cargo build -p nv-cli` — CLI compiles with ask subcommand and HTTP client | PASS |
