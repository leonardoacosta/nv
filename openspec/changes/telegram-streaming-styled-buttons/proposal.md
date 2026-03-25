# Proposal: Telegram Streaming Styled Buttons

## Change ID
`telegram-streaming-styled-buttons`

## Summary

Leverage Telegram Bot API 9.3–9.5 features to improve message quality and emphasis:
`sendMessage`/`copyMessage` `message_effect_id` for animated emphasis on important messages,
MarkdownV2 as a formatting mode alongside the existing HTML path, and `answerInlineQuery` for
inline query support. Add helper methods to `TelegramClient` and update `worker.rs` to use the
new formatting where it improves fidelity.

## Context

- Phase: Wave 1 — Telegram UX (spec 3 of 6, v8 scope-lock)
- Depends on: `streaming-response-delivery` (Wave 1 spec 1) — placeholder send + edit infra must
  be in place before adding effect/format decoration on top
- Extends: `crates/nv-daemon/src/channels/telegram/client.rs`
- Extends: `crates/nv-daemon/src/worker.rs` (outbound message construction)
- Related: `callback-handler-completion` (same wave, inline keyboard infra)
- Source idea: nv-32o

## Motivation

The current `TelegramClient::send_message` uses HTML parse mode exclusively. This works for
bold/italic/code/links but misses three categories of improvements available since Bot API 9.3:

1. **Message effects** — Telegram added `message_effect_id` to `sendMessage` (Bot API 9.3).
   Effects play an animated emoji reaction on top of the message bubble (fire, heart, confetti,
   etc.). Nova sends important messages (morning briefing, P0 alerts, obligation summaries) that
   warrant visual emphasis to distinguish them from mundane replies.

2. **MarkdownV2 fidelity** — Claude outputs standard Markdown. The current `markdown_to_html`
   converter handles the common cases but is a hand-rolled regex chain with known gaps: nested
   formatting, strikethrough (`~~`), spoiler tags, and multi-line code blocks with language
   labels. Telegram's `MarkdownV2` parse mode handles these natively. For messages where
   HTML conversion is lossy, routing through MarkdownV2 produces cleaner output.

3. **Inline query support** — Bot API inline queries (`@nova_bot <query>`) allow Nova to be
   invoked from any chat without forwarding messages. `TelegramChannel` currently only polls
   `message` and `callback_query` in `allowed_updates`; inline queries are silently dropped.
   Adding `answerInlineQuery` support completes the bot's surface area.

## Requirements

### Req-1: `send_message_with_effect` Helper

Add `send_message_with_effect(chat_id, text, effect_id, reply_to, keyboard)` to
`TelegramClient`. This is identical to `send_message` except it includes
`"message_effect_id": effect_id` in the POST body. Effects only work in private chats; the
method must gracefully degrade (log warning, proceed without effect) if the API returns
`CHAT_SEND_PLAIN_NOT_ALLOWED` or any effect-related 400.

Predefined effect IDs to expose as typed constants in `client.rs`:

| Constant | Effect | ID |
|----------|--------|----|
| `EFFECT_FIRE` | Fire | `5104841245755180586` |
| `EFFECT_HEART` | Heart | `5159385139981059251` |
| `EFFECT_CONFETTI` | Confetti | `5046509860389126442` |
| `EFFECT_THUMBSUP` | Thumbs up | `5107584321108051014` |
| `EFFECT_THUMBSDOWN` | Thumbs down | `5104858069142078462` |
| `EFFECT_POOP` | Poop | `5046589136895476101` |

### Req-2: MarkdownV2 Formatter

Add `markdown_to_mdv2(text: &str) -> String` alongside the existing `markdown_to_html`. The
function escapes all MarkdownV2 reserved characters
(`_ * [ ] ( ) ~ \` > # + - = | { } . !`) in literal text, then applies:

- `**bold**` → `*bold*`
- `_italic_` / `*italic*` → `_italic_`
- `` `code` `` → `` `code` ``
- ```` ```lang\nblock\n``` ```` → ```` ```\nblock\n``` ```` (language label stripped — MarkdownV2
  pre blocks do not support language hints)
- `~~strikethrough~~` → `~strikethrough~`
- `||spoiler||` → `||spoiler||` (passthrough — already MarkdownV2 native)
- Headers (`# ##`) → `*bold line*` (same as HTML path)
- Horizontal rule (`---`) → `—————` (same as HTML path)
- Tables → `\`\`\`\npre block\n\`\`\`` (pre-escaped code block, same as HTML path)

Add `send_message_mdv2(chat_id, text, reply_to, keyboard)` to `TelegramClient` that calls
`markdown_to_mdv2` and posts with `"parse_mode": "MarkdownV2"`.

The existing `send_message` (HTML path) is preserved unchanged. No call sites are migrated in
this spec — the MarkdownV2 path is available for opt-in use in follow-on specs.

### Req-3: Inline Query Handling

Add `inline_query` to `allowed_updates` in `TelegramChannel::get_updates`. Add
`InlineQuery` struct to `types.rs` (fields: `id: String`, `from: TgUser`, `query: String`).
Add `inline_query: Option<InlineQuery>` to the `Update` struct.

Add `answer_inline_query(query_id: &str, results: &[InlineQueryResult]) -> Result<()>` to
`TelegramClient`, where `InlineQueryResult` is an enum with one arm for this spec:
`Article { id: String, title: String, description: String, message_text: String }`.

Wire up in `TelegramChannel::poll_messages`: when an `inline_query` update is received,
convert it to an `InboundMessage` with `metadata.inline_query = true` and
`metadata.inline_query_id` set. Worker response routing is out of scope for this spec — the
trigger reaches the agent loop but the reply path (calling `answer_inline_query`) is a
follow-on item.

### Req-4: Worker Effect Integration for High-Priority Messages

In `worker.rs`, identify the two outbound message categories that warrant visual emphasis:

1. **Morning briefing** (`CronEvent::MorningBriefing`) — use `EFFECT_CONFETTI`
2. **P0 obligation alerts** (messages with `[P0]` in content or triggered by `Priority::High`
   tasks) — use `EFFECT_FIRE`

Replace the direct `TelegramClient::send_message` call at these two sites with
`send_message_with_effect`. All other message sends remain on the HTML path unchanged.

## Scope

- **IN**: `send_message_with_effect`, effect ID constants, `markdown_to_mdv2`,
  `send_message_mdv2`, inline query struct + polling + `answer_inline_query`,
  effect application on briefing and P0 paths
- **OUT**: Migrating all send sites to MarkdownV2 (future spec), full inline query reply
  routing (future spec), per-message format preference in `OutboundMessage` type (future spec)

## Impact

| File | Change |
|------|--------|
| `crates/nv-daemon/src/channels/telegram/client.rs` | Add effect constants, `send_message_with_effect`, `markdown_to_mdv2`, `send_message_mdv2`, `answer_inline_query` |
| `crates/nv-daemon/src/channels/telegram/types.rs` | Add `InlineQuery` struct; add `inline_query` field to `Update` |
| `crates/nv-daemon/src/channels/telegram/mod.rs` | Add `inline_query` to `allowed_updates`; convert inline queries to `InboundMessage` in `poll_messages` |
| `crates/nv-daemon/src/worker.rs` | Use `send_message_with_effect` at morning briefing and P0 alert send sites |

## Risks

| Risk | Mitigation |
|------|-----------|
| Effect IDs change across Bot API versions | Embed IDs as string constants with a comment noting the API version; easy to update if Telegram changes them |
| MarkdownV2 escaping is fragile — double-escaping reserved chars in code blocks | Unit tests covering code blocks, links, and special chars; escaping applied before formatting substitution, not after |
| `send_message_with_effect` returns 400 for group/channel chats (effects only work in private chats) | Catch the error, log at warn level, fall back to plain `send_message`; do not propagate the error |
| Inline query flooding (someone types @nova_bot in a public chat) | Only the authorized `chat_id` filter is applied at the `poll_messages` level for regular messages; inline queries come from any user — add a from-user allow-list check in the inline query conversion (only convert from the authorized user's ID) |
