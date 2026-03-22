# Scope Lock — NV (Master Agent Harness)

## Vision

A proactive, always-on Rust daemon that monitors communication channels, triages messages,
manages Jira, and keeps Leo informed via Telegram — interfacing with Nexus for session
awareness and claude-daemon for TTS.

## Target Users

Leo. Solo operator. No multi-tenancy, no onboarding, no external users — ever.

## Domain

**In scope:** Communication channel monitoring, proactive task extraction, Jira lifecycle
management, Telegram command/notification interface, cross-session markdown memory, Nexus
integration for session awareness.

**Out of scope:** Replacing Nexus (session dashboard), replacing claude-daemon (TTS/health),
web UI, mobile apps, multi-model support, plugin marketplace.

## Differentiator

Unlike OpenClaw (general-purpose assistant with 21+ channels), NV is a task-focused orchestrator
that proactively organizes work — not a chatbot. It watches, triages, acts (with confirmation),
and reports. The agent loop is event-driven, not polling — it sleeps until woken.

## Execution Model

**Event-driven, not polling.** The daemon runs continuously, but the agent loop (Claude API) is
dormant until triggered. This avoids wasted API calls and ensures instant response to messages.

**Triggers that wake the agent loop:**

| Trigger | Source | Latency |
|---------|--------|---------|
| Inbound Telegram message | Long-poll listener (always-on) | Instant |
| Inbound channel message | Channel adapter (always-on) | Instant |
| Cron tick (digest) | tokio interval timer | Scheduled |
| Nexus session event | gRPC stream (always-on) | Instant |
| CLI command (`nv ask`) | IPC / HTTP | Instant |

**State machine:**

```
                  ┌──────────────────────────────────────┐
                  │                                      │
    Dormant ──(trigger)──→ Processing ──(done)──→ Drain Queue
       ▲                       │                     │
       │                       │ queue inbound       │ more items?
       │                       │ while busy          ├── yes → Processing
       └───────────────────────┘                     └── no  → Dormant
```

**Key:** Channel listeners (Telegram long-poll, Discord gateway, Nexus gRPC stream) are separate
tokio tasks that run continuously. They push messages onto an `mpsc` channel. The agent loop
`recv()`s from that channel — blocking when empty (dormant), waking instantly on new message.

**Not OpenClaw style:** OpenClaw processes messages synchronously per-request. NV's agent loop is
a shared resource — multiple triggers feed into one queue, processed sequentially with full context.

## Service Topology

```
┌─────────────────────────────────────────────┐
│  NV (this project)                          │
│  Standalone Rust binary — ~/nv/             │
│  systemd service — nv.service               │
│                                             │
│  ┌─────────────┐  ┌──────────────────────┐  │
│  │ Gateway     │  │ Agent Loop           │  │
│  │ (channel    │──│ Claude API           │  │
│  │  routing)   │  │ Tool pipeline        │  │
│  └─────────────┘  │ Memory (markdown)    │  │
│        │          └──────────────────────┘  │
│        │                    │               │
│  ┌─────┴──────┐    ┌────────┴────────┐      │
│  │ Channels   │    │ Integrations    │      │
│  │ ├ Telegram │    │ ├ Jira (REST)   │      │
│  │ ├ Discord  │    │ ├ Nexus (gRPC)  │      │
│  │ ├ Teams    │    │ └ claude-daemon  │      │
│  │ ├ Email    │    │   (HTTP :9999)  │      │
│  │ └ Slack    │    └─────────────────┘      │
│  └────────────┘                              │
└─────────────────────────────────────────────┘
         │                    │
    ┌────┴─────┐        ┌─────┴──────┐
    │ External │        │ Internal   │
    │ APIs     │        │ Services   │
    │ Telegram │        │ Nexus:7400 │
    │ Discord  │        │ daemon:9999│
    │ MS Graph │        └────────────┘
    │ Jira     │
    │ Slack    │
    └──────────┘
```

## Autonomy Model

**Suggest + Confirm.** NV drafts actions (Jira issues, status transitions, summaries) and
presents them via Telegram for Leo's approval before executing. No autonomous writes to
external systems without confirmation.

Exception: Read operations and memory updates are autonomous.

## Channel Architecture

**Input flow:** All messages from all connected sources flow into NV. The agent triages —
decides what's relevant based on context, memory, and current task state. No pre-filtering
by channel/keyword.

**Channels (full list, phased):**


| Channel       | Protocol                 | Weekend MVP? | Notes                                               |
| ------------- | ------------------------ | ------------ | --------------------------------------------------- |
| Telegram      | Bot API (HTTP long poll) | Yes          | Primary command/notify                              |
| iMessage      | BlueBubbles / Mac relay  | No (P1)      | Via existing claude-daemon infra or BlueBubbles API |
| Discord       | Gateway WebSocket + REST | No (P1)      | Next week                                           |
| Teams         | MS Graph REST + webhooks | No (P1)      | Next week                                           |
| Email/Outlook | MS Graph REST            | No (P2)      | Later                                               |
| Slack         | Events API + REST        | No (P2)      | Later                                               |


**Channel trait (Rust):**

```rust
#[async_trait]
trait Channel: Send + Sync {
    fn name(&self) -> &str;
    async fn connect(&mut self) -> Result<()>;
    async fn poll_messages(&self) -> Result<Vec<InboundMessage>>;
    async fn send_message(&self, msg: OutboundMessage) -> Result<()>;
    async fn disconnect(&mut self) -> Result<()>;
}
```

**Unified message model:**

```rust
struct InboundMessage {
    id: String,
    channel: String,          // "telegram", "discord", "teams", etc.
    sender: String,
    content: String,
    timestamp: DateTime<Utc>,
    thread_id: Option<String>,
    metadata: serde_json::Value,
}
```

## v1 Must-Do (Weekend MVP)

**The full loop, Telegram-only:**

1. **Telegram bot** — send/receive messages via Bot API (reqwest, long polling)
2. **Agent loop** — Claude API call with context window (basic prompt + memory)
3. **Jira integration** — create issues, read status, transition (NV→Jira first)
4. **Proactive digest** — periodic scan, Telegram summary of what needs attention
5. **Context query** — Leo asks NV questions via Telegram, gets answers from memory
6. **Memory** — simple markdown files (`~/.nv/memory/`), searchable by grep

**Quality bar for weekend:** Functional > elegant. Ship the loop, polish later.

## v1 Won't-Do (Next Week+)

- iMessage channel (P1 — via BlueBubbles API or Mac relay, next week)
- Discord channel (P1 — next)
- Teams/MS Graph integration (P1 — next)
- Email/Outlook monitoring (P2)
- Slack integration (P2)
- Jira→NV webhook sync (bidirectional, P1 next week)
- Embedding-based memory search (P2)
- Plugin SDK (P3 — maybe never for single-user tool)
- Web dashboard (never — Nexus TUI is the dashboard)
- TUI interface (Nexus handles this)

## Jira Integration

**Weekend (v1):** NV→Jira only. Create, read, transition, assign, comment via REST API.
Agent drafts, Leo confirms via Telegram.

**Next week (v1.1):** Jira→NV webhook inbound. When someone else changes an issue, NV knows.

**Credentials:** `JIRA_API_TOKEN` + `JIRA_USERNAME` already in Doppler. Instance:
`leonardoacosta.atlassian.net`.

## Nexus Integration

NV connects to Nexus agent(s) via gRPC (:7400) to:

- Query running sessions (`GetSessions`)
- Get session details (`GetSession`)
- Stream events (`StreamEvents`) for awareness of what's happening
- Optionally start/stop sessions (`StartSession`, `StopSession`)

This gives NV context: "3 sessions running on homelab, 1 is applying spec X, 1 is idle."
NV can report this in Telegram digests and use it for task prioritization.

## Memory Architecture

**Location:** `~/.nv/memory/`

**Format:** Markdown files (OpenClaw-inspired, Claude Code memory-aligned)

```
~/.nv/
├── nv.toml                    # Runtime config (channels, schedules, Jira)
├── memory/
│   ├── MEMORY.md              # Index
│   ├── conversations.md       # Key conversation summaries
│   ├── tasks.md               # Active task context
│   ├── decisions.md           # Decisions made
│   └── people.md              # Who's who
└── state/
    ├── last-digest.json       # Last digest timestamp + content
    ├── pending-actions.json   # Actions awaiting Leo's confirmation
    └── channel-state.json     # Per-channel cursor/offset
```

**Weekend quality:** File-based, grep-searchable. No embeddings, no vector DB.

## Tech Stack


| Layer             | Technology                   | Why                                                    |
| ----------------- | ---------------------------- | ------------------------------------------------------ |
| **Runtime**       | Rust + tokio                 | Matches Nexus, matches claude-daemon, 24/7 reliability |
| **HTTP Client**   | reqwest                      | Telegram Bot API, Jira REST, claude-daemon health      |
| **gRPC Client**   | tonic                        | Connect to Nexus agents                                |
| **AI**            | Anthropic API (reqwest)      | Claude for agent reasoning                             |
| **Config**        | toml (serde)                 | Matches Nexus config pattern                           |
| **Serialization** | serde + serde_json           | Standard                                               |
| **CLI**           | clap 4                       | Subcommands (start, config, status)                    |
| **Logging**       | tracing + tracing-subscriber | Matches Nexus pattern                                  |
| **Errors**        | anyhow + thiserror           | Matches Nexus pattern                                  |
| **Time**          | chrono                       | Matches Nexus pattern                                  |
| **Service**       | systemd                      | Matches claude-daemon + nexus-agent                    |


## Crate Structure (Weekend MVP)

```
nv/
├── Cargo.toml                 # Workspace
├── crates/
│   ├── nv-core/               # Shared types, message model, config
│   ├── nv-daemon/             # Main binary: gateway + agent loop + scheduler
│   └── nv-cli/                # CLI: nv status, nv config, nv ask "question"
├── proto/                     # Nexus proto re-export (or git submodule)
├── config/
│   └── nv.example.toml
└── deploy/
    └── nv.service             # systemd unit
```

## Configuration

```toml
# ~/.nv/nv.toml

[agent]
model = "claude-sonnet-4-6"      # Default model for agent loop
think = true                       # Enable extended thinking
digest_interval_minutes = 60       # Proactive digest every hour

[telegram]
# Bot token from Doppler: TELEGRAM_BOT_TOKEN
chat_id = 123456789                # Leo's chat ID

[jira]
# Credentials from Doppler: JIRA_API_TOKEN, JIRA_USERNAME
instance = "leonardoacosta.atlassian.net"
default_project = "OO"

[nexus]
agents = [
    { name = "homelab", host = "homelab", port = 7400 },
    { name = "macbook", host = "macbook", port = 7400 },
]

[daemon]
tts_url = "http://100.91.88.16:9999"   # claude-daemon TTS endpoint
```

## Hard Constraints

- **Secrets:** Doppler only. No .env files, no hardcoded tokens.
- **Hosting:** Linux homelab (systemd). No cloud deployment.
- **Data:** All local. No cloud sync. Memory files on disk.
- **Network:** Tailscale for inter-machine communication.
- **AI:** Claude-only. No multi-model abstraction.
- **Single user:** No auth, no permissions, no multi-tenancy.

## Timeline

- **Saturday:** Gateway + Telegram bot + agent loop + basic memory
- **Sunday:** Jira integration + proactive digest + context query
- **Next week:** Discord, Teams, bidirectional Jira, richer memory

## Assumptions Corrected

- ~~TypeScript~~ → **Rust** (all channels are HTTP REST, no SDK dependency)
- ~~Extend claude-daemon~~ → **Standalone binary** (separate concerns, interfaces via HTTP/gRPC)
- ~~Plugin SDK in MVP~~ → **Never** (single-user tool, extensibility via code changes)
- ~~Filtered channels~~ → **Everything flows in, agent triages** (smarter, simpler config)
- ~~OpenClaw port~~ → **OpenClaw-inspired patterns** (channel trait, memory format, lean core)

