# Roadmap — NV Implementation Specs

> Generated from PRD. Each spec maps to an `/apply`-able OpenSpec change.
> Weekend MVP = Specs 1-6. P1 = Specs 7-10.

---

## Wave 1: Foundation (Saturday morning)

### Spec 1: `cargo-workspace-scaffold`

**Type:** task | **Effort:** S | **Deps:** none

Scaffold the Cargo workspace with 3 crates. No logic — just compilable structure.

- Workspace `Cargo.toml` with members
- `nv-core`: lib crate with placeholder types (Config, InboundMessage, Trigger, Channel trait)
- `nv-daemon`: binary crate with `main.rs` (tokio::main, tracing init, config load)
- `nv-cli`: binary crate with clap subcommands (status, ask, config, digest)
- `config/nv.example.toml` with all sections
- `deploy/nv.service` systemd unit file
- Shared dependencies: tokio, serde, anyhow, thiserror, tracing, chrono, reqwest, clap

**Gate:** `cargo build` succeeds for all 3 crates.

---

### Spec 2: `core-types-and-config`

**Type:** task | **Effort:** S | **Deps:** spec-1

Define the core type system in nv-core.

- `Config` struct (serde Deserialize from TOML): agent, telegram, jira, nexus, daemon sections
- `InboundMessage` struct (id, channel, sender, content, timestamp, thread_id, metadata)
- `OutboundMessage` struct (channel, content, reply_to, keyboard)
- `Trigger` enum (Message, Cron, NexusEvent, CliCommand)
- `Channel` trait (async_trait: name, connect, poll_messages, send_message, disconnect)
- `AgentResponse` enum (Reply, Action, Digest, Query)
- `PendingAction` struct (id, description, jira_payload, status)
- Config loading from `~/.nv/nv.toml` with env var override for secrets

**Gate:** `cargo test` — unit tests for config parsing, message serialization.

---

## Wave 2: Telegram + Agent Loop (Saturday afternoon)

### Spec 3: `telegram-channel`

**Type:** feature | **Effort:** M | **Deps:** spec-2

Implement Telegram Bot API channel adapter.

- Implement `Channel` trait for `TelegramChannel`
- Long-poll via `getUpdates` (reqwest POST to `api.telegram.org/bot<token>/getUpdates`)
- `send_message` via `sendMessage` with optional `reply_markup` (inline keyboard JSON)
- Handle `callback_query` for inline keyboard responses (answer + route to agent)
- Parse incoming messages into `InboundMessage`
- Reconnect loop with exponential backoff on poll failure
- Spawn as tokio task, push to `mpsc::Sender<Trigger>`

**Gate:** Send "hello" to bot on Telegram → bot echoes back.

---

### Spec 4: `agent-loop`

**Type:** feature | **Effort:** M | **Deps:** spec-2, spec-3

Implement the event-driven agent loop.

- `mpsc::channel<Trigger>` — all listeners push here
- Agent loop: `recv()` from channel (blocks when dormant)
- Batch drain: after first trigger, `try_recv()` to collect queued items
- Build Claude API request: system prompt + memory context + trigger batch
- Call Anthropic Messages API via reqwest (POST `api.anthropic.com/v1/messages`)
- Parse response into `AgentResponse` (reply, action, digest, query answer)
- Route response: reply → Telegram send_message, action → pending-actions.json
- System prompt defines NV's role, autonomy rules, available tools (Jira, Nexus, memory)
- Basic tool use: `read_memory`, `search_memory`, `query_jira`, `query_nexus`

**Gate:** Message NV on Telegram → Claude processes → meaningful response returned.

---

## Wave 3: Memory + Jira (Sunday morning)

### Spec 5: `memory-system`

**Type:** feature | **Effort:** S | **Deps:** spec-4

Implement markdown-native memory.

- Initialize `~/.nv/memory/` with MEMORY.md index + topic files
- `read_memory(topic)` — read specific memory file
- `search_memory(query)` — grep across all memory files, return matches with context
- `write_memory(topic, content)` — append to topic file, update MEMORY.md index
- `state/` directory: last-digest.json, pending-actions.json, channel-state.json
- Agent loop loads relevant memory into Claude context before each call
- Memory summarization: after N messages, Claude summarizes and compacts

**Gate:** Ask NV "remember that the Stripe fee is 5%" → ask "what's the Stripe fee?" → correct answer.

---

### Spec 6: `jira-integration`

**Type:** feature | **Effort:** M | **Deps:** spec-4, spec-5

Implement Jira REST API v3 client as agent tools.

- `JiraClient` struct with reqwest + auth (Basic: email + API token)
- Tool: `jira_search(jql)` — GET `/rest/api/3/search` → parsed issues
- Tool: `jira_create(project, type, title, description, priority, assignee, labels)` — POST `/rest/api/3/issue`
- Tool: `jira_transition(issue_key, transition_name)` — GET transitions → POST transition
- Tool: `jira_assign(issue_key, assignee)` — PUT `/rest/api/3/issue`
- Tool: `jira_comment(issue_key, body)` — POST `/rest/api/3/issue/{key}/comment`
- Tool: `jira_get(issue_key)` — GET `/rest/api/3/issue/{key}`
- All write operations require Telegram confirmation (PendingAction flow)
- Telegram inline keyboard: ✅ Create / ✏️ Edit / ❌ Cancel

**Gate:** "Create a P1 bug on OO" via Telegram → draft shown → confirm → issue exists in Jira.

---

## Wave 4: Digest + Query (Sunday afternoon)

### Spec 7: `proactive-digest`

**Type:** feature | **Effort:** M | **Deps:** spec-5, spec-6

Implement the cron-triggered proactive digest.

- Cron scheduler: tokio `interval` at `config.agent.digest_interval_minutes`
- On tick: push `Trigger::Cron(Digest)` to mpsc channel
- Agent loop handles digest trigger:
  - Gather: Jira open issues (my assignments), Nexus sessions, memory recent entries
  - Claude synthesizes digest with sections: Jira, Sessions, Channels, Suggested Actions
  - Send to Telegram as formatted message with inline keyboard for suggested actions
- Store digest in `state/last-digest.json` (timestamp, content hash, actions taken)
- `nv digest --now` CLI command triggers immediate digest via HTTP to daemon

**Gate:** Wait for cron tick → digest arrives on Telegram with real Jira data + action buttons.

---

### Spec 8: `context-query`

**Type:** feature | **Effort:** S | **Deps:** spec-5, spec-6

Implement cross-system context queries.

- Agent loop classifies intent: command vs query vs chat (Claude decides)
- For queries: parallel gather from Jira + memory + Nexus
- Claude synthesizes answer with source attribution
- Follow-up affordance: query response can trigger commands ("assign that to me")
- `nv ask "question"` CLI command → HTTP to daemon → same pipeline → stdout response

**Gate:** "What's blocking OO?" via Telegram → answer includes Jira issues + session status.

---

## Wave 5: Polish + Deploy (Sunday evening)

### Spec 9: `nexus-integration`

**Type:** feature | **Effort:** M | **Deps:** spec-4

Connect NV to Nexus via gRPC.

- Add tonic + prost to nv-daemon dependencies
- Include nexus.proto (copy or git submodule from ~/dev/nexus/proto/)
- `NexusClient`: connect to configured agents, handle partial connectivity
- Tool: `query_sessions()` → GetSessions RPC
- Tool: `query_session(id)` → GetSession RPC
- Background task: StreamEvents subscription → push `Trigger::NexusEvent` on significant events
- Session completion → Telegram notification
- Session error → Telegram alert with "View Error" / "Retry" / "Create Bug" buttons

**Gate:** NV reports running sessions in digest. Session completion triggers Telegram alert.

---

### Spec 10: `systemd-deploy`

**Type:** task | **Effort:** S | **Deps:** spec-9

Package and deploy as systemd service.

- `deploy/nv.service`: ExecStart, EnvironmentFile (Doppler), Restart=on-failure
- `deploy/install.sh`: cargo build --release, copy binaries, install service
- Health check endpoint: GET `/health` on localhost port (for nv-cli status)
- Log rotation via tracing-appender (daily, 5 file retention)
- `nv status` reads health endpoint + systemd status

**Gate:** `systemctl start nv` → daemon running → Telegram receives test message.

---

## Future Specs (P1 — Next Week)

### Spec 11: `discord-channel` (P1)
Implement Discord gateway WebSocket + REST channel adapter.

### Spec 12: `teams-channel` (P1)
Implement MS Graph API channel adapter with OAuth2 flow.

### Spec 13: `imessage-channel` (P1)
Implement iMessage channel via BlueBubbles API or Mac relay.

### Spec 14: `jira-webhooks` (P1)
Inbound Jira webhook handler for bidirectional sync.

---

## Spec Dependency Graph

```
spec-1 (scaffold)
  └─→ spec-2 (core types)
        ├─→ spec-3 (telegram)
        │     └─→ spec-4 (agent loop)
        │           ├─→ spec-5 (memory)
        │           │     ├─→ spec-6 (jira)
        │           │     │     ├─→ spec-7 (digest)
        │           │     │     └─→ spec-8 (query)
        │           │     └─→ spec-7 (digest)
        │           └─→ spec-9 (nexus)
        └─→ spec-9 (nexus)

spec-9 + spec-7 + spec-8 ──→ spec-10 (deploy)
```

## Wave Execution Plan

| Wave | Specs | Parallelism | Gate |
|------|-------|-------------|------|
| 1 | 1, 2 | Sequential | `cargo build` |
| 2 | 3, 4 | Sequential (3→4) | Telegram echo test |
| 3 | 5, 6 | Parallel (both need spec-4) | Memory recall + Jira create |
| 4 | 7, 8 | Parallel | Digest arrives + query works |
| 5 | 9, 10 | Sequential (9→10) | systemd running, Nexus connected |
