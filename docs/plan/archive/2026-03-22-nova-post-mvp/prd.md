# PRD — Nova Post-MVP

> Accumulated from: scope-lock.md + carry-forward from archived MVP plan
> Generated: 2026-03-22

## 1. Vision

Fix the architecture bottleneck (CLI subprocess latency), expand to all communication channels,
then harden everything. Make Nova fast, connected, and reliable — in that order.

## 2. Target Users

Leo. Solo operator. No change from MVP.

## 3. Success Metrics

| Metric | Current (MVP) | Target (Post-MVP) |
|--------|:---:|:---:|
| Response time | 8-14s | <3s |
| Channels | 1 (Telegram) + 2 relays | 6 native (Telegram, Discord, Teams, iMessage, Email, Jira webhooks) |
| Test coverage | 307 tests | 400+ |
| Deferred tasks | 15 | 0 |

## 4. Phase 1: Architecture (Mon-Tue)

### 4.1 Persistent Claude Session

**Problem:** Each agent turn spawns a new `claude -p` subprocess (~8-14s cold start). This is
the single biggest UX pain.

**Approaches (from scope lock):**

| Approach | Latency | Complexity | Notes |
|----------|---------|------------|-------|
| Long-lived CLI (`--input-format stream-json`) | ~2s | Medium | Keep subprocess alive, pipe messages via stdin |
| Claude Agent SDK (`query()`) | ~2s | Medium | TypeScript child process, session management |
| Direct API + API key | ~1s | Low | Needs sk-ant-api03 key (not OAuth compatible) |
| Session resume (`--continue`) | ~4s | Low | Reuses session state, still cold start |

**Recommended:** Long-lived CLI with `stream-json`. Keeps the subprocess alive between turns,
avoids cold start, works with OAuth, no new dependencies. Falls back to current cold-start
mode if the subprocess dies.

**Implementation:**
- Replace `ClaudeClient::send_messages()` — instead of spawn+wait+parse per turn, keep a
  persistent `tokio::process::Child` with stdin/stdout streams
- Parse `stream-json` events incrementally (tool_use, text, result)
- Reconnect on subprocess death (auto-restart with backoff)
- Existing tool_call parsing stays — stream-json emits the same content blocks

### 4.2 Interaction Diary (spec written)

Apply `add-interaction-diary` — 9 tasks. Rust-written daily log at `~/.nv/diary/`.
No token cost. Audit trail for Nova's decisions.

### 4.3 Voice Reply (spec written)

Apply `add-voice-reply` — 14 tasks. ElevenLabs TTS → Telegram voice messages.
Requires: ffmpeg, ELEVENLABS_API_KEY.

## 5. Phase 2: Channels (Wed-Thu)

All channels implement the existing `Channel` trait from nv-core. Each gets:
- Config section in `nv.toml`
- Secrets in env file
- systemd service (if separate process) or tokio task (if in-daemon)
- Auto-logging to SQLite message store
- Injection into agent loop via mpsc trigger channel

### 5.1 Discord Channel

Replace Python relay bot with native Rust adapter.
- Discord gateway WebSocket for receiving events
- REST API for sending messages
- Filter by server/channel via config
- Handle DMs + mentions + watched channels

### 5.2 Teams Channel

Replace Python webhook relay with native MS Graph adapter.
- OAuth2 app registration + token refresh
- Subscription webhooks for channel messages
- REST API for sending replies
- Handle channel messages + DMs

### 5.3 iMessage Channel

New channel via BlueBubbles API.
- HTTP client to BlueBubbles server (runs on Mac)
- Message polling or webhook for new messages
- Send via BlueBubbles API
- Requires BlueBubbles server running on a Mac

### 5.4 Email Channel

Email via MS Graph API (Outlook) or IMAP fallback.
- Poll or webhook for new messages
- Filter by sender, subject, folder
- Reply capability with confirmation
- Parse email body (HTML → text extraction)

### 5.5 Jira Webhooks

Inbound bidirectional sync — others' changes flow into Nova.
- HTTP endpoint receiving Jira webhook payloads
- Parse: issue:updated, issue:created, comment:created
- Update Nova's memory with external changes
- Alert via Telegram when tracked issues change

## 6. Phase 3: Hardening (Fri-Sat)

### 6.1 Jira Integration (12 deferred tasks)

- Retry wrapper with exponential backoff (1s, 2s, 4s) for 429/5xx/network
- Callback handler: `approve:{uuid}` → execute stored JiraClient action
- Callback handler: `edit:{uuid}` → ask what to change, revise draft
- Callback handler: `cancel:{uuid}` → remove from pending, notify
- Expiry sweep: pending actions older than 1 hour → mark expired, edit Telegram message
- Wire callback routing in agent loop (match callback_data prefix)
- HTTP mock tests: 401, 403, 404, 429, 5xx responses
- Case-insensitive transition name matching
- Integration test: real Jira (behind `NV_JIRA_INTEGRATION_TEST=1`)
- Integration test: full create → transition → comment flow
- Manual e2e gate

### 6.2 Telegram Channel (2 deferred tasks)

- Integration test (real Telegram API, behind feature gate)
- Manual e2e: send hello → bot echoes

### 6.3 Nexus Integration (1 deferred task)

- Wire `nexus_err:view:` and `nexus_err:bug:` callback handlers

## 7. Scope & Constraints

### In Scope
Everything in Phases 1-3 above.

### Out of Scope
- Multi-user/multi-tenant
- Web dashboard / TUI (Nexus handles this)
- Embedding-based memory search (grep works)
- Plugin SDK / marketplace
- Slack channel (P3)

### Hard Constraints
- Rust standalone binary
- Secrets via env file (Doppler)
- Linux homelab (systemd)
- Tailscale for inter-machine
- Claude-only AI
- Single user, no auth

## 8. Timeline

| Day | Phase | Deliverables |
|-----|-------|-------------|
| Mon | Architecture | Persistent session spec + exploration |
| Tue | Architecture | Apply diary + voice specs |
| Wed | Channels | Discord + Teams native adapters |
| Thu | Channels | iMessage + Email + Jira webhooks |
| Fri | Hardening | Deferred tasks (retry, callbacks, tests) |
| Sat | Verification | Integration testing + full deploy |

## 9. Ambiguity Notes

| Item | Status |
|------|--------|
| Persistent session approach | 4 options listed — needs exploration/decision |
| BlueBubbles availability | Requires Mac running BlueBubbles server — verify |
| MS Graph OAuth | Teams + Email share OAuth — single registration? |
| ffmpeg availability | Required for voice — verify installed on homelab |
| ElevenLabs API key | Required for voice — provision in Doppler |
