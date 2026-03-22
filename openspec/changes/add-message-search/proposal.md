# Proposal: Add Message Search

## Change ID
`add-message-search`

## Summary

Add FTS5 full-text search to the existing SQLite messages table. New tool `search_messages(query,
limit)` lets Claude search past conversations — "search my conversations for Stripe fee discussion."

## Context
- Extends: `crates/nv-daemon/src/messages.rs` (FTS5 virtual table, search method), `crates/nv-daemon/src/tools.rs` (new tool definition)
- Related: Existing `MessageStore` with `messages` table, `get_recent_messages` tool (returns last N by time, not by relevance)
- Depends on: none (messages table already exists)

## Motivation

The existing `get_recent_messages` tool returns messages by recency. When Leo asks "what did we
discuss about Stripe fees last week?" the tool returns the last 20 messages regardless of content.
FTS5 enables relevance-based search across all stored messages, unlocking the full conversation
archive as a queryable knowledge base.

## Requirements

### Req-1: FTS5 Virtual Table

Create an FTS5 virtual table synchronized with the existing messages table:

```sql
CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
    content,
    content=messages,
    content_rowid=id
);
```

### Req-2: FTS5 Triggers

Keep the FTS index in sync via SQLite triggers on the messages table:

```sql
CREATE TRIGGER IF NOT EXISTS messages_ai AFTER INSERT ON messages BEGIN
    INSERT INTO messages_fts(rowid, content) VALUES (new.id, new.content);
END;

CREATE TRIGGER IF NOT EXISTS messages_ad AFTER DELETE ON messages BEGIN
    INSERT INTO messages_fts(messages_fts, rowid, content) VALUES('delete', old.id, old.content);
END;

CREATE TRIGGER IF NOT EXISTS messages_au AFTER UPDATE ON messages BEGIN
    INSERT INTO messages_fts(messages_fts, rowid, content) VALUES('delete', old.id, old.content);
    INSERT INTO messages_fts(rowid, content) VALUES (new.id, new.content);
END;
```

### Req-3: Backfill Existing Messages

On first init (FTS table creation), run a one-time backfill:

```sql
INSERT INTO messages_fts(rowid, content) SELECT id, content FROM messages;
```

This populates the index with any messages logged before the FTS table existed.

### Req-4: Search Method

Add `search(query, limit)` method to `MessageStore`:

```rust
pub fn search(&self, query: &str, limit: usize) -> Result<Vec<StoredMessage>>
```

- Uses FTS5 `MATCH` query: `SELECT m.* FROM messages m JOIN messages_fts f ON m.id = f.rowid WHERE messages_fts MATCH ?1 ORDER BY rank LIMIT ?2`
- Default limit: 10, max: 50
- Returns full `StoredMessage` rows ranked by relevance

### Req-5: Tool Definition

Register `search_messages` tool:

```json
{
    "name": "search_messages",
    "description": "Search past conversations using full-text search. Returns messages matching the query ranked by relevance.",
    "input_schema": {
        "type": "object",
        "properties": {
            "query": { "type": "string", "description": "Search query (supports FTS5 syntax: AND, OR, NOT, phrases)" },
            "limit": { "type": "integer", "description": "Max results to return (default 10, max 50)" }
        },
        "required": ["query"]
    }
}
```

## Scope
- **IN**: FTS5 virtual table, sync triggers, backfill on init, search method, search_messages tool
- **OUT**: Embedding-based semantic search, message deletion, search result highlighting, search history

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/messages.rs` | Add FTS5 table creation, triggers, backfill in init(); add search() method |
| `crates/nv-daemon/src/tools.rs` | Register search_messages tool definition |
| `crates/nv-daemon/src/worker.rs` | Handle search_messages tool call in execution loop |

## Risks
| Risk | Mitigation |
|------|-----------|
| FTS5 not compiled into rusqlite | rusqlite bundles SQLite with FTS5 enabled by default via `bundled` feature |
| Backfill slow on large message store | One-time operation, ~1ms per 1000 messages. At 10K messages, <10ms. |
| FTS5 query syntax errors from Claude | Wrap query in try-catch, return "Invalid search query" on parse error |
