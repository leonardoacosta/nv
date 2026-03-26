# Implementation Tasks

<!-- beads:epic:nv-x0zq -->

## Req-1: send_message_with_effect

- [x] [1.1] [P-1] Add effect ID string constants to `client.rs`: `EFFECT_FIRE`, `EFFECT_HEART`, `EFFECT_CONFETTI`, `EFFECT_THUMBSUP`, `EFFECT_THUMBSDOWN`, `EFFECT_POOP` — each a `&str` holding the Telegram-specified numeric string ID [owner:api-engineer]
- [x] [1.2] [P-1] Add `send_message_with_effect(chat_id: i64, text: &str, effect_id: &str, reply_to: Option<String>, keyboard: Option<&InlineKeyboard>) -> Result<i64>` to `TelegramClient` — identical to `send_message` but with `"message_effect_id": effect_id` in the POST body; chunking and keyboard placement rules identical to `send_message` [owner:api-engineer]
- [x] [1.3] [P-1] Add graceful degradation in `send_message_with_effect`: if the API returns a 400 with description containing `CHAT_SEND_PLAIN_NOT_ALLOWED`, `MEDIA_EMPTY`, or `MESSAGE_EFFECTS_UNAVAILABLE`, log a warn and retry the send without the effect field [owner:api-engineer]

## Req-2: MarkdownV2 Formatter

- [x] [2.1] [P-1] Add `escape_mdv2(text: &str) -> String` private fn to `client.rs` that escapes all MarkdownV2 reserved characters (`_ * [ ] ( ) ~ \` > # + - = | { } . !`) in literal text segments using backslash prefix [owner:api-engineer]
- [x] [2.2] [P-1] Add `markdown_to_mdv2(text: &str) -> String` to `client.rs` implementing the conversions specified in Req-2: bold, italic, inline code, fenced code blocks (language label stripped), strikethrough, spoiler passthrough, headers to bold, horizontal rule, tables to pre block [owner:api-engineer]
- [x] [2.3] [P-2] Add `send_message_mdv2(chat_id: i64, text: &str, reply_to: Option<String>, keyboard: Option<&InlineKeyboard>) -> Result<i64>` to `TelegramClient` — calls `markdown_to_mdv2` and posts with `"parse_mode": "MarkdownV2"`; chunking rules identical to `send_message` [owner:api-engineer]
- [x] [2.4] [P-2] Add unit tests for `markdown_to_mdv2` covering: bold, italic, inline code, fenced code block with language label, strikethrough, spoiler, table, horizontal rule, text containing MarkdownV2 reserved characters in plain prose [owner:api-engineer]

## Req-3: Inline Query Handling

- [x] [3.1] [P-1] Add `InlineQuery` struct to `types.rs`: `id: String`, `from: TgUser`, `query: String`, `offset: String` — derived `Debug, Deserialize` [owner:api-engineer]
- [x] [3.2] [P-1] Add `inline_query: Option<InlineQuery>` field to the `Update` struct in `types.rs` [owner:api-engineer]
- [x] [3.3] [P-1] Add `"inline_query"` to the `allowed_updates` array in `TelegramChannel::get_updates` call in `mod.rs` [owner:api-engineer]
- [x] [3.4] [P-2] Add `InlineQueryResult` enum to `client.rs` with one variant: `Article { id: String, title: String, description: String, message_text: String }` — serializes to Telegram `InlineQueryResultArticle` JSON shape [owner:api-engineer]
- [x] [3.5] [P-2] Add `answer_inline_query(query_id: &str, results: &[InlineQueryResult]) -> Result<()>` to `TelegramClient` — POST `/answerInlineQuery` with `inline_query_id` and `results` array [owner:api-engineer]
- [x] [3.6] [P-2] In `TelegramChannel::poll_messages` in `mod.rs`: when an update carries `inline_query`, check that `update.inline_query.from.id` matches the authorized user ID (stored in `TelegramChannel`); if authorized, convert to `InboundMessage` with `metadata.inline_query = true` and `metadata.inline_query_id = query_id`; if unauthorized, skip silently [owner:api-engineer]
- [x] [3.7] [P-3] Store the authorized user ID on `TelegramChannel` — add `authorized_user_id: Option<i64>` field, populated from the config `telegram.authorized_user_id` key (optional; if absent, skip the user-ID filter for inline queries) [owner:api-engineer]

## Req-4: Worker Effect Integration

- [x] [4.1] [P-2] Identify the `CronEvent::MorningBriefing` outbound send site in `worker.rs`; replace the `TelegramClient::send_message` call with `send_message_with_effect(..., EFFECT_CONFETTI, ...)` [owner:api-engineer]
- [x] [4.2] [P-2] Identify the P0 alert / `Priority::High` outbound send site in `worker.rs`; replace the `TelegramClient::send_message` call with `send_message_with_effect(..., EFFECT_FIRE, ...)` [owner:api-engineer]

## Verify

- [x] [5.1] `cargo build` passes with no new errors [owner:api-engineer]
- [x] [5.2] `cargo clippy -- -D warnings` — no new warnings introduced [owner:api-engineer]
- [x] [5.3] `cargo test` — unit tests for `markdown_to_mdv2` all pass [owner:api-engineer]
- [ ] [5.4] [user] Manual test: send morning briefing trigger — observe confetti animation on the briefing message bubble in Telegram [owner:api-engineer]
- [ ] [5.5] [user] Manual test: type `@nova_bot test` in the authorized chat — confirm the update is received and appears in daemon logs as an inline query trigger [owner:api-engineer]
