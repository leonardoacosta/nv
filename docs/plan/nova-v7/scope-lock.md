# Scope Lock -- Nova v7

## Vision

Nova becomes a CC-native intelligence hosted in a Next.js dashboard, with the Rust daemon
reduced to a message broker for Telegram and channels.

## Target Users

Leo (sole operator). Nova is a personal operations daemon -- no multi-tenant, no public access.

## Domain

Daemon architecture migration, Telegram UX, memory persistence, dashboard rebuild.
Explicit boundary: this is infrastructure/DX work, not feature work for Leo's other projects.

## v7 Must-Do

Move Nova's brain from cold-start Claude CLI subprocesses into a persistent CC session
managed by a Next.js dashboard. Sub-10s Telegram response latency.

## v7 Won't-Do

- Multi-tenant support
- Public API
- New external integrations (existing 20+ tools stay, no new ones)
- Mobile app
- Multi-project coordination beyond what CC team agents provide natively

## Architecture Direction

### Current (v6)

```
Telegram --> nv-daemon (Rust) --> Claude CLI cold-start (18-30s)
                 |                      |
                 +-- tools/mod.rs ------+  (100 tools in system prompt)
                 +-- ConversationStore (in-memory, 10min timeout)
                 +-- MessageStore (SQLite)
                 +-- Dashboard (embedded React SPA)
```

### Target (v7)

```
Telegram --> nv-daemon (thin broker) --> Dashboard (Next.js)
                                              |
                                         CC Session (Docker)
                                              |
                                         Direct Anthropic API
                                              |
                                         Tool dispatch (Rust MCP)
```

- **nv-daemon**: Telegram long-poll, message routing, channel management. No Claude calls.
- **Dashboard (Next.js)**: Hosts Nova's CC session. Manages conversation state. Serves
  briefing pages, cold-start logs, session views. Full wireframe-aligned UI.
- **CC Session (Docker)**: Persistent Claude Code session with all tools. Managed by
  the Next.js server. Falls back to direct Anthropic API if CC CLI is unreliable.
- **nv-tools (MCP)**: Existing tool server, unchanged. Connected to the CC session.

### Fallback: Direct Anthropic API

If CC CLI persistent mode remains broken, bypass it entirely:
- Call Anthropic messages API with `reqwest` from Rust or Next.js server
- Tool schemas sent as native API `tools` parameter (not prompt-embedded)
- Full control over conversation state, streaming, token budgets
- Lose CC-native tools (Read/Glob/Grep) but gain sub-5s latency

## Waves

### Wave 1: Memory & Quick Fixes (UX layer, current architecture)

Ship immediately on existing daemon. No architecture changes.

1. **Cold-start memory loss fix** -- inject recent outbound messages into system prompt
2. **Telegram null bubble fix** -- meaningful callback answer text
3. **Session diary narrative** -- enrich diary entries with accomplishment summaries
4. **Typing indicator refresh** -- re-send on tool calls, throttled
5. **Nexus dedup fix** -- query before launch, skip already-represented projects
6. **300s timeout investigation** -- trace source, add user feedback
7. **reminders.db migration fix** -- delete and recreate (v7 migrations incompatible)

### Wave 2: Dashboard & Architecture (Next.js migration)

1. **Extract dashboard to Next.js** -- standalone app, Docker-hosted
2. **CC session management** -- Next.js manages persistent CC session
3. **Move Nova's brain** -- conversation engine migrates from daemon to CC session
4. **Morning briefing page** -- AI-generated daily digest on dashboard
5. **Cold-start dashboard logging** -- event tracking, latency charts
6. **Session slug names** -- human-readable names with dashboard links
7. **Full wireframe realignment** -- rebuild all dashboard pages to match approved wireframes

### Wave 3: Direct API Fallback (if CC CLI unreliable)

1. **Anthropic API client in Rust** -- reqwest-based, streaming support
2. **Native tool_use protocol** -- tool schemas as API parameter, not prompt
3. **Persistent conversation state** -- SQLite-backed, survives restarts
4. **Response latency target** -- sub-5s for simple queries, sub-10s for tool calls

### Wave 4: Nexus Deprecation & Team Agent Migration

1. **Replace Nexus dispatch with CC team agents** -- drop gRPC client, use native coordination
2. **Remove nexus crate** -- delete nexus/ module from nv-daemon
3. **Update session lifecycle** -- team agents handle start/stop/monitor natively
4. **Clean up Nexus config** -- remove agent endpoints from nv.toml

## Latency Target

Under 10 seconds for Telegram responses. This is a hard requirement that drives
the architecture migration away from cold-start CLI subprocesses.

## Hard Constraints

- Single operator (Leo), single homelab deployment
- Doppler for secrets, systemd for services
- nv-tools MCP server must remain Rust (performance + existing 45 tools)
- Dashboard must be accessible via Tailscale (not public internet)

## Timeline

No external deadline. Ship Wave 1 immediately (existing architecture), Wave 2 within
the next planning cycle, Wave 3 only if CC CLI remains unreliable, Wave 4 last
(Nexus deprecation after team agents are proven stable).

## Assumptions Corrected

- "Cold-start is fine for now" --> Under 10s is a hard requirement, not a nice-to-have
- "Dashboard is optional polish" --> Dashboard becomes the primary intelligence host
- "Nexus is the orchestration future" --> Migrating to CC team agents (v8), dedup fix only in v7
- "Keep everything in Rust" --> Next.js for dashboard, Rust for daemon/tools only

## Idea Coverage

All 9 backlog ideas are in scope:

| Idea | Wave | Spec |
|------|------|------|
| session-diary-narrative (nv-de2) | Wave 1 | #3 |
| morning-briefing-digest (nv-837) | Wave 2 | #4 |
| cold-start-dashboard-logging (nv-clp) | Wave 2 | #5 |
| dashboard-wireframe-drift (nv-4zs) | Wave 2 | #7 |
| telegram-null-bubble-on-approve (nv-zsr) | Wave 1 | #2 |
| session-slug-names-with-dashboard-links (nv-wqd) | Wave 2 | #6 |
| request-timeout-300s-investigation (nv-yhu) | Wave 1 | #6 |
| telegram-typing-and-presence-status (nv-b4i) | Wave 1 | #4 |
| nexus-duplicate-sessions-vs-team-agents (nv-unw) | Wave 1 (dedup) + Wave 4 (full deprecation) | #5 / W4 |
