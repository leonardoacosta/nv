# Scope Lock — Nova Post-MVP

## Vision

Fix the architecture bottleneck (CLI subprocess latency), expand to all communication channels,
then harden everything. Make Nova fast, connected, and reliable — in that order.

## Target Users

Leo. Same as MVP. No change.

## Domain

**In scope:** Persistent Claude sessions (fix latency), native channel adapters (Discord, Teams,
iMessage), Jira bidirectional sync, MVP deferred tasks (retry, callbacks, tests), written specs
(diary, voice).

**Out of scope:** Multi-user support, web dashboard, plugin marketplace, embedding-based search
(deferred to v3).

## Execution Order

**Architecture → Channels → Hardening.** This order is deliberate:

1. Fix the foundation (latency) so new channels benefit from fast response times
2. Expand reach (Discord, Teams, iMessage, Jira webhooks)
3. Harden everything that's now proven (retry, callbacks, tests)

## Phase 1: Architecture (This Week — Priority 1)

### Spec: persistent-claude-session

Replace `claude -p` cold-start per turn with a persistent session. Options:

| Approach | Latency | Complexity | Trade-off |
|----------|---------|------------|-----------|
| **Claude Agent SDK** (`query()`) | ~2s (warm) | Medium | TypeScript child process, session management |
| **Direct API** + API key | ~1s | Low | Needs sk-ant-api03 key (not OAuth) |
| **Long-lived CLI** (`--input-format stream-json`) | ~2s | Medium | Keep subprocess alive between turns |
| **Session resume** (`--continue --session-id`) | ~4s | Low | Reuses session, still cold start per turn |

**Goal:** Response time from 8-14s → under 3s.

### Spec: add-interaction-diary (already written)

Apply existing spec. ~9 tasks. Rust-written daily log at `~/.nv/diary/`.

### Spec: add-voice-reply (already written)

Apply existing spec. ~14 tasks. ElevenLabs TTS → Telegram voice messages.

## Phase 2: Channels (This Week — Priority 2)

### Spec: discord-channel

Native Discord gateway adapter replacing the Python relay bot.
- WebSocket gateway for real-time events
- REST API for sending messages
- Channel filtering via config
- Maps to existing Channel trait

### Spec: teams-channel

Native MS Graph API adapter replacing the webhook relay.
- OAuth2 authentication flow
- Subscription-based message webhooks
- REST API for sending
- Maps to existing Channel trait

### Spec: imessage-channel

iMessage via BlueBubbles API or Mac relay.
- HTTP client to BlueBubbles server
- Message polling or webhook
- Maps to existing Channel trait

### Spec: jira-webhooks

Inbound webhook handler for bidirectional Jira sync.
- HTTP endpoint receiving Jira webhook payloads
- Parse issue:updated, issue:created, comment:created events
- Update Nova's memory with external changes
- Alert via Telegram when others change issues Nova is tracking

## Phase 3: Hardening (This Week — Priority 3)

### Deferred from jira-integration (12 tasks)

- Retry wrapper with exponential backoff (429/5xx/network)
- Callback handlers: edit, cancel, expiry sweep
- Callback routing in agent loop
- HTTP mock tests
- Integration tests (behind env var gate)
- Manual e2e gate

### Deferred from telegram-channel (2 tasks)

- Integration test (real Telegram API)
- Manual e2e gate

### Deferred from nexus-integration (1 task)

- Wire error callbacks in Telegram handler

## v1 Must-Do

Response time under 3 seconds. Everything else is secondary — if Nova is fast, all other
features feel better. If Nova is slow, no number of channels matters.

## v1 Won't-Do

- Multi-user/multi-tenant support
- Web dashboard or TUI (Nexus handles dashboarding)
- Embedding-based semantic memory search (grep works, deferred to v3)
- Plugin SDK / marketplace
- Email/Outlook channel (P2, not this phase)
- Slack channel (P2, not this phase)

## Hard Constraints

Same as MVP:
- Rust standalone binary
- Secrets via Doppler/env file
- Linux homelab (systemd)
- Tailscale for inter-machine
- Claude-only AI
- Single user, no auth

## Timeline

- **Mon-Tue:** Architecture (persistent sessions, diary, voice)
- **Wed-Thu:** Channels (Discord, Teams, iMessage, Jira webhooks)
- **Fri:** Hardening (deferred tasks from MVP)
- **Sat:** Integration testing + deploy

## Assumptions Corrected

- ~~Latency is livable~~ → **Major pain, fix first** (8-14s → target <3s)
- ~~Deferred tasks are optional~~ → **Needed** (retry logic will bite eventually)
- ~~Channels before architecture~~ → **Architecture first** (fast foundation benefits all channels)
