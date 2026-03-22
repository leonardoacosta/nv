# Context: Nova v3 — Data Source Integrations + Chat Feedback

## Context Tag

system

## Current State (Post-MVP v2 — COMPLETE)

**Delivered:** 21 specs across MVP + post-MVP. ~24K LOC Rust, 493 tests.
Running as systemd service with orchestrator pattern (non-blocking, 3 concurrent workers).

**Channels (6):** Telegram (active), Discord (code exists), Teams (code exists),
iMessage (code exists), Email (code exists), Jira webhooks (code exists).

**Architecture:** Orchestrator → WorkerPool → PersistentSession. Telegram reactions
as read receipts (👀→✅). Priority queue for message dispatch.

## Known Bugs (carry-forward — ALL must be fixed in v3)

### Critical (blocks usability)

1. **Tool call JSON leaking to Telegram** — raw `` `tool_call` `` blocks visible to user
   instead of being executed silently. Worker sends intermediate thinking + tool call JSON.
   Seen: `tool_call {"tool": "read_memory", "topic": "projects"}` shown to Leo.
   Fix: filter tool_call blocks from response text before sending to Telegram.

2. **Worker deserialization crash** — `⚠️ Worker error: Deserialization error: CLI JSON parse
   error: EOF while parsing a value at line 1 column 0`. Claude subprocess returns empty/invalid
   JSON. Worker crashes instead of retrying or falling back.

3. **Stalled tool calls** — "read_memory call stalled" then Nova self-reports the stall to user.
   Tool execution hangs without timeout, eventually worker recovers but loses context.

4. **Markdown table rendering broken** — Tables show raw `|------|----------|` in Telegram
   (visible in CT epics response). HTML converter doesn't handle tables.

### UX Issues

5. **Status updates not implemented** — orchestrator spec deferred the "Searching Jira..."
   30-second status updates for workers.

6. **No reply threading** — Nova sends responses as new messages, not replies to the original.
   In busy conversations, hard to tell which response maps to which question.

7. **Manual e2e gates** — 3 user tasks from hardening specs (Telegram echo, Nexus callbacks,
   Jira create flow).

### Positive (working well)

- 👀 Reactions working as read receipts
- Nova recovered from stall and continued conversation
- CT epic scaffolding was good quality (8 relevant epics proposed)
- Nova asked clarifying questions before acting

## Data Source Inventory

### 🟢 Live (Nova has it now)

| Source | Projects | Data |
|--------|----------|------|
| Jira | oo, tc | Issues, epics, status, priority, comments |
| Nexus | all | Active CC sessions, cost, duration, command |
| Git/Filesystem | all | Commits, branches, beads, schema files |
| SQLite | nv | Message history, analytics, diary |
| Memory (markdown) | nv | Decisions, projects, conversations |

### 🟡 Available (API exists, not wired)

| Source | Projects | Data | Access |
|--------|----------|------|--------|
| Neon PostgreSQL | oo, tc, tl, mv, ss, cl, cw | All app data | POSTGRES_URL via Doppler |
| Vercel | oo, tc, tl, mv, ss, la | Deploy status, logs | `vercel` CLI + REST API |
| Sentry | oo, tc, mv | Errors, traces, releases | API token |
| PostHog | oo, tc, mv | Analytics, feature flags | API key |
| Stripe | oo, tc | Payments, subscriptions | API key |
| Resend | oo, tc, mv | Email delivery events | API key |
| Upstash | oo, tc | Cache state, rate limits | URL + token |
| GitHub | all | PRs, actions, issues | `gh` CLI |
| Azure DevOps | ws | Pipelines, deployments | CLI + REST |

### ~~🔴 Manual/Blocked~~ → Reclassified after research

All 4 sources are accessible from Nova's homelab machine. None are truly blocked.

| Source | Projects | Access | Auth | Effort |
|--------|----------|--------|------|--------|
| Docker | hl | Unix socket `/var/run/docker.sock` (nyaptor in docker group) | None | S |
| Tailscale | hl, cw | `docker exec tailscale tailscale status --json` | None | S |
| Home Assistant | cl, hl | REST API `localhost:8123` (12 entities running) | HA long-lived token (generate in UI) | S-M |
| Plaid | cl | Read via cortex-postgres — Rust tool reads only allowed columns (account name, type, balance, last updated). No merchant names, transaction details, or account numbers ever reach Claude. PII filtered in Rust before tool result. | DB password | M |

## Leo's Core Problem (from memory/decisions.md)

> Aggregating data from multiple sources with different schemas into a coherent holistic
> status view. Goal is a 'command center' that lets him zoom in/out on any project or life
> aspect.
>
> Nova's eventual role: should own the canonical project health state object (health,
> active sessions, open issues, last deploy, last error) per project.

## Chat Feedback Issues

1. **Tool call leak** — Claude outputs `` `tool_call` `` JSON blocks that get sent to
   Telegram instead of being filtered. extract_text() should filter but isn't catching all cases.

2. **Response quality** — Nova sometimes outputs summaries instead of actionable commands.
   Leo wants: "commands first, context after" when asked for operational help.

3. **Commands on request** — Nova should only provide commands when asked, not default to them.

4. **Beads/Jira sync** — Leo wants Nova to manage Jira boards proactively, creating and
   managing the Civilant (ct) board specifically.

## v3 Scope (from user's direction)

**"All integrations highlighted + improving chat feedback"**

This means:
1. Wire the 🟡 Available data sources into Nova as tools
2. Fix the chat feedback bugs (tool call leak, response quality)
3. Build the "project health dashboard" — per-project status aggregation
4. Azure DevOps integration for work (ws) project
5. **Worker DAG observability** — workers emit progress events (load_context → claude_call →
   tool_loop → route_response), orchestrator streams milestones to Telegram as sub-agents
   complete stages. Treat each worker execution as a DAG with observable nodes, not fire-and-forget.
6. **Tool usage audit log** — every tool invocation (jira_search, query_nexus, vercel_status,
   sentry_errors, etc.) must be logged to SQLite with timestamp, tool name, input params,
   result summary, duration_ms, and which worker/trigger invoked it. This is foundational —
   must exist BEFORE wiring new data sources. Enables: usage analytics, cost tracking,
   debugging ("why did Nova query Jira 47 times today?"), and rate limit awareness.

## Additional Features (from idea audit)

### From session review + Telegram chat log

7. **Long-task confirmation** — if Nova estimates a task will take >1 minute, send a
   confirmation first: "This will take ~2 min — I'll search Jira across all projects and
   check Vercel deploys. Be right back." Then proceed. Better than silent 👀 for heavy queries.

8. **Reply threading** — respond as reply-to the original message (Telegram `reply_to_message_id`).
   Critical for orchestrator pattern — when 3 workers respond in parallel, user needs to know
   which response maps to which question.

9. **Telegram typing indicator** — `sendChatAction(typing)` while worker is processing. Standard
   bot UX, shows "Nova is typing..." in chat header. Lighter than reactions for quick responses.

10. **Quiet hours** — configurable time window where Nova suppresses all notifications except P0.
    Leo's sleep schedule is irregular — needs explicit quiet window, not assumption-based.

11. **Full-text search on message store** — SQLite FTS5 on the messages table. Schema supports it,
    tool not built. Easy win — "search my conversations for when we discussed Stripe fees."

12. **Nexus session retry button** — inline keyboard button on session error alerts to restart
    the failed CC session. Callback infrastructure exists from hardening specs.

13. **Voice-to-text (inbound)** — transcribe Leo's Telegram voice messages to text using Deepgram
    or Whisper API. Completes bidirectional voice (outbound TTS already built).

14. **Scoped bash toolkit** — Nova's own programmatic tools for basic project chores, executed
    directly in Rust via `Command::new()` (no Claude subprocess needed, instant ~10ms).
    Scoped to an allowlist of safe read-only commands per project:
    - Git: status, log, branch, diff --stat, remote status, stash list
    - Build: check if build passes (read-only gate check)
    - Files: ls, cat specific config files, grep patterns
    - No write operations (push, commit, reset) without PendingAction confirmation

    Each project in the registry (`~/.nv/memory/projects.md`) gets its path mapped, so
    Nova can run `git -C ~/dev/oo status` without Claude needing filesystem access.

    Discovery: audit CC session history for common git patterns Leo runs, build the
    allowlist from actual usage data.

## Discovery Metadata

- **Project**: nv (Nova)
- **Path**: /home/nyaptor/nv
- **Timestamp**: 2026-03-22T15:46:00-05:00
- **Detected mode**: system
- **Previous phases**: MVP (10 specs), Post-MVP (11 specs), both archived + tagged
