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

## Known Bugs (carry-forward)

1. **Tool call JSON leaking to Telegram** — raw `` `tool_call` `` blocks visible to user
   instead of being executed silently. Worker sends intermediate thinking + tool call JSON
   to Telegram. Fix: filter tool_call blocks from response before sending.

2. **Status updates not implemented** — orchestrator spec deferred the "Searching Jira..."
   30-second status updates for workers.

3. **Manual e2e gates** — 3 user tasks from hardening specs (Telegram echo, Nexus callbacks,
   Jira create flow).

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

### 🔴 Manual/Blocked

| Source | Projects | Notes |
|--------|----------|-------|
| Home Assistant | cl, hl | REST API available, home automation |
| Docker | hl | Container health via REST |
| Tailscale | hl, cw | VPN node status |
| Plaid | cl | Banking (sensitive) |

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

## Discovery Metadata

- **Project**: nv (Nova)
- **Path**: /home/nyaptor/nv
- **Timestamp**: 2026-03-22T15:46:00-05:00
- **Detected mode**: system
- **Previous phases**: MVP (10 specs), Post-MVP (11 specs), both archived + tagged
