# Context: NV — Master Claude Agent Harness

## Context Tag

system

## Description

A master Claude agent harness inspired by OpenClaw's architecture: TypeScript gateway control plane
with WebSocket coordination, unified channel abstraction over messaging platforms, plugin SDK,
markdown-native cross-session memory, and proactive task management with Jira integration and
Telegram notifications.

## Reference Architecture: OpenClaw

**OpenClaw** (github.com/openclaw/openclaw) — 328K stars, 500+ contributors.
Self-hosted personal AI assistant with gateway control plane pattern.

### Key Patterns to Emulate

| Pattern | OpenClaw Implementation | NV Adaptation |
|---------|------------------------|---------------|
| **Gateway Control Plane** | WebSocket server (ws://127.0.0.1:18789) | WebSocket gateway, leverage existing session-daemon patterns |
| **Channel Abstraction** | 21+ channels with unified interface (typing, reactions, threading) | Start with 4-6 channels, same unified interface |
| **Plugin SDK** | In-process plugins with testing surface | Plugin SDK for custom integrations |
| **Markdown-Native Memory** | `MEMORY.md` + `memory/*.md` searchable index | Extend Claude Code's existing memory system |
| **Lean Core + Plugins** | Gateway + Agent + Channel = core; rest = plugins | Same: gateway + agent loop + channels = core |
| **Local-First Daemon** | systemd service, no cloud sync | systemd, Doppler for secrets |
| **Event-Driven Streaming** | Tool results as streams, incremental UI updates | Stream events to channels in real-time |
| **Cron Tasks** | Background scheduled execution | Proactive task scheduling loop |

### OpenClaw Architecture (Reference)

```
Gateway (WebSocket Control Plane)
├── Session Management
├── Channel Routing
├── Tool Execution
├── Event Broadcasting
│
├── Channels (unified abstraction)
│   ├── Telegram (grammy)
│   ├── Discord (discord.js)
│   ├── Slack (Bolt)
│   ├── Teams (MS Graph)
│   ├── WhatsApp (Baileys)
│   ├── Email (IMAP/MS Graph)
│   └── 15+ more
│
├── Agent Runtime (RPC)
│   ├── Claude API / Anthropic SDK
│   ├── Tool execution pipeline
│   └── Context engine
│
├── Memory (Markdown-native)
│   ├── MEMORY.md (index)
│   ├── memory/*.md (topics)
│   └── Searchable index
│
├── Plugin SDK
│   ├── Tool registration
│   ├── Channel registration
│   └── Testing surface
│
└── Cron Scheduler
    └── Proactive background tasks
```

### OpenClaw Tech Stack

- **Runtime**: Node.js 24+ / TypeScript (strict)
- **Package Manager**: pnpm
- **Messaging**: grammy (Telegram), discord.js, Bolt (Slack), matrix-js-sdk
- **AI**: Anthropic SDK, OpenAI, Google Gemini, OpenRouter
- **Browser**: Playwright / CDP
- **Voice**: ElevenLabs TTS, Deepgram STT
- **Testing**: Vitest + Playwright
- **Build**: tsdown bundler
- **Config**: JSON config file (`~/.openclaw/openclaw.json`)

## Codebase Structure

**Project:** `nv` (greenfield — empty directory at `~/nv/`)

### Proposed Monorepo Structure (OpenClaw-inspired)

```
nv/
├── src/
│   ├── gateway/           # WebSocket control plane
│   ├── agent/             # Agent runtime (Claude API loop)
│   ├── channels/          # Channel abstraction layer
│   │   ├── telegram/      # grammy
│   │   ├── discord/       # discord.js
│   │   ├── teams/         # MS Graph API
│   │   ├── slack/         # Bolt
│   │   ├── email/         # IMAP / MS Graph
│   │   └── terminal/      # Local stdin/stdout
│   ├── memory/            # Markdown-native persistence
│   ├── tasks/             # Jira + proactive task management
│   ├── cron/              # Background scheduler
│   ├── config/            # Configuration management
│   ├── plugin-sdk/        # Plugin development kit
│   └── notifications/     # Outbound status (Telegram, TTS)
├── plugins/               # Built-in plugins
├── skills/                # Skill definitions
├── tests/                 # Vitest + integration tests
├── package.json
├── tsconfig.json
└── nv.json                # Runtime config (~/.nv/nv.json)
```

## Related Work

### Existing Infrastructure (Reusable Components)

| Component | Location | Reuse Strategy |
|-----------|----------|----------------|
| **Session Daemon** (TS) | `~/dev/co/apps/session-daemon/` | Port WebSocket protocol, session lifecycle patterns |
| **claude-daemon** (Rust) | `~/.claude/scripts/bin/claude-daemon` | Keep running separately; NV gateway coexists |
| **Jira Sync** | `~/.claude/scripts/bin/jira-sync` | Port logic into NV tasks module |
| **TTS/Notifications** | `~/.claude/scripts/bin/claude-notify` | Call as external service (HTTP :9999) |
| **Memory System** | `~/.claude/projects/*/memory/` | Extend pattern into NV markdown memory |
| **Beads** | `.beads/` per-project | Optional integration via `bd` CLI |
| **Doppler** | Secrets management | Same pattern: env vars via systemd EnvironmentFile |
| **Agent Registry** | `~/.claude/agents/` | Reference for agent specialization patterns |

### Session Daemon Patterns to Port

From `~/dev/co/apps/session-daemon/`:
- `daemon-protocol.ts` — Zod schemas for WebSocket message protocol
- `sdk-session-manager.ts` — Anthropic Agent SDK session launcher
- `process-manager.ts` — Process lifecycle tracking (pending → running → completed/failed)
- `session-watcher.ts` — JSONL monitoring, change detection
- `cost-aggregator.ts` — Session cost tracking
- `db.ts` — SQLite persistence

### Key Differences from OpenClaw

| Aspect | OpenClaw | NV |
|--------|----------|-----|
| **Scope** | General-purpose assistant | Task-focused orchestrator |
| **Channels** | 21+ (community-driven) | 4-6 (curated for Leo's workflow) |
| **Agent** | Pi (multi-model) | Claude-only (Anthropic SDK) |
| **Memory** | Generic MEMORY.md | Integrated with Claude Code memory + project context |
| **Task Management** | None (left to plugins) | Core feature: Jira integration, proactive organization |
| **Notifications** | Via channels | Telegram bot + existing TTS system |
| **Plugin Ecosystem** | ClawHub marketplace | Local plugins for personal workflow |

## Mode-Specific Findings (System Architecture)

### Core Build (OpenClaw-aligned)

| Component | OpenClaw Pattern | NV Implementation | Effort |
|-----------|-----------------|-------------------|--------|
| **Gateway** | WebSocket control plane | Port from session-daemon + extend | Medium |
| **Channel Abstraction** | Unified interface over 21+ | Unified interface over 4-6 | Medium |
| **Agent Loop** | RPC mode with tool execution | Claude SDK loop with tool pipeline | Medium |
| **Memory** | MEMORY.md + memory/*.md | Same pattern + Claude Code integration | Low |
| **Config** | openclaw.json | nv.json with channel configs | Low |
| **Plugin SDK** | In-process with testing | Simplified in-process plugin loading | Medium |
| **Cron** | Background scheduler | Proactive task loop (configurable interval) | Low-Medium |

### Channel Priority (NV-specific)

| Channel | SDK | Priority | Why |
|---------|-----|----------|-----|
| **Telegram** | grammy | P0 | Primary notification + command channel |
| **Discord** | discord.js | P1 | Team communication monitoring |
| **Teams** | MS Graph API | P1 | Work communication monitoring |
| **Email/Outlook** | MS Graph API | P2 | Email triage + task extraction |
| **Slack** | Bolt | P2 | Workspace monitoring |
| **Terminal** | stdin/stdout | P0 | Local development / debugging |

### Secrets to Provision (Doppler)

| Key | Source | Priority |
|-----|--------|----------|
| `ANTHROPIC_API_KEY` | Already exists | P0 |
| `JIRA_API_TOKEN` | Already exists | P0 |
| `TELEGRAM_BOT_TOKEN` | @BotFather | P0 |
| `DISCORD_BOT_TOKEN` | Discord developer portal | P1 |
| `MS_GRAPH_CLIENT_ID` + `MS_GRAPH_CLIENT_SECRET` | Azure AD | P1 |
| `SLACK_BOT_TOKEN` | Slack app OAuth | P2 |

### Proactive Task Management (NV-unique feature)

OpenClaw doesn't have built-in task management. NV's differentiator:

1. **Message Scanning** — Channels deliver messages to agent loop
2. **Task Extraction** — Agent identifies actionable items from conversations
3. **Jira Management** — Create/update/transition Jira issues automatically
4. **Context Resolution** — Agent searches codebase + memory for solutions
5. **Status Updates** — Telegram notifications with progress
6. **Scheduling** — Cron-based proactive task review loop

## Discovery Metadata

- **Project**: nv (greenfield)
- **Path**: /home/nyaptor/nv
- **Timestamp**: 2026-03-21T19:10:00-05:00
- **Updated**: 2026-03-21T19:30:00-05:00 (OpenClaw reference added)
- **Quick mode**: false
- **Detected mode**: system
- **Reference**: OpenClaw (github.com/openclaw/openclaw)
- **Stack**: TypeScript, Node.js, pnpm, WebSocket gateway, channel abstraction
