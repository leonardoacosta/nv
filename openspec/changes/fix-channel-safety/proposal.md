# Proposal: Fix Channel Safety

## Change ID
`fix-channel-safety`

## Summary

Address 11 correctness and security defects across the channel layer: three UTF-8 byte-index
panics that cause runtime crashes on non-ASCII content, a Teams subscription renewal function
that is dead code (channel silently stops after 60 minutes), a missing clientState validation
on the Teams webhook endpoint (security), duplicated chunk_message logic in Discord and Telegram,
leaked temporary photo files, Discord gateway always doing a full Identify on reconnect,
incomplete HTML entity stripping in Teams, no sender allowlist in iMessage, and a new
aiohttp session opened per Discord relay message.

## Context
- Extends: `crates/nv-daemon/src/channels/telegram/client.rs` (edit_message, chunk_message)
- Extends: `crates/nv-daemon/src/channels/telegram/mod.rs` (photo temp file lifecycle)
- Extends: `crates/nv-daemon/src/channels/teams/mod.rs` (connect, spawn_subscription_renewal)
- Extends: `crates/nv-daemon/src/channels/teams/types.rs` (strip_html_tags)
- Extends: `crates/nv-daemon/src/channels/discord/client.rs` (chunk_message)
- Extends: `crates/nv-daemon/src/channels/discord/gateway.rs` (reconnect handling)
- Extends: `crates/nv-daemon/src/channels/imessage/mod.rs` (inbound filter)
- Extends: `crates/nv-daemon/src/http.rs` (teams_webhook_handler)
- Extends: `crates/nv-daemon/src/digest/format.rs` (truncate_for_telegram)
- Extends: `crates/nv-daemon/src/query/format.rs` (format_query_for_telegram)
- Extends: `relays/discord/bot.py` (forward_to_telegram)
- Related: Audit 2026-03-23 (channels domain score 77/C)

## Motivation

The audit identified two panic-class bugs and nine lower-severity defects. The panics are the
most urgent: `edit_message` in telegram/client.rs does `&html_text[..max_len]`, which is a
byte-index slice on a UTF-8 `String`. Any message where a multi-byte character (emoji, CJK,
accented letter) falls on the 4096-byte boundary causes an immediate `byte index N is not a
char boundary` panic, crashing the worker thread. The same pattern appears in `digest/format.rs`
and `query/format.rs`. The Teams subscription renewal bug is also P1: subscriptions expire
after 60 minutes and `spawn_subscription_renewal` is never called, so the Teams channel silently
goes deaf after the first hour of uptime.

## Requirements

### Req-1: Fix UTF-8 panic in edit_message (P1)

In `telegram/client.rs:341`, replace `&html_text[..html_text.len().min(TELEGRAM_MAX_MESSAGE_LEN)]`
with a char-boundary-safe truncation. Use `str::floor_char_boundary` (stable since 1.86) or an
equivalent `char_indices`-based scan. The result must not exceed `TELEGRAM_MAX_MESSAGE_LEN`
bytes and must never split a multi-byte character.

### Req-2: Fix UTF-8 panic in truncate_for_telegram (P1)

In `digest/format.rs:32`, replace `&text[..budget]` with the same char-boundary-safe helper.
The fix must also preserve the existing `rfind('\n')` line-break preference and the
`[... truncated]` suffix.

### Req-3: Fix UTF-8 panic in format_query_for_telegram (P1)

In `query/format.rs`, apply the same char-boundary-safe truncation wherever the function
slices the answer text to `TELEGRAM_MAX_CHARS`. If a shared `safe_truncate(s, max_bytes)`
helper is introduced for Req-1 and Req-2, call it here too — do not duplicate the logic a
third time.

### Req-4: Wire spawn_subscription_renewal into connect() (P1)

In `teams/mod.rs`, remove the `#[allow(dead_code)]` attribute from `spawn_subscription_renewal`
and call it at the end of `connect()`, after `register_subscriptions()` succeeds. Pass the
`Arc<MsGraphAuth>` and the subscription IDs collected during registration. Teams subscriptions
expire after 60 minutes; without this call the channel stops receiving messages silently.

### Req-5: Add clientState validation to teams_webhook_handler (P2)

In `http.rs`, add `teams_client_state: String` to `HttpState` (or equivalent state struct
passed to the handler). Before processing any notification in `teams_webhook_handler`, compare
each notification's `clientState` field to the stored secret. Reject the entire request with
`StatusCode::UNAUTHORIZED` if any notification fails validation. The client state secret must
come from the same source as the one used when registering subscriptions (already stored in
`TeamsChannel.client_state`).

### Req-6: Extract chunk_message into channels::util (P2)

Create `crates/nv-daemon/src/channels/util.rs` (or equivalent shared module) and move
`chunk_message` there. The implementations in `discord/client.rs:101` and
`telegram/client.rs:580` are byte-for-byte identical; keep one canonical copy. Update both
call sites to use the shared version. The function must remain `pub`.

### Req-7: Delete Telegram temp photo files after use (P2)

In `telegram/mod.rs`, after the photo path is consumed (passed into `InboundMessage` metadata
and the message is forwarded to the worker), schedule deletion of the temp file. Use a
`tokio::spawn` or a `defer`-style drop guard so the file is removed regardless of whether
processing succeeds. The path pattern is `/tmp/nv-photo-{uuid}.jpg`.

### Req-8: Implement Discord gateway Resume (P3)

In `discord/gateway.rs`, store the `session_id` and last `sequence` number received from the
gateway. On `GatewayOpcode::Reconnect` or a resumable `InvalidSession(true)`, send a Resume
payload instead of breaking out of the loop and triggering a fresh Identify. On non-resumable
`InvalidSession(false)`, clear the stored session and fall back to Identify. This prevents
burning the 1000/day Identify quota and avoids the message gap during reconnect.

### Req-9: Decode HTML entities in strip_html_tags (P3)

In `teams/types.rs`, after the existing tag-stripping loop, decode the five standard HTML
entities: `&amp;` → `&`, `&lt;` → `<`, `&gt;` → `>`, `&quot;` → `"`, `&nbsp;` → ` `.
Do not pull in an external crate — a simple `str::replace` chain is sufficient.

### Req-10: Add sender allowlist to iMessage channel (P3)

In `imessage/mod.rs`, read an optional `allowed_chat_guids: Vec<String>` from channel config.
When the list is non-empty, filter out any inbound message whose `chat_guid` is not in the
allowlist before pushing to the message buffer. When the list is empty (default), preserve
current behaviour (all non-self messages pass through).

### Req-11: Reuse aiohttp session in Discord relay (P3)

In `relays/discord/bot.py`, create a single `aiohttp.ClientSession` at module level (or as a
bot attribute in `on_ready`) and reuse it across all `forward_to_telegram` calls. Close the
session in an `on_close` / cleanup hook. This restores HTTP connection pooling and eliminates
the per-message handshake overhead.

## Scope
- **IN**: UTF-8 safe truncation helpers, Teams subscription wiring, webhook clientState check,
  chunk_message deduplication, temp file cleanup, Discord Resume, HTML entity decoding,
  iMessage allowlist, aiohttp session reuse
- **OUT**: New channel integrations, message retry queues, Teams subscription monitoring
  dashboard, persistent photo storage

## Impact

| Area | Change |
|------|--------|
| `crates/nv-daemon/src/channels/telegram/client.rs` | Safe truncation in `edit_message`; remove duplicate `chunk_message` |
| `crates/nv-daemon/src/channels/telegram/mod.rs` | Delete temp photo file after forwarding |
| `crates/nv-daemon/src/channels/teams/mod.rs` | Call `spawn_subscription_renewal` from `connect()`; remove `#[allow(dead_code)]` |
| `crates/nv-daemon/src/channels/teams/types.rs` | Decode HTML entities in `strip_html_tags` |
| `crates/nv-daemon/src/channels/discord/client.rs` | Remove duplicate `chunk_message`; use shared util |
| `crates/nv-daemon/src/channels/discord/gateway.rs` | Store session_id + seq; send Resume on reconnect |
| `crates/nv-daemon/src/channels/imessage/mod.rs` | Filter by `allowed_chat_guids` when configured |
| `crates/nv-daemon/src/channels/util.rs` | New file: canonical `chunk_message` implementation |
| `crates/nv-daemon/src/http.rs` | Validate `clientState` in `teams_webhook_handler` |
| `crates/nv-daemon/src/digest/format.rs` | Safe truncation in `truncate_for_telegram` |
| `crates/nv-daemon/src/query/format.rs` | Safe truncation in `format_query_for_telegram` |
| `relays/discord/bot.py` | Shared `aiohttp.ClientSession`; close on shutdown |

## Risks
| Risk | Mitigation |
|------|-----------|
| `floor_char_boundary` requires Rust 1.86 | Check MSRV in workspace Cargo.toml; if below 1.86 use `char_indices` scan instead |
| clientState field absent in older MS Graph notification schemas | Log a warning and reject (fail-closed); do not silently accept missing state |
| Resume requires storing mutable gateway state across reconnects | Session state is local to the `run_gateway` function; pass by `&mut` or wrap in a small struct |
| iMessage allowlist breaks existing single-user deployments | Default is empty list (allow-all) — no behaviour change unless explicitly configured |
