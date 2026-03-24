# Channels Domain Audit Memory

**Audited:** 2026-03-23
**Auditor:** codebase-health-analyst
**Scope:** All 5 channel adapters, message store, and relay scripts

---

## Files Reviewed

- `crates/nv-daemon/src/channels/discord/` — client.rs, gateway.rs, mod.rs, types.rs
- `crates/nv-daemon/src/channels/telegram/` — client.rs, mod.rs, types.rs
- `crates/nv-daemon/src/channels/teams/` — client.rs, mod.rs, oauth.rs, types.rs
- `crates/nv-daemon/src/channels/email/` — client.rs, mod.rs, html.rs, types.rs
- `crates/nv-daemon/src/channels/imessage/` — client.rs, mod.rs, types.rs
- `crates/nv-daemon/src/messages.rs`
- `crates/nv-daemon/src/http.rs` (webhook handler section)
- `relays/discord/bot.py`
- `relays/teams/server.py`

---

## Scores

| Axis         | Score | Grade |
|--------------|-------|-------|
| Structure    | 82    | B     |
| Quality      | 76    | C     |
| Architecture | 74    | C     |
| **Health**   | **77**| **C** |

Composite: (82 × 0.30) + (76 × 0.35) + (74 × 0.35) = 24.6 + 26.6 + 25.9 = **77.1**

---

## Per-Channel Checklist

### Discord

| Check | Status | Notes |
|-------|--------|-------|
| Auto-chunking at 2000 chars | PASS | chunk_message correctly splits at paragraph/line/hard-cut |
| Embed support | CONCERN | No embed support — plain text only |
| Error recovery | PASS | Exponential backoff 1s→60s in run_poll_loop |
| Rate-limit retry | PASS | HTTP 429 + Retry-After handled in post_message |
| Self-message filter | PASS | bot_user_id set after READY, messages filtered |
| Gateway reconnect | CONCERN | Full Identify on reconnect (no Resume) wastes identify budget |

### Telegram

| Check | Status | Notes |
|-------|--------|-------|
| Long polling with update_id cursor | PASS | AtomicI64 offset advanced correctly |
| Inline keyboard support | PASS | keyboard attached to last chunk on send |
| HTML formatting | PASS | markdown_to_html converts tables, bold, code |
| Callback query ACK | PASS | answer_callback_query called for authorized updates |
| Voice transcription | PASS | ElevenLabs STT integrated with graceful degradation |
| Photo handling | PASS | Largest PhotoSize used; temp file written |
| Temp file cleanup | FAIL | /tmp/nv-photo-*.jpg files never deleted |

### Teams

| Check | Status | Notes |
|-------|--------|-------|
| Subscription lifecycle | PASS | Created on connect(), deleted on disconnect() |
| Subscription max 60min | PASS | Expiration set to 55min |
| Renewal logic | FAIL | spawn_subscription_renewal is #[allow(dead_code)] and never called |
| Message buffer | PASS | VecDeque, mutex-protected, drained by poll_messages |
| Webhook clientState validation | FAIL | Validation not performed in teams_webhook_handler |
| HTML body stripping | CONCERN | strip_html_tags doesn't decode HTML entities |

### Email

| Check | Status | Notes |
|-------|--------|-------|
| OAuth token refresh | PASS | MsGraphAuth caches token, refreshes 5min before expiry |
| MIME/HTML parsing | PASS | html_to_text decodes entities, preserves paragraph structure |
| Folder filtering | PASS | Per-folder last_seen cursor |
| Sender filter | PASS | Exact, domain, and @domain patterns all work |
| Subject filter | PASS | Case-insensitive substring match |
| Mark as read | PASS | Prevents re-processing |
| Timestamp cursor ordering | CONCERN | Lexicographic max() over ISO strings — mixed tz formats could cause duplicates |

### iMessage

| Check | Status | Notes |
|-------|--------|-------|
| Timestamp-based polling | PASS | AtomicI64 last_seen_ts in milliseconds |
| Attachment handling | PASS | attachment-only messages (null text) filtered out |
| Self-message filter | PASS | is_from_me checked |
| Allowlist / auth filter | FAIL | No sender or chat GUID allowlist |
| Connect validation cost | CONCERN | get_messages(0, 1) fetches from epoch 0 |

### Message Store

| Check | Status | Notes |
|-------|--------|-------|
| Migrations run correctly | PASS | rusqlite_migration with user_version |
| WAL mode enabled | PASS | PRAGMA journal_mode=WAL |
| StoredMessage fields populated | PASS | direction, channel, sender, tokens_in/out all present |
| Stats aggregation | PASS | daily_counts, tool_stats, usage_stats, budget_status |
| FTS5 search | PASS | messages_fts virtual table with triggers |
| Concurrent access safety | CONCERN | Connection is not thread-safe; works only because single-threaded use |

### Relays

| Check | Status | Notes |
|-------|--------|-------|
| Discord relay — DM + channel forwarding | PASS | Both DMs and watched channels forwarded |
| Teams relay — Power Automate webhook | PASS | JSON parsing and HTML stripping works |
| Webhook secret validation | CONCERN | Teams relay uses plain string equality (not constant-time) |
| Discord relay HTTP session | FAIL | New aiohttp.ClientSession per message (no pooling) |
| Teams relay sync blocking | CONCERN | Synchronous server blocks on Telegram API calls |

---

## Top Findings (Priority Order)

### P1 — Must Fix

**1. UTF-8 panic in edit_message (Telegram)**
`html_text[..html_text.len().min(TELEGRAM_MAX_MESSAGE_LEN)]` truncates by byte count after HTML conversion. Multi-byte UTF-8 characters (e.g., emoji, CJK) can fall on a byte boundary that is not a char boundary, causing a Rust panic.
- File: `crates/nv-daemon/src/channels/telegram/client.rs` line 331
- Fix: Use `html_text.char_indices().take_while(|(i, _)| *i < TELEGRAM_MAX_MESSAGE_LEN).last().map(|(i, c)| i + c.len_utf8()).unwrap_or(0)` or `html_text[..].chars().take(N).collect()`

**2. Teams webhook has no clientState validation**
Any POST to `/webhooks/teams` is accepted. The `client_state` secret generated in `TeamsChannel::new` is registered with MS Graph but never checked when notifications arrive.
- File: `crates/nv-daemon/src/http.rs` line 146
- Fix: Compare `notification.client_state` against `state.teams_client_state` before pushing to buffer

**3. Teams subscription renewal never runs**
`spawn_subscription_renewal` is dead code. Subscriptions expire after 60 minutes and the Teams channel silently stops receiving.
- File: `crates/nv-daemon/src/channels/teams/mod.rs` line 205
- Fix: Remove `#[allow(dead_code)]` and call `spawn_subscription_renewal` from `connect()` after registering subscriptions

### P2 — Should Fix

**4. chunk_message duplicated across Discord and Telegram**
The exact same function body exists in both `discord/client.rs` and `telegram/client.rs`. Extract to `crates/nv-daemon/src/channels/util.rs` or `nv-core`.

**5. Telegram temp photo files never cleaned up**
`/tmp/nv-photo-{uuid}.jpg` files are written but never deleted. Over time these accumulate.
- Fix: Return the path in InboundMessage metadata and clean up after the Claude turn completes in run_poll_loop.

**6. Discord gateway does not Resume sessions**
On Reconnect (op 7) or InvalidSession (op 9), the code breaks out of the event loop, triggering a full reconnect with Identify. Discord's session Resume should be used when the session is still resumable (stores `resume_gateway_url` and `session_id` in `ReadyData` but never uses them).
- File: `crates/nv-daemon/src/channels/discord/gateway.rs` line 317

**7. migration user_version test asserts wrong value**
Test at `messages.rs:768` asserts `user_version == 1` but there are 4 migrations, so the correct assertion is `== 4`.
- Fix: Change `assert_eq!(version, 1, ...)` to `assert_eq!(version, 4, ...)`

### P3 — Track

**8. Email timestamp cursor uses lexicographic max**
May cause duplicate processing on ISO timestamps with mixed timezone formats.

**9. iMessage has no allowlist**
No chat_guid or phone number filter — all iMessages on the BlueBubbles host are forwarded.

**10. Teams HTML entity stripping incomplete**
`strip_html_tags` in teams/types.rs does not decode `&amp;`, `&lt;`, etc. The email html_to_text should be shared.

**11. Discord relay creates new HTTP session per message**
Performance issue under load. Create session once at startup.

---

## Structural Completeness

- [x] All channel adapters implement the Channel trait from nv-core
- [x] All poll loops have exponential backoff
- [x] All send paths require reply_to for routing
- [x] Error isolation: each channel has its own poll task; failure in one does not crash others
- [x] Graceful degradation: TeamsChannel, EmailChannel, IMessageChannel are optional (guarded by config.enabled)
- [ ] Teams subscription renewal: implementation exists but is never invoked
- [ ] Telegram temp file cleanup: not implemented

### Blocking Issues
- UTF-8 panic in Telegram edit_message (production crash risk on non-ASCII messages over 4096 chars)
- Teams subscription renewal dead code (channel silently fails after 60 min)

### Debt-Inducing Issues
- Duplicated chunk_message function
- Missing clientState validation in Teams webhook
- Missing session Resume in Discord gateway
- No iMessage allowlist
- Wrong assertion in migration test

---

## Architecture Notes

The channel design is clean: a common `Channel` trait (connect/poll_messages/send_message/disconnect) backed by per-channel poll loops with mpsc trigger channels. Error isolation is solid — each channel is an independent tokio task. The reply_to routing convention (different meaning per channel: Discord uses it as channel_id, Telegram as message_id, Teams as "team_id:channel_id", Email as "message_id:address") is a leaky abstraction that should be documented or replaced with typed routing.

The MS Graph OAuth implementation (`MsGraphAuth`) is well-factored and correctly shared between Teams and Email channels. Token caching with a 5-minute refresh buffer is appropriate.

The `MessageStore` using rusqlite directly (not an async wrapper) is a pragmatic choice for a single-threaded daemon, but the `Connection` not being `Send` is a latent risk if threading changes.
