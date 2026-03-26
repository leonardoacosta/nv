# Nova v10 Context — Tool Fleet + Dashboard Consolidation

## Previous Phase Summary

Nova v9 delivered a clean-room TypeScript rewrite of the daemon: packages/daemon with Telegram
adapter, Agent SDK integration, Hono HTTP API, 5 CLI tools, obligation system, diary, proactive
watcher, memory system, and morning briefing. Deployed to homelab via systemd.

## Architecture Decision: Tool Fleet

Nova v10 adopts a **microservices tool fleet** architecture. Instead of a monolith daemon, each
tool domain runs as its own Hono server instance:

- 9 Hono microservices in `packages/tools/*-svc/`
- Ports 4000-4008, each with its own systemd user service
- Grouped under `nova-tools.target` for batch management
- Central tool router at :4000 dispatches by tool name
- All services share Postgres via DATABASE_URL
- MCP integration deferred — HTTP router first

**Why:** Leo runs 10+ concurrent agents. A monolith Node process has thread contention.
Independent services give process isolation, fault tolerance, and rolling deploys.

## Tool Services to Build

| Service | Port | Tools | Source |
|---------|------|-------|--------|
| tool-router | :4000 | Central dispatch | New |
| memory-svc | :4001 | read_memory, write_memory, search_memory | Port from Rust |
| messages-svc | :4002 | get_recent_messages, search_messages | Port from Rust |
| channels-svc | :4003 | list_channels, send_to_channel | Port from Rust |
| discord-svc | :4004 | discord_list_guilds, discord_list_channels, discord_read_messages | Port from Rust |
| teams-svc | :4005 | teams_list_chats, teams_read_chat, teams_messages, teams_channels, teams_presence, teams_send | Port from Rust |
| schedule-svc | :4006 | set_reminder, cancel_reminder, list_reminders, add_schedule, modify_schedule, remove_schedule, list_schedules, start_session, stop_session | Port from Rust |
| graph-svc | :4007 | calendar_today, calendar_upcoming, calendar_next, ado_projects, ado_pipelines, ado_builds | Rewrite (Graph API via SSH to CloudPC) |
| meta-svc | :4008 | check_services, self_assessment_run, update_soul | Port from Rust |

## Carry-Forward: Deferred from v9

- Conversation history wiring (ConversationManager exists, not used)
- ElevenLabs TTS/STT integration
- 5 dashboard polish specs (improve-messages-ux, improve-obligation-ux, improve-system-pages, polish-dashboard, restructure-navigation)
- Root tsconfig.json for LSP resolution

## Carry-Forward: Open Beads

50 open beads issues — review with `bd list --status=open` for candidates.

## Runtime State

- Nova TS daemon: active on port 3443 (systemd nova-ts.service)
- Postgres: pgvector/pgvector:pg17 on port 5432 (Docker)
- 6 tables: messages, obligations, contacts, diary, memory, briefings
- Telegram: connected, polling, agent wired
- Rust daemon: stopped (nv.service inactive)
- Dashboard: Docker container on homelab (Traefik reverse proxy)

## Key Constraints

- Agent SDK spawns claude CLI — services that call it need ~/.claude/ credentials
- Graph API tools (ADO, calendar) must use SSH to CloudPC for authenticated PowerShell
- All services run bare metal (not Docker) for filesystem access
- Doppler for secrets, systemd for process management
