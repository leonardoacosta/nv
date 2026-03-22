# Roadmap — Nova v3

> Generated from v3 PRD. ~25 specs across 4 phases, 10 waves.
> Execution order: Bugs → Foundation → Data Sources → Features/UX

---

## Wave 1: Fix Critical Bugs (Day 1 AM)

### Spec 1: `fix-chat-bugs`

**Type:** bugfix | **Effort:** M | **Deps:** none

Bundle all 4 critical bugs into one spec:
- Strip tool_call blocks from response text before sending to Telegram
- Handle empty/truncated CLI JSON (retry once, fall back to cold-start)
- Add per-tool timeout (30s default) for stalled tool execution
- Add table rendering to markdown_to_html() converter

**Gate:** Send message with tool use → no raw JSON visible. Tables render cleanly.

---

## Wave 2: Foundation (Day 1 PM)

### Spec 2: `add-tool-audit-log`

**Type:** feature | **Effort:** S | **Deps:** none

SQLite table for tool invocations. Log before every tool result returns.
Extends `nv stats` with tool usage section.

### Spec 3: `add-worker-dag-events`

**Type:** feature | **Effort:** M | **Deps:** none

Workers emit WorkerEvent enum via mpsc. Orchestrator decides what to surface.
Long-task confirmation: if estimated >1min, send confirmation before proceeding.

### Spec 4: `add-scoped-bash-toolkit`

**Type:** feature | **Effort:** M | **Deps:** none

Allowlisted read-only commands (git status/log/branch/diff, ls, cat, bd ready/stats)
per project via Command::new(). ~10ms execution, no Claude needed.

---

## Wave 3: Data Sources — Zero Auth (Day 2 AM)

### Spec 5: `add-docker-tools`

**Type:** feature | **Effort:** S | **Deps:** tool-audit-log

Docker container health via unix socket or `docker` CLI.
Tools: `docker_status()`, `docker_logs(container, lines)`

### Spec 6: `add-tailscale-tools`

**Type:** feature | **Effort:** S | **Deps:** tool-audit-log

Tailscale network topology via `docker exec tailscale tailscale status --json`.
Tools: `tailscale_status()`, `tailscale_node(name)`

### Spec 7: `add-github-tools`

**Type:** feature | **Effort:** S | **Deps:** tool-audit-log

GitHub via `gh` CLI (already authenticated).
Tools: `gh_pr_list(repo)`, `gh_run_status(repo)`, `gh_issues(repo)`

---

## Wave 4: Data Sources — API Key (Day 2 PM)

### Spec 8: `add-vercel-tools`

**Type:** feature | **Effort:** M | **Deps:** tool-audit-log

Vercel deploy status via REST API or `vercel` CLI.
Tools: `vercel_deployments(project)`, `vercel_logs(deploy_id)`

### Spec 9: `add-sentry-tools`

**Type:** feature | **Effort:** M | **Deps:** tool-audit-log

Sentry error tracking via REST API.
Tools: `sentry_issues(project)`, `sentry_issue(id)`

### Spec 10: `add-posthog-tools`

**Type:** feature | **Effort:** M | **Deps:** tool-audit-log

PostHog analytics via REST API.
Tools: `posthog_trends(project, event)`, `posthog_flags(project)`

---

## Wave 5: Data Sources — OAuth/DB (Day 3 AM)

### Spec 11: `add-neon-tools`

**Type:** feature | **Effort:** M | **Deps:** tool-audit-log

Direct SQL queries to Neon PostgreSQL via POSTGRES_URL per project.
Tools: `neon_query(project, sql)` — read-only queries, parameterized.

### Spec 12: `add-stripe-tools`

**Type:** feature | **Effort:** S | **Deps:** tool-audit-log

Stripe read-only via REST API.
Tools: `stripe_customers(query)`, `stripe_invoices(status)`

### Spec 13: `add-resend-tools`

**Type:** feature | **Effort:** S | **Deps:** tool-audit-log

Resend email delivery status via REST API.
Tools: `resend_emails(status)`, `resend_bounces()`

### Spec 14: `add-upstash-tools`

**Type:** feature | **Effort:** S | **Deps:** tool-audit-log

Upstash Redis info via REST API.
Tools: `upstash_info()`, `upstash_keys(pattern)`

---

## Wave 6: Data Sources — Special (Day 3 PM)

### Spec 15: `add-ha-tools`

**Type:** feature | **Effort:** M | **Deps:** tool-audit-log

Home Assistant via REST API on localhost:8123.
Tools: `ha_states()`, `ha_entity(id)`, `ha_service_call(domain, service, data)` (needs confirmation)

### Spec 16: `add-ado-tools`

**Type:** feature | **Effort:** M | **Deps:** tool-audit-log

Azure DevOps via REST API or `az` CLI.
Tools: `ado_pipelines(project)`, `ado_builds(pipeline_id)`

### Spec 17: `add-plaid-tools`

**Type:** feature | **Effort:** M | **Deps:** tool-audit-log

Plaid via cortex-postgres read-only. Allowed columns only, PII filtered in Rust.
Tools: `plaid_balances()`, `plaid_bills()`

### Spec 18: `add-aggregation-layer`

**Type:** feature | **Effort:** M | **Deps:** all data source specs

Three composite tools that call individual tools in parallel:
- `project_health(code)` — Vercel + Sentry + Jira + Nexus + Neon + GitHub
- `homelab_status()` — Docker + Tailscale + HA
- `financial_summary()` — Plaid + Stripe

---

## Wave 7: Chat UX (Day 4 AM)

### Spec 19: `improve-chat-ux`

**Type:** feature | **Effort:** M | **Deps:** fix-chat-bugs

- Reply threading (reply_to_message_id on all responses)
- Typing indicator (sendChatAction typing)
- Long-task confirmation ("This will take ~2min...")
- Quiet hours config (suppress non-P0 during window)

---

## Wave 8: Nexus + Commands (Day 4 PM)

### Spec 20: `mature-nexus-integration`

**Type:** feature | **Effort:** L | **Deps:** scoped-bash-toolkit

- Project-scoped queries (bd ready, proposals list per project)
- StartSession RPC from Telegram (with confirmation)
- SendCommand RPC for remote /apply, /feature, /ci:gh
- StopSession for runaway sessions

### Spec 21: `add-telegram-commands`

**Type:** feature | **Effort:** M | **Deps:** mature-nexus, aggregation-layer

Register in BotFather: /status, /digest, /health, /apply, /projects
Transform output for mobile: inline keyboards, status dots, condensed format.

---

## Wave 9: Search + Retry (Day 5 AM)

### Spec 22: `add-message-search`

**Type:** feature | **Effort:** S | **Deps:** none

FTS5 on SQLite messages table. Tool: `search_messages(query, limit)`

### Spec 23: `add-nexus-retry`

**Type:** feature | **Effort:** S | **Deps:** mature-nexus

Inline button on session error alerts: [🔄 Retry] [🐛 Create Bug]

---

## Wave 10: Voice-to-Text (Day 5 PM)

### Spec 24: `add-voice-to-text`

**Type:** feature | **Effort:** M | **Deps:** none

Inbound Telegram voice → Deepgram API → transcribed text → Trigger::Message.
Config: DEEPGRAM_API_KEY in env.

---

## Spec Dependency Graph

```
spec-1 (fix bugs)
  └─→ spec-19 (chat UX) — needs bugs fixed first

spec-2 (tool audit) ─── foundation for ALL data source specs (3-18)
spec-3 (DAG events) ─── independent
spec-4 (bash toolkit) ─→ spec-20 (nexus maturation)

specs 5-17 (data sources) ─── all depend on spec-2, independent of each other
  └─→ spec-18 (aggregation) ─── depends on all data sources

spec-18 (aggregation) ─→ spec-21 (telegram commands)
spec-20 (nexus) ─→ spec-21 (telegram commands)
spec-20 (nexus) ─→ spec-23 (nexus retry)

spec-22 (search) ─── independent
spec-24 (voice-to-text) ─── independent
```

## Wave Execution Plan

| Wave | Day | Specs | Parallelism |
|------|-----|-------|-------------|
| 1 | Day 1 AM | fix-chat-bugs | Sequential (1 spec) |
| 2 | Day 1 PM | tool-audit-log, worker-dag-events, scoped-bash-toolkit | Parallel (3 specs) |
| 3 | Day 2 AM | docker-tools, tailscale-tools, github-tools | Parallel (3 specs) |
| 4 | Day 2 PM | vercel-tools, sentry-tools, posthog-tools | Parallel (3 specs) |
| 5 | Day 3 AM | neon-tools, stripe-tools, resend-tools, upstash-tools | Parallel (4 specs) |
| 6 | Day 3 PM | ha-tools, ado-tools, plaid-tools, aggregation-layer | Sequential (aggregation depends on all) |
| 7 | Day 4 AM | improve-chat-ux | Sequential (1 spec) |
| 8 | Day 4 PM | mature-nexus, telegram-commands | Sequential (dependency chain) |
| 9 | Day 5 AM | message-search, nexus-retry | Parallel (2 specs) |
| 10 | Day 5 PM | voice-to-text | Sequential (1 spec) |

**Total: 24 specs across 10 waves**
