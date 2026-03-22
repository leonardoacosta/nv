# Implement Proactive Digest

| Field | Value |
|-------|-------|
| Spec | `proactive-digest` |
| Priority | P2 |
| Type | feature |
| Effort | medium |
| Wave | 4 |

## Context

NV's value proposition extends beyond reactive command handling — it must proactively surface what matters without Leo checking 6+ apps. The proactive digest is a cron-triggered synthesis that gathers context from Jira, Nexus, and memory, then delivers a formatted summary to Telegram with inline action buttons.

The daemon already has the agent loop (spec-4) processing `Trigger` variants from an `mpsc` channel, the memory system (spec-5) for context retrieval, and the Jira client (spec-6) for issue queries. This spec adds the cron scheduler that periodically pushes `Trigger::Cron(Digest)` into the same channel, the gathering logic that pulls data from all sources in parallel, the Claude prompt that synthesizes a structured digest, and the Telegram message formatting with inline keyboard buttons for suggested actions.

The digest interval is configurable via `config.agent.digest_interval_minutes` (default: 60 minutes). State is persisted in `~/.nv/state/last-digest.json` to prevent duplicate digests on restart and to track which suggested actions were acted upon. The CLI command `nv digest --now` triggers an immediate digest via HTTP POST to the daemon.

## User Stories

- **Consumer (PRD 6.2)**: Leo receives a morning digest without checking 6 apps — 3 Jira issues need attention, 1 session running, 2 memory items flagged
- **Success Criteria #2 (PRD 16)**: Receive a morning digest without checking 6 apps
- **Success Criteria #4 (PRD 16)**: See session status from Nexus in the digest

## Proposed Changes

### Cron Scheduler

- `crates/nv-daemon/src/scheduler.rs`: Cron scheduler task — spawns a tokio task with `tokio::time::interval` at `config.agent.digest_interval_minutes`. On each tick, pushes `Trigger::Cron(CronEvent::Digest)` to the shared `mpsc::Sender<Trigger>`. Respects a minimum interval of 5 minutes to prevent runaway loops. Loads initial delay from `last-digest.json` to avoid immediate fire on restart if a digest was sent recently.

### Digest Context Gathering

- `crates/nv-daemon/src/digest/gather.rs`: Parallel context gathering — on digest trigger, spawns parallel `tokio::join!` fetches:
  1. **Jira**: `jira_search("assignee = currentUser() AND resolution = Unresolved ORDER BY priority ASC, updated DESC")` — open issues assigned to Leo, grouped by project and priority
  2. **Nexus sessions**: `query_sessions()` — active/recent sessions across configured agents (homelab, macbook), with status and duration
  3. **Memory recent**: `search_memory("*")` with date filter for entries since last digest — decisions made, tasks noted, conversation summaries
  - Each fetch has an independent 30-second timeout. Partial results are accepted (if Jira is down, digest still includes Nexus + memory sections). Failed fetches produce a "[source] unavailable" line in the digest.

### Digest Synthesis

- `crates/nv-daemon/src/digest/synthesize.rs`: Claude prompt for digest synthesis — builds a system prompt instructing Claude to produce a structured digest with these sections:
  1. **Jira** — open issues grouped by priority (P0/P1 highlighted), blocked items called out, staleness warnings for issues untouched >3 days
  2. **Sessions** — running Claude Code sessions, recently completed sessions, any errors
  3. **Memory** — notable decisions, pending follow-ups, items flagged for review
  4. **Suggested Actions** — 3-5 actionable items Claude recommends based on the gathered context (e.g., "Close OO-142 — resolved yesterday", "Respond to Maria's Teams question about timeline")
  - Claude returns structured JSON with sections and suggested actions, each action having an `id`, `label`, and `payload` (Jira transition, memory update, or follow-up query)

### Telegram Formatting

- `crates/nv-daemon/src/digest/format.rs`: Telegram message formatter — converts Claude's structured digest into a Telegram message with:
  - Section headers with emoji indicators (bullet dots for items, warning indicators for P0/P1)
  - Truncation for long digests (Telegram 4096 char limit) — prioritize P0/P1 items, truncate lower priority
  - Inline keyboard with suggested action buttons — each button maps to an action `id` from Claude's response
  - "Dismiss All" button to acknowledge the digest without acting

### Action Execution

- `crates/nv-daemon/src/digest/actions.rs`: Action execution on callback — when Leo taps an action button, the `callback_query` handler (from spec-3) routes to this module. Matches action `id` against the stored digest actions in `last-digest.json`. Executes the action (Jira transition, memory write, follow-up query) and sends confirmation to Telegram. Updates `last-digest.json` with action completion status.

### State Persistence

- `crates/nv-daemon/src/digest/state.rs`: Digest state management — reads/writes `~/.nv/state/last-digest.json`:
  ```json
  {
    "last_sent_at": "2026-03-21T08:00:00Z",
    "content_hash": "sha256:...",
    "suggested_actions": [
      { "id": "act_1", "label": "Close OO-142", "payload": {...}, "status": "pending" }
    ],
    "sources_status": {
      "jira": "ok",
      "nexus": "ok",
      "memory": "ok"
    }
  }
  ```
  Content hash prevents duplicate digests if nothing changed since last send. On daemon restart, checks `last_sent_at` to skip if within the configured interval.

### CLI Trigger

- `crates/nv-cli/src/commands/digest.rs`: `nv digest --now` subcommand — sends HTTP POST to `http://localhost:{daemon_port}/digest` to trigger an immediate digest. The daemon's HTTP server (same one used for `nv ask`) handles this by pushing `Trigger::Cron(CronEvent::Digest)` with a `force: true` flag that bypasses the content-hash dedup check.

### Daemon Integration

- `crates/nv-daemon/src/main.rs`: Wire scheduler — spawn the cron scheduler task alongside channel listeners. Pass `mpsc::Sender<Trigger>` clone to scheduler.
- `crates/nv-daemon/src/agent_loop.rs`: Handle `Trigger::Cron(CronEvent::Digest)` — route to digest gather → synthesize → format → send pipeline. Distinct from message handling path.
- `crates/nv-daemon/src/http.rs`: Add `POST /digest` endpoint for CLI trigger.

## Dependencies

- `agent-loop` (spec-4) — agent loop processing triggers from mpsc channel
- `memory-system` (spec-5) — `search_memory` for recent entries
- `jira-integration` (spec-6) — `jira_search` for open issues

## Out of Scope

- Nexus session data in digest (Nexus client implemented in spec-9; digest will include a placeholder "[Nexus] not connected" section until spec-9 lands, then backfill is trivial)
- Digest scheduling via external cron/systemd timer (all scheduling is internal via tokio)
- Digest history beyond the last sent (no digest archive — just last-digest.json)
- Configurable digest sections (all sections always included; empty sections say "Nothing to report")
- Rich media in digest (images, charts — text only for v1)
