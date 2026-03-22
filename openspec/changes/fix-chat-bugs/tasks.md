# Implementation Tasks

<!-- beads:epic:TBD -->

## API Batch

- [ ] [2.1] [P-1] Add `strip_tool_call_blocks()` function in worker.rs — regex removes fenced code blocks tagged `tool_call` or `tool_use` from text content [owner:api-engineer]
- [ ] [2.2] [P-1] Call `strip_tool_call_blocks()` inside `extract_text()` in worker.rs before returning text [owner:api-engineer]
- [ ] [2.3] [P-1] Apply same `strip_tool_call_blocks()` filter to `extract_text()` in agent.rs [owner:api-engineer]
- [ ] [2.4] [P-1] Add retry logic in claude.rs JSON parsing path — if serde_json fails on empty/truncated stdout, retry once after 1s delay [owner:api-engineer]
- [ ] [2.5] [P-1] Add fallback path in claude.rs — if retry also fails, return structured error (not panic) with message "Thinking failed — retrying..." for Telegram [owner:api-engineer]
- [ ] [2.6] [P-1] Wrap `execute_tool()` in worker.rs with `tokio::time::timeout(Duration::from_secs(30))` — on timeout, return ToolResultBlock with error "Tool timed out after 30s" [owner:api-engineer]
- [ ] [2.7] [P-1] Wrap `execute_tool_send()` in agent.rs with same timeout pattern [owner:api-engineer]
- [ ] [2.8] [P-2] Add `TOOL_TIMEOUT_READ` (30s) and `TOOL_TIMEOUT_WRITE` (60s) constants; apply write timeout to Jira write tools [owner:api-engineer]
- [ ] [2.9] [P-1] Add table detection to `markdown_to_html()` in telegram/client.rs — detect lines with `|` column separators [owner:api-engineer]
- [ ] [2.10] [P-1] Convert detected tables to `<pre>` blocks with aligned columns — strip separator rows (`|------|`), pad columns with spaces [owner:api-engineer]

## Verify

- [ ] [3.1] cargo build passes [owner:api-engineer]
- [ ] [3.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [ ] [3.3] Unit test: `strip_tool_call_blocks()` removes tool_call fenced blocks, preserves normal text [owner:api-engineer]
- [ ] [3.4] Unit test: `strip_tool_call_blocks()` preserves non-tool code blocks (e.g., ```rust) [owner:api-engineer]
- [ ] [3.5] Unit test: JSON parse retry succeeds on second attempt (mock empty then valid response) [owner:api-engineer]
- [ ] [3.6] Unit test: JSON parse retry exhaustion returns structured error, no panic [owner:api-engineer]
- [ ] [3.7] Unit test: tool timeout fires after configured duration, returns error ToolResultBlock [owner:api-engineer]
- [ ] [3.8] Unit test: `markdown_to_html()` converts 3-column table to aligned `<pre>` block [owner:api-engineer]
- [ ] [3.9] Unit test: `markdown_to_html()` strips separator row from table output [owner:api-engineer]
- [ ] [3.10] Existing tests pass [owner:api-engineer]
