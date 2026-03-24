# Context: Fix Channel Safety Issues

## Source: Audit 2026-03-23 (channels domain 77/C, digest, nexus)

## Problem
Three UTF-8 panics, dead Teams subscription renewal, missing webhook validation, and code duplication across channels.

## Findings

### P1 — UTF-8 panic in Telegram edit_message
- `crates/nv-daemon/src/channels/telegram/client.rs:331`
- `let truncated = &html_text[..html_text.len().min(TELEGRAM_MAX_MESSAGE_LEN)]`
- After markdown_to_html, byte-indexing on UTF-8 string
- Non-ASCII character at 4096-byte boundary causes runtime panic
- Fix: Use `char_indices` or `floor_char_boundary` for safe truncation

### P1 — UTF-8 panic in digest truncate_for_telegram
- `crates/nv-daemon/src/digest/format.rs:26`
- `&text[..budget]` — same byte-indexing bug
- Fix: Same char-boundary-safe approach

### P2 — UTF-8 panic in query format_query_for_telegram
- `crates/nv-daemon/src/query/format.rs` (4066-byte cut point)
- Same pattern, same fix

### P1 — Teams subscription renewal is dead code
- `crates/nv-daemon/src/channels/teams/mod.rs:205`
- `spawn_subscription_renewal` has `#[allow(dead_code)]` and is never called
- MS Graph subscriptions expire after 60 minutes
- Without renewal, Teams channel silently stops receiving after first hour
- Fix: Call spawn_subscription_renewal from connect() after register_subscriptions()

### P2 — Teams webhook has no clientState validation (security)
- `crates/nv-daemon/src/http.rs:146`
- teams_webhook_handler pushes notifications without checking clientState matches stored secret
- Anyone discovering webhook URL can inject arbitrary Teams messages
- Fix: Add state.teams_client_state to HttpState, validate before processing

### P2 — chunk_message duplicated in Discord and Telegram
- `crates/nv-daemon/src/channels/discord/client.rs:101`
- `crates/nv-daemon/src/channels/telegram/client.rs:570`
- Identical implementations — extract to shared `channels::util` module

### P2 — Telegram temp photo files never cleaned up
- `crates/nv-daemon/src/channels/telegram/mod.rs:250`
- `/tmp/nv-photo-{uuid}.jpg` written on every photo message, never deleted

### P3 — Discord gateway doesn't use Resume
- `crates/nv-daemon/src/channels/discord/gateway.rs:317`
- On Reconnect/InvalidSession: full Identify instead of Resume
- Consumes 1000/day identify limit, misses messages during reconnect

### P3 — Teams HTML entity stripping incomplete
- `crates/nv-daemon/src/channels/teams/types.rs:270`
- strip_html_tags strips tags but leaves HTML entities (&amp;, &lt;, &nbsp;)

### P3 — iMessage has no sender allowlist
- `crates/nv-daemon/src/channels/imessage/mod.rs:84`
- All non-self iMessages forwarded — no allowed_chat_guids filter

### P3 — Discord relay creates new aiohttp session per message
- `relays/discord/bot.py:37`
- Each forward_to_telegram creates/closes ClientSession, losing connection pooling

## Files to Modify
- `crates/nv-daemon/src/channels/telegram/client.rs`
- `crates/nv-daemon/src/channels/teams/mod.rs`
- `crates/nv-daemon/src/channels/teams/types.rs`
- `crates/nv-daemon/src/channels/discord/client.rs`
- `crates/nv-daemon/src/channels/discord/gateway.rs`
- `crates/nv-daemon/src/channels/imessage/mod.rs`
- `crates/nv-daemon/src/http.rs`
- `crates/nv-daemon/src/digest/format.rs`
- `crates/nv-daemon/src/query/format.rs`
- `relays/discord/bot.py`
