# Implement Cross-System Context Queries

| Field | Value |
|-------|-------|
| Spec | `context-query` |
| Priority | P2 |
| Type | feature |
| Effort | small |
| Wave | 4 |

## Context

NV must answer cross-system questions like "What's blocking OO?" by searching Jira, memory, and Nexus in parallel and synthesizing a unified answer with source attribution. This is the Querier interaction mode (PRD 6.3) — distinct from commands (which mutate state) and chat (which is conversational). The agent loop already classifies intent via Claude (spec-4), but query handling requires a dedicated pipeline: parallel data gathering, answer synthesis with citations, and follow-up affordance so a query answer can trigger a command ("assign that to me").

The key design challenge is intent classification — Claude must distinguish between a command ("create a bug for OO"), a query ("what's blocking OO?"), and casual chat ("thanks NV"). Commands route to the existing action pipeline with confirmation. Queries route to the parallel gather + synthesize pipeline described here. Chat gets a simple conversational reply. This classification happens in the agent loop via the system prompt, not a separate classifier model.

The CLI command `nv ask "question"` sends the question to the daemon via HTTP and streams the answer back to stdout, providing the same query capability outside of Telegram.

## User Stories

- **Querier (PRD 6.3)**: Leo asks "What's blocking the OO release?" → NV queries Jira + Nexus + memory → returns cross-system answer with sources
- **Success Criteria #3 (PRD 16)**: Ask "What's blocking X?" and get a cross-system answer
- **Ambiguity Resolution (PRD 17)**: "How does NV distinguish command vs query vs chat?" → Claude classifies intent from message content

## Proposed Changes

### Intent Classification

- `crates/nv-daemon/src/classify.rs`: Intent classifier — extends the agent loop's Claude system prompt with explicit intent classification instructions. Claude returns a structured response with `intent: "command" | "query" | "chat"` as the first field. The agent loop inspects this field to route:
  - `command` → existing action pipeline (draft + confirm via Telegram)
  - `query` → query pipeline (gather → synthesize → respond)
  - `chat` → simple conversational reply
  - Classification is part of the main Claude call, not a separate preflight request (saves latency and tokens). The system prompt includes few-shot examples:
    - "Create a P1 bug for checkout crash" → command
    - "What's blocking OO?" → query
    - "Thanks NV" → chat
    - "How many open issues on TC?" → query
    - "Assign OO-142 to me" → command

### Parallel Data Gathering

- `crates/nv-daemon/src/query/gather.rs`: Query-specific context gathering — given the classified query, spawns parallel fetches via `tokio::join!`:
  1. **Jira**: Intelligent JQL construction — Claude extracts project keys, issue types, and status filters from the question. Falls back to broad search if extraction fails. Example: "What's blocking OO?" → `project = OO AND status != Done ORDER BY priority ASC`
  2. **Memory**: `search_memory(query_keywords)` — keyword extraction from the question, grep across memory files for relevant context
  3. **Nexus**: `query_sessions()` filtered to relevant project — session status, errors, recent completions (stub until spec-9)
  - Each fetch has a 15-second timeout (shorter than digest — queries should feel responsive). Partial results accepted.

### Answer Synthesis

- `crates/nv-daemon/src/query/synthesize.rs`: Claude answer synthesis — second Claude call (or single call if gathering is embedded as tool use) that takes the gathered context and the original question, produces:
  - **Answer text**: Direct, concise answer to the question
  - **Source attribution**: Each claim tagged with its source (`[Jira: OO-142]`, `[Memory: decisions.md]`, `[Nexus: session-abc]`)
  - **Follow-up suggestions**: 1-3 related actions the user might want to take based on the answer (e.g., "Transition OO-142 to In Progress", "Check session logs")
  - The system prompt instructs Claude to be direct and factual, not hedging. If data is missing, say so explicitly rather than speculating.

### Follow-Up Affordance

- `crates/nv-daemon/src/query/followup.rs`: Follow-up state tracking — after sending a query answer to Telegram, stores the follow-up suggestions in `~/.nv/state/query-context.json` with a TTL (5 minutes). If Leo's next message references a follow-up ("assign that to me", "do the first one"), the agent loop detects the reference via Claude and executes the corresponding action from the stored context. After TTL expires or a non-follow-up message arrives, the context is cleared.
  ```json
  {
    "query_id": "q_abc123",
    "asked_at": "2026-03-21T10:30:00Z",
    "ttl_minutes": 5,
    "followups": [
      { "index": 1, "label": "Transition OO-142 to In Progress", "action": {...} },
      { "index": 2, "label": "View session logs", "action": {...} }
    ]
  }
  ```

### Telegram Response Formatting

- `crates/nv-daemon/src/query/format.rs`: Query answer formatter — converts the synthesized answer into a Telegram message with:
  - Answer text with inline source citations
  - Follow-up actions as inline keyboard buttons (numbered to match suggestions)
  - "Ask follow-up" button that prompts for a related question
  - Respects 4096-char Telegram limit

### CLI Query Endpoint

- `crates/nv-daemon/src/http.rs`: Add `POST /ask` endpoint — accepts JSON body `{ "question": "..." }`. Runs the same query pipeline (classify → gather → synthesize). Returns the answer text as plain text response (no Telegram formatting). Used by `nv ask`.
- `crates/nv-cli/src/commands/ask.rs`: `nv ask "question"` subcommand — sends HTTP POST to `http://localhost:{port}/ask` with the question. Prints the answer to stdout. Supports `--json` flag for structured output with sources.

### Agent Loop Integration

- `crates/nv-daemon/src/agent_loop.rs`: Extend message handling — after Claude classifies intent as "query", route to query pipeline instead of action pipeline. Load follow-up context from `query-context.json` to check if current message is a follow-up reference.

## Dependencies

- `agent-loop` (spec-4) — agent loop with Claude API integration and intent classification
- `memory-system` (spec-5) — `search_memory` for context retrieval
- `jira-integration` (spec-6) — `jira_search` for issue queries

## Out of Scope

- Nexus session data in query results (placeholder until spec-9)
- Streaming responses to Telegram (Telegram Bot API doesn't support message streaming)
- Query history or analytics (no tracking of past queries)
- Natural language JQL composition beyond simple project/status extraction (Claude does best-effort)
- Multi-turn conversation state beyond the 5-minute follow-up window
