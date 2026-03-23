# PRD — Nova v3

> Accumulated from: scope-lock.md + context.md
> Generated: 2026-03-22

## 1. Vision

Make Nova a full-spectrum operations hub: fix critical chat bugs, wire all available data
sources with individual tools + aggregation layer, expose the full CC command surface through
Telegram, and mature Nexus into a remote control for Claude Code sessions.

## 2. Target Users

Leo. Solo operator. 20+ projects across work + LLC + personal.

## 3. Success Metrics

| Metric | Current (v2) | Target (v3) |
|--------|:---:|:---:|
| Chat bugs | 4 critical | 0 |
| Data sources wired | 3 (Jira, Nexus, filesystem) | 16 (+ aggregation layer) |
| Tool invocations logged | 0% | 100% |
| CC commands via Telegram | 0 | Full namespace |
| Voice bidirectional | Outbound only (TTS) | Inbound + outbound |

## 4. Phase 0: Fix Critical Bugs

### 4.1 Tool Call JSON Leak

**Problem:** Raw `` `tool_call` `` blocks visible in Telegram. Claude outputs tool call JSON
that should be intercepted by the worker but gets passed through to `extract_text()`.

**Fix:** Strip tool_call markdown blocks from response content before sending to Telegram.
Add regex filter in worker response routing: anything matching `` ```tool_call...``` `` gets
removed from the text sent to the user. Only final response text after all tool loops complete
should reach Telegram.

### 4.2 Worker Deserialization Crash

**Problem:** `CLI JSON parse error: EOF while parsing a value at line 1 column 0`. Claude
subprocess returns empty or truncated JSON.

**Fix:** Handle empty stdout gracefully. If `stream-json` response is empty or unparseable,
retry once. If retry fails, send error to Telegram ("Thinking failed — retrying...") and
fall back to cold-start mode for this turn.

### 4.3 Stalled Tool Calls

**Problem:** Tool execution hangs indefinitely. Worker self-reports "read_memory call stalled"
but doesn't timeout.

**Fix:** Add per-tool timeout (default 30s). If a tool_call doesn't return within timeout,
return error result to Claude ("Tool timed out") and let it continue without that data.

### 4.4 Markdown Table Rendering

**Problem:** Tables show raw `|------|` in Telegram. HTML converter doesn't handle tables.

**Fix:** Add table parsing to `markdown_to_html()` — convert markdown tables to `<pre>` blocks
(Telegram doesn't support `<table>`) with aligned columns, or use a condensed key-value format.

## 5. Phase 1: Foundation

### 5.1 Tool Usage Audit Log

SQLite table logging every tool invocation:

```sql
CREATE TABLE tool_usage (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL,
    worker_id TEXT,
    tool_name TEXT NOT NULL,
    input_summary TEXT,
    result_summary TEXT,
    success INTEGER NOT NULL,
    duration_ms INTEGER,
    tokens_in INTEGER,
    tokens_out INTEGER
);
```

Every tool call — jira_search, query_nexus, vercel_status, etc. — logged before returning
result. Enables: `nv stats` tool usage section, rate limit awareness, debugging.

### 5.2 Worker DAG Events

Workers emit progress events via `mpsc` channel to orchestrator:

```rust
enum WorkerEvent {
    StageStarted { worker_id: Uuid, stage: &str },
    ToolCalled { worker_id: Uuid, tool: &str },
    StageComplete { worker_id: Uuid, stage: &str, duration_ms: u64 },
    Complete { worker_id: Uuid, response_len: usize },
    Error { worker_id: Uuid, error: String },
}
```

Orchestrator decides what to surface to Telegram. For tasks >30s, sends brief status update.

### 5.3 Scoped Bash Toolkit

Allowlisted read-only commands per project, executed in Rust via `Command::new()`:

| Command | Example | Confirmation? |
|---------|---------|:---:|
| `git status` | `git -C ~/dev/oo status --short` | No |
| `git log` | `git -C ~/dev/oo log --oneline -10` | No |
| `git branch` | `git -C ~/dev/oo branch --show-current` | No |
| `git diff --stat` | `git -C ~/dev/oo diff --stat` | No |
| `ls` | `ls ~/dev/oo/packages/` | No |
| `cat` (config files) | `cat ~/dev/oo/package.json` | No |
| `bd ready` | `bd -C ~/dev/oo ready` | No |
| `bd stats` | `bd -C ~/dev/oo stats` | No |

No write operations without PendingAction confirmation. ~10ms execution (no Claude needed).

## 6. Phase 2: Data Sources

### 6.1 Individual Tools (13 sources)

Each source gets: HTTP client module + tool definition + config/secrets + logging.

**Tier 1 — Zero auth:**

| Source | Tools | Access |
|--------|-------|--------|
| Docker | `docker_status()`, `docker_logs(container, lines)` | Unix socket |
| Tailscale | `tailscale_status()`, `tailscale_node(name)` | `docker exec` CLI |
| GitHub | `gh_pr_list(repo)`, `gh_run_status(repo)`, `gh_issues(repo)` | `gh` CLI |

**Tier 2 — API key:**

| Source | Tools | Auth |
|--------|-------|------|
| Vercel | `vercel_deployments(project)`, `vercel_logs(deploy_id)` | CLI or REST |
| Sentry | `sentry_issues(project)`, `sentry_issue(id)` | API token |
| PostHog | `posthog_trends(project, event)`, `posthog_flags(project)` | API key |

**Tier 3 — OAuth/DB:**

| Source | Tools | Auth |
|--------|-------|------|
| Neon | `neon_query(project, sql)` | POSTGRES_URL per project |
| Stripe | `stripe_customers(query)`, `stripe_invoices(status)` | API key |
| Resend | `resend_emails(status)`, `resend_bounces()` | API key |
| Upstash | `upstash_info()`, `upstash_keys(pattern)` | URL + token |

**Tier 4 — Special:**

| Source | Tools | Auth | Notes |
|--------|-------|------|-------|
| Home Assistant | `ha_states()`, `ha_entity(id)`, `ha_service_call(domain, service, data)` | HA token | service_call needs confirmation |
| Azure DevOps | `ado_pipelines(project)`, `ado_builds(pipeline_id)` | Azure CLI | Day job |
| Plaid | `plaid_balances()`, `plaid_bills()` | DB read | Allowed columns only, PII filtered in Rust |

### 6.2 Aggregation Layer (3 composite tools)

| Tool | Sources Combined | Output |
|------|-----------------|--------|
| `project_health(code)` | Vercel + Sentry + Jira + Nexus + Neon + GitHub | 🟢/🟡/🔴 per dimension |
| `homelab_status()` | Docker + Tailscale + HA | Container + network + home health |
| `financial_summary()` | Plaid + Stripe | Account balances + upcoming bills |

`project_health` is what powers the dashboard digest:
```
OO: 🟢 deploy 22m ago | 🟢 0 errors | 🟡 3 Jira (1 P1) | 🟢 1 session
```

## 7. Phase 3: Features + UX

### 7.1 Chat UX Improvements

| Feature | Implementation |
|---------|---------------|
| Reply threading | Use `reply_to_message_id` on all responses — maps response to trigger |
| Typing indicator | `sendChatAction(typing)` when worker starts, before reaction |
| Long-task confirmation | If estimated >1min: "This will take ~2min. Searching Jira across all projects. Be right back." |
| Quiet hours | Config: `quiet_start = "23:00"`, `quiet_end = "07:00"`. Suppress all except P0 during window. |

### 7.2 Mature Nexus Integration

- Project-scoped queries: `"What's ready on OO?"` → `bd ready` via scoped bash
- List open proposals: `openspec/changes/` per project via filesystem
- **StartSession RPC**: Launch CC session from Telegram (with confirmation)
- **SendCommand RPC**: Run `/apply`, `/feature`, `/ci:gh` remotely
- **StopSession**: Kill runaway sessions (needs Nexus fix for managed sessions)

### 7.3 Telegram Command Surface

Register in BotFather + handle in orchestrator:
- `/status` → `project_health` for all projects
- `/digest` → trigger immediate digest
- `/health` → `homelab_status`
- `/apply <project> <spec>` → StartSession + SendCommand via Nexus
- `/projects` → list all projects with latest status dot

Transform output for mobile: inline keyboards for choices, status dots for health,
condensed tables (no raw `|---|`).

### 7.4 Message Search

FTS5 on SQLite messages table:
```sql
CREATE VIRTUAL TABLE messages_fts USING fts5(content, content=messages, content_rowid=id);
```
Tool: `search_messages(query, limit)` — "search my conversations for Stripe fee discussion"

### 7.5 Nexus Retry Button

Inline keyboard on session error alerts: `[🔄 Retry] [🐛 Create Bug]`
- Retry: `StartSession` + `SendCommand` with same spec/command
- Create Bug: existing `create_bug_from_session_error` flow

### 7.6 Voice-to-Text (Inbound)

Receive Telegram voice messages → transcribe via Deepgram or Whisper API → process as text.
- Download voice file via `getFile` API
- POST to Deepgram `https://api.deepgram.com/v1/listen`
- Inject transcribed text as regular `Trigger::Message`
- Config: `DEEPGRAM_API_KEY` in env

## 8. Scope & Constraints

### In Scope
Everything in Phases 0-3 above. 16 items total.

### Out of Scope
- Multi-user/multi-tenant
- Web dashboard (Nexus TUI)
- Embedding-based memory search
- Slack, IMAP, email attachments, iMessage groups
- Custom TTS providers, multi-model routing

### Hard Constraints
- Rust standalone binary
- Secrets via env file
- Linux homelab (systemd)
- Tailscale inter-machine
- Claude-only AI
- Single user
- Plaid: allowed columns only

## 9. Timeline

| Day | Phase | Specs |
|-----|-------|-------|
| 1 AM | Phase 0 | fix-chat-bugs (4 bugs) |
| 1 PM | Phase 1 | tool-audit-log, worker-dag-events, scoped-bash-toolkit |
| 2 AM | Phase 2a | docker-tools, tailscale-tools, github-tools |
| 2 PM | Phase 2b | vercel-tools, sentry-tools, posthog-tools |
| 3 AM | Phase 2c | neon-tools, stripe-tools, resend-tools, upstash-tools |
| 3 PM | Phase 2d | ha-tools, ado-tools, plaid-tools, aggregation-layer |
| 4 | Phase 3a | chat-ux, nexus-maturation, telegram-commands |
| 5 | Phase 3b | message-search, nexus-retry, voice-to-text |
| 6 | Verify | Integration testing + full deploy |
