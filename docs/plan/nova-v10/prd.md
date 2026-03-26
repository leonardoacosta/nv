# PRD — Nova v10: Tool Fleet + Dashboard Consolidation

## Overview

Decompose Nova's monolith daemon into independently deployable Hono+MCP microservices, port 30
tools from Rust, wire conversation history, fix the dashboard, and consolidate MS Graph operations
through SSH-to-CloudPC.

## Architecture

### Slim Daemon + Tool Fleet

The existing nova-ts.service keeps only Telegram polling + agent dispatch. All tool logic moves
to independent services under nova-tools.target.

### Service Inventory

| Service | Port | Tools | Transport |
|---------|------|-------|-----------|
| tool-router | :4000 | dispatch(tool_name, input) | HTTP only |
| memory-svc | :4001 | read_memory, write_memory, search_memory | HTTP + MCP |
| messages-svc | :4002 | get_recent_messages, search_messages | HTTP + MCP |
| channels-svc | :4003 | list_channels, send_to_channel | HTTP + MCP |
| discord-svc | :4004 | discord_list_guilds, discord_list_channels, discord_read_messages | HTTP + MCP |
| teams-svc | :4005 | teams_*, 6 tools | HTTP + MCP |
| schedule-svc | :4006 | reminders (3) + schedules (4) + sessions (2) | HTTP + MCP |
| graph-svc | :4007 | calendar (3) + ado (3) | HTTP + MCP |
| meta-svc | :4008 | check_services, self_assessment_run, update_soul | HTTP + MCP |

### MS Graph: SSH to CloudPC (Primary)

All Microsoft Graph operations (Teams, Outlook, Calendar, ADO) route through SSH to the CloudPC's
authenticated PowerShell. PowerShell scripts manage their own OAuth tokens.

### Dual HTTP + MCP

Each service is both a Hono HTTP server (for dashboard) and an MCP server (for Agent SDK native
tool discovery). MCP registered in ~/.claude/mcp.json.

### Traefik Exposure

Services get public routes: tools.nova.leonardoacosta.dev/{service}/*

## Feature Areas

### F1: Tool Fleet Infrastructure
- Tool router service (central dispatch)
- Shared service scaffold (Hono + MCP dual template)
- systemd target group (nova-tools.target)
- Traefik routing config
- Deploy script for fleet

### F2: Core Tool Services
- memory-svc (Postgres + filesystem sync + pgvector search)
- messages-svc (Postgres queries, pagination)
- channels-svc (adapter registry, dispatch to Telegram/Discord)

### F3: Communication Tool Services
- discord-svc (Discord Bot API)
- teams-svc (SSH→CloudPC→Graph API)
- graph-svc (SSH→CloudPC for calendar + ADO)

### F4: Automation Tool Services
- schedule-svc (reminders, schedules, sessions — new Postgres tables)
- meta-svc (health checks, self-assessment, soul updates)

### F5: Daemon Refactor
- Slim daemon (remove tool logic, keep Telegram + agent dispatch)
- Wire ConversationManager into agent loop
- Route tool calls to fleet via MCP

### F6: Dashboard
- 5 polish specs (messages UX, obligation UX, system pages, polish, navigation)
- Authentication (nv-x3m idea)
- Connect to tool fleet via Traefik URLs

## Dependencies

```
F1 (infrastructure) → F2 (core tools) → F3 (comms) + F4 (automation)
                    → F5 (daemon refactor)
                    → F6 (dashboard)
```

F1 must ship first. F2-F6 can partially overlap after F1.

## Constraints

- CloudPC must be online for Graph tools
- Bare metal (systemd, not Docker) — Agent SDK needs fs
- Doppler for secrets, Traefik for proxy
- Shared Postgres instance
