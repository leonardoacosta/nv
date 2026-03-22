# Proposal: Tool Audit Log

## Change ID
`add-tool-audit-log`

## Summary

Add a `tool_usage` SQLite table that logs every tool invocation — name, input summary, result
summary, success/failure, duration, worker ID, and token counts. Extend `nv stats` with a tool
usage section showing invocation counts, success rates, and average durations.

## Context
- Extends: `crates/nv-daemon/src/messages.rs` (MessageStore, SQLite connection)
- Extends: `crates/nv-daemon/src/tools.rs` (execute_tool, execute_tool_send)
- Extends: `crates/nv-daemon/src/http.rs` (stats_handler for nv stats endpoint)
- Related: PRD §5.1 (Tool Usage Audit Log)

## Motivation

Nova currently has zero visibility into tool usage. When a tool fails, stalls, or gets called
unexpectedly, there is no record. The audit log enables:

1. **Debugging** — trace any tool invocation by timestamp/worker
2. **`nv stats`** — show which tools are used most, failure rates, latency percentiles
3. **Rate limit awareness** — track API call volume against provider limits
4. **Foundation** — all future tools (Docker, Tailscale, GitHub, etc.) log to this table

## Requirements

### Req-1: tool_usage SQLite Table

Add to the existing SQLite database (`~/.nv/messages.db`) via `MessageStore::init()`:

```sql
CREATE TABLE IF NOT EXISTS tool_usage (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL DEFAULT (datetime('now')),
    worker_id TEXT,
    tool_name TEXT NOT NULL,
    input_summary TEXT,
    result_summary TEXT,
    success INTEGER NOT NULL DEFAULT 1,
    duration_ms INTEGER,
    tokens_in INTEGER,
    tokens_out INTEGER
);
CREATE INDEX IF NOT EXISTS idx_tool_usage_name ON tool_usage(tool_name);
CREATE INDEX IF NOT EXISTS idx_tool_usage_timestamp ON tool_usage(timestamp);
```

### Req-2: Logging Wrapper

Add `MessageStore::log_tool_usage()` method that accepts tool name, input summary (truncated
to 500 chars), result summary (truncated to 500 chars), success bool, duration_ms, worker_id,
and optional token counts. Call this after every `execute_tool()` and `execute_tool_send()`
return in tools.rs — wrap the call site, not each tool individually.

### Req-3: Tool Stats Query

Add `MessageStore::tool_stats()` method returning a `ToolStatsReport` struct:
- Total invocations
- Invocations today
- Per-tool breakdown: name, count, success_count, avg_duration_ms
- Top 5 slowest invocations (tool_name, duration_ms, timestamp)

### Req-4: Extend nv stats

Add a "Tool Usage" section to the `stats_handler` HTTP endpoint and the `nv stats` CLI output.
Display: total calls, calls today, per-tool table (name | calls | success% | avg ms), top 5
slowest.

## Scope
- **IN**: SQLite schema, logging wrapper, stats query, nv stats extension
- **OUT**: Real-time monitoring, alerting on failure rates, dashboard UI

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/messages.rs` | Add tool_usage table to init(), add `log_tool_usage()` and `tool_stats()` methods, add `ToolStatsReport` struct |
| `crates/nv-daemon/src/tools.rs` | Wrap execute_tool/execute_tool_send call sites with timing + logging |
| `crates/nv-daemon/src/http.rs` | Extend stats_handler to include tool stats in JSON response |
| `crates/nv-cli/src/commands/status.rs` | Display tool usage section in `nv stats` CLI output |

## Risks
| Risk | Mitigation |
|------|-----------|
| Logging overhead on hot path | SQLite insert is <1ms; truncate summaries to 500 chars to bound size |
| Database growth over time | Add timestamp index for efficient pruning; future: add retention policy (not this spec) |
| Worker ID not available in all call paths | Pass Option<String> for worker_id; None for agent-loop tools |
