# Implementation Tasks

<!-- beads:epic:TBD -->

## Rust Implementation

- [x] [1.1] [P-1] Add rusqlite to workspace deps (Cargo.toml) and nv-daemon deps [owner:api-engineer]
- [x] [1.2] [P-1] Create crates/nv-daemon/src/messages.rs — MessageStore struct with init() that creates ~/.nv/messages.db + schema [owner:api-engineer]
- [x] [1.3] [P-2] Add log_inbound(channel, sender, content, trigger_type) method — inserts inbound message row [owner:api-engineer]
- [x] [1.4] [P-2] Add log_outbound(channel, content, telegram_message_id, response_time_ms, tokens_in, tokens_out) method — inserts outbound message row with timing + token metrics [owner:api-engineer]
- [x] [1.5] [P-2] Add recent(count) method — returns last N messages as Vec<StoredMessage> [owner:api-engineer]
- [x] [1.6] [P-2] Add format_recent_for_context(count) method — returns formatted string for prompt injection [owner:api-engineer]

## Agent Integration

- [x] [2.1] [P-1] Update main.rs — init MessageStore on startup, pass to AgentLoop [owner:api-engineer]
- [x] [2.2] [P-1] Add mod messages declaration in main.rs [owner:api-engineer]
- [x] [2.3] [P-2] Update agent.rs — log inbound triggers (Message type) before Claude call [owner:api-engineer]
- [x] [2.4] [P-2] Update agent.rs — log outbound responses after Telegram send/edit [owner:api-engineer]
- [x] [2.5] [P-2] Update agent.rs — inject recent messages as <recent_messages> context before Claude call [owner:api-engineer]
- [x] [2.6] [P-2] Add get_recent_messages tool to tools.rs — queries MessageStore.recent(), returns formatted text [owner:api-engineer]

## Analytics

- [x] [3.1] [P-1] Add stats() method to MessageStore — returns StatsReport (total messages, messages today, avg response time, total tokens, messages per day for last 7 days) [owner:api-engineer]
- [x] [3.2] [P-2] Add `nv stats` CLI command — queries daemon HTTP endpoint, displays formatted stats (message volume, avg response time, token usage, 7-day chart) [owner:api-engineer]
- [x] [3.3] [P-2] Add GET /stats HTTP endpoint to daemon — returns StatsReport as JSON [owner:api-engineer]
- [x] [3.4] [P-2] Update agent.rs — capture Instant::now() before Claude call, compute response_time_ms, pass to log_outbound [owner:api-engineer]

## Verify

- [x] [4.1] cargo build passes [owner:api-engineer]
- [x] [4.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [x] [4.3] cargo test — new MessageStore tests (init, log, query, format, stats) + existing tests pass [owner:api-engineer]
