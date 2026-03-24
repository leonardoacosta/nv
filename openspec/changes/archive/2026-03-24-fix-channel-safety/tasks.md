# Implementation Tasks

<!-- beads:epic:TBD -->

## API Batch

- [x] [1.1] [P-1] Add `safe_truncate(s: &str, max_bytes: usize) -> &str` helper in `channels/util.rs` — use `str::floor_char_boundary` (Rust ≥1.86) or `char_indices` scan fallback — `crates/nv-daemon/src/channels/util.rs` [owner:api-engineer]
- [x] [1.2] [P-1] Fix UTF-8 panic in `edit_message`: replace byte-index slice with `safe_truncate` call — `crates/nv-daemon/src/channels/telegram/client.rs:341` [owner:api-engineer]
- [x] [1.3] [P-1] Fix UTF-8 panic in `truncate_for_telegram`: replace `&text[..budget]` with `safe_truncate`; preserve `rfind('\n')` line-break preference and `[... truncated]` suffix — `crates/nv-daemon/src/digest/format.rs:32` [owner:api-engineer]
- [x] [1.4] [P-1] Fix UTF-8 panic in `format_query_for_telegram`: replace byte-index slice with `safe_truncate` — `crates/nv-daemon/src/query/format.rs` [owner:api-engineer]
- [x] [1.5] [P-1] Remove `#[allow(dead_code)]` from `spawn_subscription_renewal` and call it at end of `connect()` after `register_subscriptions()` succeeds — `crates/nv-daemon/src/channels/teams/mod.rs:113` [owner:api-engineer]
- [x] [1.6] [P-2] Add `teams_client_state: String` to `HttpState`; validate each notification's `clientState` field in `teams_webhook_handler` before processing — return `StatusCode::UNAUTHORIZED` on mismatch — `crates/nv-daemon/src/http.rs:146` [owner:api-engineer]
- [x] [1.7] [P-2] Move `chunk_message` to `channels/util.rs` and re-export from there; remove duplicate in `discord/client.rs:101` and `telegram/client.rs:580`, update both call sites — `crates/nv-daemon/src/channels/util.rs`, `crates/nv-daemon/src/channels/discord/client.rs`, `crates/nv-daemon/src/channels/telegram/client.rs` [owner:api-engineer]
- [x] [1.8] [P-2] Delete temp photo file after forwarding to worker: spawn a cleanup task or use a drop guard on the path returned at `telegram/mod.rs:250` — `crates/nv-daemon/src/channels/telegram/mod.rs:250` [owner:api-engineer]
- [x] [1.9] [P-3] Store `session_id` and last `sequence` in gateway state; send Resume payload on `GatewayOpcode::Reconnect` and resumable `InvalidSession`; fall back to full Identify only on non-resumable `InvalidSession` — `crates/nv-daemon/src/channels/discord/gateway.rs:317` [owner:api-engineer]
- [x] [1.10] [P-3] Decode HTML entities (`&amp;`, `&lt;`, `&gt;`, `&quot;`, `&nbsp;`) in `strip_html_tags` after tag removal — `crates/nv-daemon/src/channels/teams/types.rs:270` [owner:api-engineer]
- [x] [1.11] [P-3] Add optional `allowed_chat_guids: Vec<String>` to iMessage channel config; filter inbound messages to allowlist when non-empty — `crates/nv-daemon/src/channels/imessage/mod.rs:84` [owner:api-engineer]
- [x] [1.12] [P-3] Create a single `aiohttp.ClientSession` as a bot attribute (initialised in `on_ready`); reuse across all `forward_to_telegram` calls; close in shutdown handler — `relays/discord/bot.py:37` [owner:api-engineer]

## Verify

- [x] [2.1] `cargo build` passes [owner:api-engineer]
- [x] [2.2] `cargo clippy -- -D warnings` passes (pre-existing warnings only) [owner:api-engineer]
- [x] [2.3] Unit test: `safe_truncate` returns input unchanged when `len() <= max_bytes` [owner:api-engineer]
- [x] [2.4] Unit test: `safe_truncate` on a string where a 4-byte emoji straddles the boundary returns a slice ending before the emoji [owner:api-engineer]
- [x] [2.5] Unit test: `edit_message` truncation path does not panic with a 5000-char string containing non-ASCII characters [owner:api-engineer]
- [x] [2.6] Unit test: `truncate_for_telegram` with multi-byte chars at boundary — no panic, output ≤ `TELEGRAM_CHAR_LIMIT` bytes [owner:api-engineer]
- [x] [2.7] Unit test: `format_query_for_telegram` with multi-byte chars — no panic, output ≤ `TELEGRAM_MAX_CHARS` bytes [owner:api-engineer]
- [x] [2.8] Unit test: `strip_html_tags` decodes `&amp;lt;b&amp;gt;hello&amp;lt;/b&amp;gt;` correctly — entities resolved after tag removal [owner:api-engineer]
- [x] [2.9] Unit test: `chunk_message` in util produces same output as the removed discord/telegram copies (golden-value test) [owner:api-engineer]
- [x] [2.10] Existing tests pass [owner:api-engineer]
