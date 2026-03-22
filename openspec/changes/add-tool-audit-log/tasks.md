# Implementation Tasks

<!-- beads:epic:TBD -->

## DB Batch

- [x] [1.1] [P-1] Add `tool_usage` table creation to `MessageStore::init()` in messages.rs — schema with id, timestamp, worker_id, tool_name, input_summary, result_summary, success, duration_ms, tokens_in, tokens_out [owner:api-engineer]
- [x] [1.2] [P-1] Add indexes on tool_usage(tool_name) and tool_usage(timestamp) in init() [owner:api-engineer]

## API Batch

- [x] [2.1] [P-1] Add `ToolStatsReport` struct to messages.rs — total_invocations, invocations_today, per_tool (Vec of name/count/success_count/avg_duration_ms), slowest (Vec of name/duration_ms/timestamp) [owner:api-engineer]
- [x] [2.2] [P-1] Add `MessageStore::log_tool_usage()` method — accepts tool_name, input_summary (truncate 500 chars), result_summary (truncate 500 chars), success bool, duration_ms, worker_id (Option<String>), tokens_in/out (Option<i64>) [owner:api-engineer]
- [x] [2.3] [P-1] Add `MessageStore::tool_stats()` method — query total invocations, today count, per-tool breakdown with GROUP BY, top 5 slowest via ORDER BY duration_ms DESC LIMIT 5 [owner:api-engineer]
- [x] [2.4] [P-1] Wrap `execute_tool()` call site in worker.rs — record Instant::now() before, compute duration_ms after, call log_tool_usage() with tool name from ToolUseBlock [owner:api-engineer]
- [x] [2.5] [P-1] Wrap `execute_tool_send()` call site in agent.rs — same timing + logging pattern [owner:api-engineer]
- [x] [2.6] [P-2] Extend `stats_handler` in http.rs — call tool_stats(), add tool_usage section to JSON response [owner:api-engineer]
- [x] [2.7] [P-2] Extend `nv stats` CLI in status.rs — parse tool_usage section from stats JSON, display per-tool table and top 5 slowest [owner:api-engineer]

## Verify

- [x] [3.1] cargo build passes [owner:api-engineer]
- [x] [3.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [x] [3.3] Unit test: tool_usage table created on init with correct schema [owner:api-engineer]
- [x] [3.4] Unit test: log_tool_usage() inserts row, retrievable via tool_stats() [owner:api-engineer]
- [x] [3.5] Unit test: tool_stats() returns correct per-tool breakdown with multiple tools [owner:api-engineer]
- [x] [3.6] Unit test: input_summary and result_summary truncated to 500 chars [owner:api-engineer]
- [x] [3.7] Unit test: tool_stats() top 5 slowest ordered correctly [owner:api-engineer]
- [x] [3.8] Existing tests pass [owner:api-engineer]
