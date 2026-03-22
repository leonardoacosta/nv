# PRD — NV (Master Agent Harness)

> Consolidated from: scope-lock.md, user-stories.md, wireframes/
> Generated: 2026-03-21

---

## 1. Problem Statement

Leo operates across 14+ projects, 6+ communication channels (Telegram, Discord, Teams, Email,
Slack, iMessage), Jira, and multiple Claude Code sessions on a Tailscale-connected homelab. Tasks
fall through cracks. Important messages get buried. Context is scattered across systems with no
unified view.

**Core problem:** No single system watches everything, triages what matters, and acts on Leo's
behalf with confirmation.

## 2. Vision

NV is a proactive, always-on Rust daemon that:
- Monitors all communication channels
- Triages messages using Claude as the reasoning engine
- Manages Jira issues (create, transition, query)
- Sends Telegram notifications and accepts commands
- Maintains cross-session memory for context continuity
- Interfaces with Nexus for Claude Code session awareness

**Not a chatbot.** NV is a task-focused orchestrator that sleeps until needed, then acts.

## 3. Target User

Leo. Solo operator. Power user. No other users — ever. No multi-tenancy, no permissions,
no onboarding.

## 4. Execution Model

**Event-driven daemon.** The agent loop is dormant until triggered:

| Trigger | Source | Response |
|---------|--------|----------|
| Telegram message | Long-poll listener | Instant wake → process command/query |
| Channel message | Channel adapter | Instant wake → triage → notify if actionable |
| Cron tick | tokio interval | Scheduled → digest → notify |
| Nexus event | gRPC stream | Instant wake → update context → notify if significant |
| CLI command | IPC/HTTP | Instant wake → process → respond |

Channel listeners are always-on tokio tasks feeding an `mpsc` channel. Agent loop `recv()`s —
dormant when empty, wakes instantly on message. Messages queue while agent is processing.

## 5. Autonomy Model

**Suggest + Confirm.** NV drafts actions and presents via Telegram inline keyboard. Leo taps
to approve, edit, or dismiss. No autonomous writes to external systems.

Autonomous (no confirmation): read operations, memory updates, context gathering.

## 6. Interaction Modes

### 6.1 Commander (Direct Commands)

Leo messages NV on Telegram with an instruction. NV interprets via Claude, drafts the action
(Jira issue, status transition, channel reply), and presents for confirmation.

**Example:** "Create a P1 bug for the checkout crash on OO" →
NV drafts issue → Leo taps ✅ → NV creates in Jira → confirms via Telegram.

### 6.2 Consumer (Proactive Digest)

NV runs a configurable cron loop (default: 60min). Gathers context from Jira, Nexus, memory,
and channel history. Claude synthesizes a digest. Sent to Telegram with suggested actions as
inline keyboard buttons.

**Example:** Morning digest → 3 Jira issues need attention, 1 Teams message unanswered,
2 sessions running → Leo picks which actions to execute.

### 6.3 Querier (Context Questions)

Leo asks NV questions via Telegram. NV searches across Jira, Nexus sessions, memory, and
channel history. Claude synthesizes a cross-system answer.

**Example:** "What's blocking the OO release?" → NV queries Jira + Nexus + memory →
returns: "4 open issues, 1 P0 bug, 1 session running, Maria asked about timeline."

## 7. Architecture

### 7.1 Service Topology

```
┌─────────────────────────────────────────────────────┐
│  NV Daemon (Rust, systemd, ~/nv/)                   │
│                                                     │
│  ┌──────────────┐                                   │
│  │ Channel       │──── Telegram long-poll (always-on)│
│  │ Listeners     │──── Discord gateway (P1)          │
│  │ (tokio tasks) │──── Teams webhook (P1)            │
│  └──────┬───────┘──── iMessage relay (P1)            │
│         │ mpsc::channel                              │
│         ▼                                            │
│  ┌──────────────┐     ┌────────────────────┐         │
│  │ Agent Loop    │────│ Claude API (reqwest)│         │
│  │ (dormant until│     └────────────────────┘         │
│  │  triggered)   │                                   │
│  └──────┬───────┘                                    │
│         │                                            │
│  ┌──────┴───────┐                                    │
│  │ Integrations  │                                   │
│  │ ├ Jira REST   │──── leonardoacosta.atlassian.net  │
│  │ ├ Nexus gRPC  │──── homelab:7400, macbook:7400    │
│  │ ├ TTS HTTP    │──── claude-daemon :9999            │
│  │ └ Memory FS   │──── ~/.nv/memory/                 │
│  └──────────────┘                                    │
│                                                      │
│  ┌──────────────┐                                    │
│  │ Cron Scheduler│──── Digest every N minutes        │
│  └──────────────┘                                    │
└─────────────────────────────────────────────────────┘
```

### 7.2 Crate Structure

```
nv/
├── Cargo.toml              # Workspace
├── crates/
│   ├── nv-core/            # Types, config, message model, Channel trait
│   ├── nv-daemon/          # Binary: listeners + agent loop + scheduler
│   └── nv-cli/             # Binary: nv status, nv ask, nv config
├── proto/                  # Nexus protobuf (re-export or submodule)
├── config/nv.example.toml
└── deploy/nv.service       # systemd unit
```

### 7.3 Key Abstractions

**Channel trait:**
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
    channel: String,
    sender: String,
    content: String,
    timestamp: DateTime<Utc>,
    thread_id: Option<String>,
    metadata: serde_json::Value,
}
```

**Trigger enum:**
```rust
enum Trigger {
    Message(InboundMessage),
    Cron(CronEvent),        // digest, cleanup
    NexusEvent(SessionEvent),
    CliCommand(CliRequest),
}
```

## 8. Channels (Phased)

| Channel | Protocol | Phase | Notes |
|---------|----------|-------|-------|
| Telegram | Bot API (HTTP long poll) | Weekend MVP | Primary command/notify |
| iMessage | BlueBubbles / Mac relay | P1 (next week) | Via existing infra |
| Discord | Gateway WebSocket + REST | P1 (next week) | Team communication |
| Teams | MS Graph REST + webhooks | P1 (next week) | Work communication |
| Email/Outlook | MS Graph REST | P2 (later) | Email triage |
| Slack | Events API + REST | P2 (later) | Workspace monitoring |

## 9. Jira Integration

**Weekend (v1):** NV→Jira. Create, read, transition, assign, comment via REST API v3.
Agent drafts, Leo confirms via Telegram.

**Next week (v1.1):** Jira→NV webhook. Bidirectional sync — external changes reflected
in NV's memory.

**Credentials:** `JIRA_API_TOKEN` + `JIRA_USERNAME` via Doppler. Instance:
`leonardoacosta.atlassian.net`.

## 10. Nexus Integration

NV connects to Nexus agent(s) via gRPC (:7400):
- `GetSessions` — query running sessions
- `GetSession` — session detail
- `StreamEvents` — real-time session lifecycle events
- `StartSession` / `StopSession` — optional session control

Session data feeds into digests, query responses, and context for task prioritization.

## 11. Memory

**Location:** `~/.nv/memory/`

**Format:** Markdown files, grep-searchable.

```
~/.nv/
├── nv.toml                 # Config
├── memory/
│   ├── MEMORY.md           # Index
│   ├── conversations.md    # Conversation summaries
│   ├── tasks.md            # Active task context
│   ├── decisions.md        # Decisions made
│   └── people.md           # Who's who
└── state/
    ├── last-digest.json    # Last digest timestamp
    ├── pending-actions.json # Actions awaiting confirmation
    └── channel-state.json  # Per-channel cursor
```

**Weekend quality:** File-based, grep search. Embeddings/vector DB deferred.

## 12. Tech Stack

| Layer | Technology |
|-------|-----------|
| Runtime | Rust + tokio |
| HTTP Client | reqwest |
| gRPC Client | tonic (protobuf) |
| AI | Anthropic Messages API (reqwest) |
| Config | TOML (serde) |
| CLI | clap 4 |
| Logging | tracing + tracing-subscriber |
| Errors | anyhow + thiserror |
| Time | chrono |
| Service | systemd |

## 13. Configuration

```toml
# ~/.nv/nv.toml
[agent]
model = "claude-sonnet-4-6"
think = true
digest_interval_minutes = 60

[telegram]
chat_id = 123456789

[jira]
instance = "leonardoacosta.atlassian.net"
default_project = "OO"

[nexus]
agents = [
    { name = "homelab", host = "homelab", port = 7400 },
    { name = "macbook", host = "macbook", port = 7400 },
]

[daemon]
tts_url = "http://100.91.88.16:9999"
```

Secrets via Doppler env vars: `ANTHROPIC_API_KEY`, `TELEGRAM_BOT_TOKEN`, `JIRA_API_TOKEN`,
`JIRA_USERNAME`.

## 14. Constraints

- Secrets: Doppler only
- Hosting: Linux homelab (systemd)
- Data: All local, no cloud sync
- Network: Tailscale
- AI: Claude-only
- Single user: No auth

## 15. Timeline

| Phase | When | Deliverables |
|-------|------|-------------|
| **Weekend MVP** | Sat–Sun | Telegram bot + agent loop + Jira (NV→Jira) + digest + query + memory |
| **P1** | Next week | iMessage, Discord, Teams channels + bidirectional Jira |
| **P2** | Week 3+ | Email/Outlook, Slack, richer memory (embeddings) |

### Weekend Breakdown

- **Saturday:** Cargo workspace + nv-core types + Telegram listener + agent loop (Claude API) + basic memory
- **Sunday:** Jira REST client + proactive digest cron + context query + Telegram inline keyboards

## 16. Success Criteria

NV is successful when Leo can:
1. Message NV on Telegram and get a Jira issue created with one tap
2. Receive a morning digest without checking 6 apps
3. Ask "What's blocking X?" and get a cross-system answer
4. See session status from Nexus in the digest
5. Trust that nothing actionable slips through the cracks

## 17. Ambiguity Audit

| Question | Resolution |
|----------|-----------|
| How does NV handle rate limits on Claude API? | Weekend: ignore. Later: backoff + queue. |
| What if Telegram long-poll drops? | Reconnect loop with exponential backoff. |
| How much context fits in Claude's window? | Start with recent messages + memory summaries. Truncate oldest first. |
| How does NV handle conflicting triggers? | Queue via mpsc. Process sequentially. No parallelism in agent loop. |
| What if Jira API is down? | Retry 3x with backoff. Notify Leo on Telegram. Store action in pending. |
| How does NV distinguish command vs query vs chat? | Claude classifies intent from message content. No prefix commands needed. |
| What about Telegram message ordering? | Long-poll returns in order. Process FIFO from mpsc queue. |
| How does nv-cli talk to nv-daemon? | HTTP on localhost (simple, debuggable). Unix socket later if needed. |
