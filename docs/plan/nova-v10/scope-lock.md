# Scope Lock — Nova v10: Tool Fleet + Dashboard Consolidation

## Vision

Decompose Nova's 47 tools into independently deployable Hono+MCP microservices, wire conversation
history, fix the dashboard, and consolidate all MS Graph operations through the authenticated
CloudPC via SSH.

## Target Users

- **Leo (operator)** — power user running 10+ concurrent Claude agents, each needing tool access
- **Nova (agent)** — the AI assistant consuming tools via MCP native discovery
- **Dashboard (frontend)** — Next.js app consuming tool services via HTTP for UI rendering

## Domain

**IN scope:**
- 9 Hono+MCP tool services (memory, messages, channels, discord, teams, schedule, graph, meta, tool-router)
- SSH-to-CloudPC bridge for all MS Graph operations (Teams, Outlook, Calendar, ADO)
- Slim daemon refactor (keep Telegram polling + agent dispatch, move everything else to fleet)
- Conversation history wiring (ConversationManager → agent loop)
- Dashboard fixes (5 polish specs + authentication)
- Traefik routing for all tool services
- systemd target group for fleet management
- MCP server registration in Claude config

**OUT of scope:**
- ElevenLabs TTS/STT (separate v11 spec)
- Jira tools (existing MCP server handles these)
- Plaid/Stripe/Resend/Sentry/PostHog/Upstash/Vercel/Neon tools (low priority, existing CLIs sufficient)
- Web tools (fetch_url, search_web — Claude Code has WebFetch/WebSearch built in)
- Aggregation tools (project_health, homelab_status, financial_summary — defer until base tools stable)
- Cloudflare/Docker/Doppler/GitHub/Home Assistant tools (accessible via CLI already)

## Architecture

### Daemon Slim + Fleet

```
nova-ts.service (slim daemon)
├── Telegram polling (long-lived)
├── Agent dispatch (processMessage → Claude CLI)
└── Callback routing (watcher, obligation)

nova-tools.target (fleet)
├── tool-router.service      :4100  — central dispatch + health aggregation
├── memory-svc.service        :4101  — read/write/search memory (Postgres + fs)
├── messages-svc.service      :4102  — recent messages, search (Postgres)
├── channels-svc.service      :4103  — list channels, send to channel
├── discord-svc.service       :4104  — guilds, channels, messages (Bot API)
├── teams-svc.service         :4105  — chats, channels, presence, send (SSH→CloudPC)
├── schedule-svc.service      :4106  — reminders, schedules, sessions (Postgres)
├── graph-svc.service         :4107  — calendar, ADO (SSH→CloudPC PowerShell)
└── meta-svc.service          :4108  — check_services, self_assessment, update_soul
```

### Dual HTTP + MCP

Each service exposes:
- **HTTP** (Hono) — for dashboard and direct tool calls
- **MCP** (stdio) — for Agent SDK native tool discovery

MCP registration in `~/.claude/mcp.json` per service.

### SSH-to-CloudPC (Primary for MS Graph)

All Microsoft Graph operations route through SSH to the CloudPC's authenticated PowerShell:

```
Agent → teams-svc → SSH → CloudPC → PowerShell (graph-teams.ps1) → Graph API
Agent → graph-svc → SSH → CloudPC → PowerShell (graph-outlook.ps1) → Graph API
```

Host: `cloudpc` (SSH config). PowerShell scripts manage their own OAuth tokens.
This is the proven pattern from the Rust daemon — no direct Graph API from homelab.

### Traefik Exposure

Tool services get public routes via Traefik:
```
tools.nova.leonardoacosta.dev/memory/*
tools.nova.leonardoacosta.dev/messages/*
tools.nova.leonardoacosta.dev/teams/*
...
```

Dashboard calls services directly via these URLs. Internal agents use localhost ports.

## Tool Inventory (v10 scope — 9 services, ~30 tools)

| Service | Tools | Data Source |
|---------|-------|-------------|
| memory-svc | read_memory, write_memory, search_memory | Postgres `memory` + `~/.nv/memory/` fs |
| messages-svc | get_recent_messages, search_messages | Postgres `messages` |
| channels-svc | list_channels, send_to_channel | Adapter registry (Telegram, Discord) |
| discord-svc | discord_list_guilds, discord_list_channels, discord_read_messages | Discord Bot API |
| teams-svc | teams_list_chats, teams_read_chat, teams_messages, teams_channels, teams_presence, teams_send | SSH→CloudPC→Graph API |
| schedule-svc | set_reminder, cancel_reminder, list_reminders, add_schedule, modify_schedule, remove_schedule, list_schedules, start_session, stop_session | Postgres (new tables) |
| graph-svc | calendar_today, calendar_upcoming, calendar_next, ado_projects, ado_pipelines, ado_builds | SSH→CloudPC→Graph API |
| meta-svc | check_services, self_assessment_run, update_soul | Health checks + Postgres + fs |
| tool-router | dispatch(tool_name, input) | Routes to services |

## Dashboard Scope

- Wire 5 existing polish specs (improve-messages-ux, improve-obligation-ux, improve-system-pages, polish-dashboard, restructure-navigation)
- Add authentication (nv-x3m: dashboard-authentication idea)
- Connect dashboard to tool fleet services via Traefik URLs

## Conversation History

Wire the existing ConversationManager into the agent loop:
- Load last N messages before each agent call
- Save user message + assistant response after each call
- Inject history as context in the agent prompt

## v1 Must-Do

Nova can call all 30 tools natively via MCP, conversation history persists between messages,
and the dashboard shows live data from tool services.

## v1 Won't-Do

- ElevenLabs voice
- Financial/aggregation meta-tools
- Tool analytics/observability dashboard
- Rate limiting or auth on tool service endpoints (homelab only)
- Docker containerization of tool services

## Hard Constraints

- CloudPC must be online for Teams/Outlook/Calendar/ADO tools
- All services bare metal (systemd, not Docker) — Agent SDK needs fs access
- Doppler for secrets injection
- Traefik for reverse proxy (existing homelab infrastructure)
- Postgres shared across all services (single pgvector instance)

## Scale Target

- 10+ concurrent Claude agents calling tools simultaneously
- Each service handles its own load independently
- Per-service restart without affecting others

## Timeline

No external deadline. Ship in waves — critical path first (memory, messages, channels), then
Graph tools, then dashboard.

## Assumptions Corrected

- "Direct Graph API from homelab" → SSH to CloudPC is primary (proven Rust pattern)
- "MCP later" → MCP from day one (dual HTTP+MCP per service)
- "Localhost only" → Traefik-exposed (dashboard needs direct access)
- "Tool fleet only" → Include dashboard fixes + auth + conversation history
- "CLI tools already prove Graph works" → CLIs are unproven with real creds; SSH is the reliable path
