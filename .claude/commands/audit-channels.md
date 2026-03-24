---
name: audit:channels
description: Audit messaging channels — Telegram, Discord, Teams, Email, iMessage, message store
type: command
execution: foreground
---

# Audit: Channels

Audit inbound/outbound message routing across 5 messaging platforms and the unified message store.

## Scope

| Channel | Directory | Client | Auth |
|---------|-----------|--------|------|
| Discord | `crates/nv-daemon/src/channels/discord/` | `DiscordRestClient` | Bot token |
| Telegram | `crates/nv-daemon/src/channels/telegram/` | `TelegramClient` | Bot token |
| Teams | `crates/nv-daemon/src/channels/teams/` | `TeamsClient` | MS Graph OAuth |
| Email | `crates/nv-daemon/src/channels/email/` | `EmailClient` | MS Graph OAuth |
| iMessage | `crates/nv-daemon/src/channels/imessage/` | `BlueBubblesClient` | Password |
| Message Store | `crates/nv-daemon/src/messages.rs` | `MessageStore` | SQLite |

## Routes

| # | Method | Path | What to check |
|---|--------|------|---------------|
| 1 | POST | `/webhooks/teams` | MS Graph subscription validation, message ingestion |
| 2 | POST | `/webhooks/jira` | Jira event processing (conditional route) |

## Audit Checklist

### Per-Channel Checks
- [ ] **Discord**: Auto-chunking at 2000 chars, embed support, error recovery
- [ ] **Telegram**: Long polling with update_id cursor, inline keyboard support, HTML formatting
- [ ] **Teams**: Subscription lifecycle (max 60min), renewal logic, message buffer
- [ ] **Email**: OAuth token refresh, MIME parsing, folder filtering
- [ ] **iMessage**: BlueBubbles timestamp-based polling, attachment handling

### Cross-Channel
- [ ] `InboundMessage` metadata correctly populated per channel
- [ ] `OutboundMessage` routing (channel selection logic)
- [ ] Message deduplication across channels
- [ ] Error isolation (one channel failure doesn't crash others)
- [ ] Graceful degradation when channel is unconfigured

### Message Store
- [ ] SQLite migrations run correctly
- [ ] `StoredMessage` fields populated (direction, channel, sender, tokens)
- [ ] Stats aggregation (daily counts, tool usage, token tracking)
- [ ] Concurrent access safety (Connection vs Arc<Mutex>)

### Relays
- [ ] Discord relay (`relays/discord/bot.py`) — DM + channel forwarding to Telegram
- [ ] Teams relay (`relays/teams/server.py`) — Power Automate webhook → Telegram
- [ ] Webhook secret validation in Teams relay

## Memory

Persist findings to: `.claude/audit/memory/channels-memory.md`

## Findings

Log to: `~/.claude/scripts/state/nv-audit-findings.jsonl`
