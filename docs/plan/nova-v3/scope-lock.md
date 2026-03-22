# Scope Lock — Nova v3

## Vision

Make Nova a full-spectrum operations hub: fix critical chat bugs, wire all available data
sources with individual tools + aggregation layer, expose the full CC command surface through
Telegram, and mature Nexus into a remote control for Claude Code sessions.

## Target Users

Leo. Same as always.

## Domain

**In scope:** Bug fixes (4 critical + 3 UX), 13 data source integrations (individual tools +
project health aggregation), worker DAG observability, tool usage audit log, scoped bash toolkit,
mature Nexus integration (remote CC control), Telegram command surface, chat UX improvements
(threading, typing indicator, long-task confirmation, quiet hours), full-text message search,
Nexus retry button, voice-to-text inbound.

**Out of scope:** Multi-user, web UI, embedding search, Slack channel, IMAP fallback, email
attachments, iMessage group chats, custom TTS providers, multi-model routing.

## Execution Order

**Phase 0 (bugs) → Phase 1 (foundation) → Phase 2 (data sources) → Phase 3 (features + UX)**

### Phase 0: Fix Critical Bugs (Day 1 AM)

Must fix before any new features — these block usability.

| Bug | Description |
|-----|-------------|
| Tool call JSON leak | Raw `` `tool_call` `` blocks visible in Telegram |
| Worker deserialization crash | `EOF while parsing a value at line 1 column 0` |
| Stalled tool calls | Tool execution hangs without timeout |
| Markdown table rendering | Tables show raw `|------|` in Telegram |

Bundle into one spec: `fix-chat-bugs`

### Phase 1: Foundation (Day 1 PM)

Infrastructure that must exist before data sources.

| Spec | Description |
|------|-------------|
| `add-tool-audit-log` | SQLite table logging every tool invocation (name, params, duration, result) |
| `add-worker-dag-events` | Workers emit progress events, orchestrator streams milestones to Telegram |
| `add-scoped-bash-toolkit` | Allowlisted git/file commands per project, executed in Rust (~10ms) |

### Phase 2: Data Sources (Day 2-3)

Individual tool per source + aggregation layer.

**Tier 1 — Zero auth (Day 2 AM):**

| Source | Tool | Access |
|--------|------|--------|
| Docker | `docker_status`, `docker_logs` | Unix socket, already has access |
| Tailscale | `tailscale_status`, `tailscale_node` | `docker exec tailscale tailscale status --json` |
| GitHub | `gh_pr_list`, `gh_run_status` | `gh` CLI already authenticated |

**Tier 2 — API key (Day 2 PM):**

| Source | Tool | Auth |
|--------|------|------|
| Vercel | `vercel_deployments`, `vercel_logs` | `vercel` CLI or REST API |
| Sentry | `sentry_issues`, `sentry_traces` | API token (Doppler) |
| PostHog | `posthog_trends`, `posthog_flags` | API key (Doppler) |

**Tier 3 — OAuth/DB (Day 3 AM):**

| Source | Tool | Auth |
|--------|------|------|
| Neon PostgreSQL | `neon_query` | POSTGRES_URL per project (Doppler) |
| Stripe | `stripe_customers`, `stripe_invoices` | API key (Doppler) |
| Resend | `resend_status`, `resend_bounces` | API key (Doppler) |
| Upstash | `upstash_keys`, `upstash_stats` | URL + token (Doppler) |

**Tier 4 — Special (Day 3 PM):**

| Source | Tool | Auth |
|--------|------|------|
| Home Assistant | `ha_states`, `ha_entity`, `ha_service_call` | HA long-lived token |
| Azure DevOps | `ado_pipelines`, `ado_builds` | Azure CLI |
| Plaid | `plaid_balances`, `plaid_bills` | Read cortex-postgres (allowed columns only) |

**Aggregation layer:**

| Tool | What it calls |
|------|---------------|
| `project_health(code)` | vercel + sentry + jira + nexus + neon (parallel) → unified status |
| `homelab_status()` | docker + tailscale + HA → infrastructure health |
| `financial_summary()` | plaid (allowed columns) + stripe → balance overview |

### Phase 3: Features + UX (Day 4-5)

| Spec | Description |
|------|-------------|
| `chat-ux-improvements` | Reply threading, typing indicator, long-task confirmation, quiet hours |
| `mature-nexus-integration` | StartSession + SendCommand RPCs, project-scoped queries, proposal listing |
| `telegram-command-surface` | Expose `/plan:*`, `/apply`, `/feature` etc. in Telegram-native format |
| `add-message-search` | FTS5 on SQLite message store |
| `add-nexus-retry` | Inline button to retry failed sessions |
| `add-voice-to-text` | Inbound voice message transcription (Deepgram/Whisper) |

## v1 Must-Do

Fix the 4 critical chat bugs. Everything else is secondary — if Nova's chat is broken,
no amount of data sources matters.

## v1 Won't-Do

- Multi-user/multi-tenant
- Web dashboard (Nexus TUI handles this)
- Embedding-based memory search
- Slack channel
- IMAP email fallback
- Email attachments
- iMessage group chats
- Custom TTS providers
- Multi-model routing (haiku for triage)

## Hard Constraints

Same as previous phases:
- Rust standalone binary
- Secrets via env file (Doppler-sourced)
- Linux homelab (systemd)
- Tailscale for inter-machine
- Claude-only AI
- Single user, no auth
- Plaid data: allowed columns only, PII filtered in Rust

## Timeline

All 16 items, this week:
- **Day 1 AM:** Phase 0 — fix 4 critical bugs
- **Day 1 PM:** Phase 1 — foundation (audit log, DAG events, bash toolkit)
- **Day 2:** Phase 2a — zero-auth data sources (Docker, Tailscale, GitHub)
- **Day 2 PM:** Phase 2b — API-key data sources (Vercel, Sentry, PostHog)
- **Day 3:** Phase 2c-d — OAuth/DB + special sources + aggregation layer
- **Day 4:** Phase 3a — chat UX + Nexus maturation + command surface
- **Day 5:** Phase 3b — message search, Nexus retry, voice-to-text
- **Day 6:** Integration testing + full deploy

## Assumptions Corrected

- ~~4 data sources are blocked~~ → **All available** (Docker/Tailscale zero auth, HA needs token, Plaid via DB)
- ~~Nexus isn't ready for remote control~~ → **10/11 RPCs implemented** (SendCommand is production-ready)
- ~~Individual tools OR aggregation~~ → **Both** (tools are atoms, aggregation is orchestration)
- ~~Bugs can ship alongside features~~ → **Phase 0** (fix first, then build)
