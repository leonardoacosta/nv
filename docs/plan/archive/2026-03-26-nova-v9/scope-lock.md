# Scope Lock -- Nova v9

## Vision

Clean-room rewrite of Nova's daemon from Rust to TypeScript, powered by the Anthropic Agent SDK
and Vercel AI Gateway, with Postgres+pgvector (local Docker) via Drizzle ORM and CLI-wrapped
tools accessible via Claude Code's native tool system.

## Target Users

Leo (sole operator). Nova serves Leo across Telegram (primary), Discord, Teams.

## Domain

Nova's daemon (currently `crates/nv-daemon` in Rust) is replaced by a TypeScript Node.js process.
The Next.js dashboard at `apps/dashboard/` stays and connects to the new TS daemon.

## Architecture

```
Telegram (node-telegram-bot-api) ──┐
Discord (discord.js) ──────────────┤
Teams (webhook) ───────────────────┤──> Nova TS Daemon (Node.js, systemd)
CLI ───────────────────────────────┘    │
                                        ├── Agent SDK (query() per message)
                                        │     ├── Claude Code MAX (OAuth, no API key)
                                        │     ├── Vercel AI Gateway (observability)
                                        │     └── Built-in tools (Read, Write, Bash, WebSearch)
                                        │
                                        ├── CLI Tool Wrappers (MCP or Bash)
                                        │     ├── teams-cli (MS Graph: chats, channels, messages)
                                        │     ├── outlook-cli (inbox, calendar)
                                        │     ├── ado-cli (pipelines, builds, work items)
                                        │     ├── az-cli (Azure resource management)
                                        │     └── discord-cli (guilds, channels, messages)
                                        │
                                        ├── Drizzle ORM → Postgres + pgvector (local Docker)
                                        │     ├── messages, obligations, contacts
                                        │     ├── cold_starts, diary, memory
                                        │     └── vector embeddings (pgvector)
                                        │
                                        └── HTTP API (for dashboard)

apps/dashboard (Next.js 15, Geist, existing)
  └── Proxies to Nova TS daemon API
```

## Rewrite Strategy

**Clean-room.** Start fresh in TypeScript. Don't port Rust code — redesign from scratch using
the Agent SDK as the core orchestration layer. The Rust daemon runs in parallel until the TS
version reaches feature parity, then is decommissioned.

### Core Principle: Agent SDK is the Brain

Every inbound message becomes an `Agent SDK query()` call with:
- Nova's system prompt
- Conversation history from Postgres
- Tools: Claude Code built-ins + CLI wrappers accessible via Bash
- `permission_mode: "bypassPermissions"` (Nova is autonomous)

The Agent SDK handles the tool loop, streaming, retry — Nova's daemon is just the message
router and state persistence layer.

## v9 Must-Do

### Phase 1: Foundation
1. **Postgres + Drizzle schema** — messages, obligations, contacts, diary, memory tables
2. **Docker Compose for Postgres** — local pg + pgvector, no cloud dependency
3. **Telegram adapter** — node-telegram-bot-api, polling loop, message → Agent SDK query()
4. **Agent SDK integration** — query() per message with system prompt + conversation history
5. **HTTP API** — Express/Hono for dashboard proxy + health endpoint
6. **systemd service** — bare metal deployment, PM2 or systemd

### Phase 2: Tool Wrappers
7. **teams-cli** — TypeScript CLI wrapping MS Graph API for chats/channels/messages
8. **outlook-cli** — inbox reading, calendar events
9. **ado-cli** — pipelines, builds, work items
10. **az-cli** — leverage existing Azure CLI, document for Nova's system prompt
11. **discord-cli** — guild/channel/message access
12. **Tool documentation** — system prompt sections describing each CLI + usage patterns

### Phase 3: Features
13. **Obligation system** — detection, storage, autonomous execution, proposed_done
14. **Proactive watcher** — scheduled scans for overdue/stale obligations
15. **Diary** — interaction logging per message
16. **Morning briefing** — scheduled digest with source aggregation
17. **Memory** — topic-based read/write files (or Postgres-backed with pgvector)

### Phase 4: Dashboard Integration
18. **API parity** — all dashboard proxy routes work with new daemon
19. **WebSocket events** — obligation activity, session updates
20. **Obligation CRUD** — /ob Telegram commands

## v9 Won't-Do

- Porting Rust code line-by-line (clean-room, not port)
- Multiple simultaneous channel adapters in Phase 1 (Telegram first, others in Phase 2)
- Voice STT/TTS (defer to v10 — works independently of core)
- Dashboard redesign (Geist design from v8 stays)
- Dashboard authentication (Tailscale sufficient)
- Streaming Telegram edits (nice-to-have, not Phase 1)

## Data Layer

**Postgres + pgvector via local Docker.** Drizzle ORM for type-safe schema.

```yaml
# docker-compose.yml (alongside existing dashboard)
postgres:
  image: pgvector/pgvector:pg17
  ports: ["5432:5432"]
  volumes: ["nova-pg-data:/var/lib/postgresql/data"]
  environment:
    POSTGRES_DB: nova
    POSTGRES_USER: nova
    POSTGRES_PASSWORD: nova-local
```

Schema migration from SQLite: one-time script reads SQLite files and imports into Postgres.

## Tool Strategy

**Claude Code built-in tools + documented CLI wrappers.**

Nova's system prompt describes available CLI tools. When Nova needs Teams data, she calls:
```bash
teams-cli chats --limit 20
teams-cli read-chat <chat-id> --limit 50
```

via Claude Code's `Bash` tool. No MCP needed initially — CLI wrappers invoked via Bash are
simpler and work immediately.

The CLI wrappers are standalone TypeScript executables in `packages/tools/`:
```
packages/tools/
  teams-cli/     — MS Graph: chats, channels, messages, presence
  outlook-cli/   — Outlook: inbox, calendar
  ado-cli/       — ADO: pipelines, builds, work items
  discord-cli/   — Discord: guilds, channels, messages
```

Each reads credentials from Doppler or environment variables.

## Deployment

**Bare metal with systemd** (same as current Rust daemon).

- Node.js 22 (already installed via mise)
- Agent SDK spawns Claude Code CLI as subprocess — needs host access to `~/.claude/`
- Vercel AI Gateway: set `ANTHROPIC_BASE_URL=https://ai-gateway.vercel.sh` in environment
- Claude MAX subscription for model access (no API key)

## Business Model

Personal tool. No monetization. Homelab deployment.

## Scale Target

1 user (Leo), ~50-100 messages/day, ~5 concurrent Agent SDK sessions.

## Hard Constraints

- Tailscale-only network (no public exposure)
- Doppler for secrets
- Postgres local (Docker on homelab, not cloud)
- Claude MAX subscription (no separate API billing)
- Vercel AI Gateway for observability
- Bare metal systemd (not Docker for the daemon)

## Timeline

No external deadline. Start after v8 stabilization.

## Assumptions Corrected

- v8 assumed Rust was the right language → TS gives Agent SDK + AI Gateway natively
- v8 used raw Anthropic HTTP API → Agent SDK handles auth, tools, streaming
- v8 had 95+ custom tool implementations → CLI wrappers + Claude Code built-ins replace most
- v8 used SQLite → Postgres + pgvector enables vector search and proper transactions
- v8 Python sidecar was a workaround → clean TS rewrite eliminates the sidecar
