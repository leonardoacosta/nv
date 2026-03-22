# Implementation Tasks

<!-- beads:epic:TBD -->

## FTS5 Schema

- [ ] [1.1] [P-1] Add FTS5 virtual table creation to MessageStore::init() — CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(content, content=messages, content_rowid=id) [owner:api-engineer]
- [ ] [1.2] [P-1] Add sync triggers in MessageStore::init() — AFTER INSERT, AFTER DELETE, AFTER UPDATE on messages to keep FTS index current [owner:api-engineer]
- [ ] [1.3] [P-2] Add one-time backfill in MessageStore::init() — INSERT INTO messages_fts SELECT id, content FROM messages WHERE id NOT IN (SELECT rowid FROM messages_fts) [owner:api-engineer]

## Search Method

- [ ] [2.1] [P-1] Add search(query, limit) method to MessageStore — FTS5 MATCH query joining messages_fts to messages, ordered by rank, returns Vec<StoredMessage> [owner:api-engineer]
- [ ] [2.2] [P-2] Add error handling for invalid FTS5 query syntax — return user-friendly error string instead of panic [owner:api-engineer]

## Tool Integration

- [ ] [3.1] [P-1] Register search_messages tool definition in tools.rs — input: query (required), limit (optional, default 10, max 50) [owner:api-engineer]
- [ ] [3.2] [P-2] Handle search_messages in worker tool execution loop — call MessageStore.search(), format results with timestamp + sender + content snippet [owner:api-engineer]

## Verify

- [ ] [4.1] cargo build passes [owner:api-engineer]
- [ ] [4.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [ ] [4.3] cargo test — new tests for FTS5 init, search with matches, search with no matches, search with invalid query, backfill idempotency [owner:api-engineer]
