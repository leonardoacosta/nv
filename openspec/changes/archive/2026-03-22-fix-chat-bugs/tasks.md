# Implementation Tasks

<!-- beads:epic:TBD -->

## API Batch

- [x] [2.1] [P-1] Add `strip_tool_call_artifacts()` function in worker.rs — string-based removal of fenced code blocks tagged `tool_call`, `tool_use`, or `json` (tool patterns) from text content [owner:api-engineer]
- [x] [2.2] [P-1] Call `strip_tool_call_artifacts()` inside `extract_text()` in worker.rs before returning text [owner:api-engineer]
- [x] [2.3] [P-1] Apply same `strip_tool_call_artifacts()` filter to `extract_text()` in agent.rs (calls worker's pub fn) [owner:api-engineer]
- [x] [2.4] [P-1] Add retry logic in claude.rs JSON parsing path — if serde_json fails on empty/truncated stdout, retry once after 1s delay [owner:api-engineer]
- [x] [2.5] [P-1] Add fallback path in claude.rs — if retry also fails, return structured error (not panic) with descriptive message for Telegram [owner:api-engineer]
- [x] [2.6] [P-1] Wrap `execute_tool_send()` in worker.rs with `tokio::time::timeout()` — on timeout, return error "Tool timed out after Ns" [owner:api-engineer]
- [x] [2.7] [P-1] Wrap `execute_tool()` in agent.rs with same timeout pattern [owner:api-engineer]
- [x] [2.8] [P-2] Add `TOOL_TIMEOUT_READ` (30s) and `TOOL_TIMEOUT_WRITE` (60s) constants; apply write timeout to Jira write tools [owner:api-engineer]
- [x] [2.9] [P-1] Add table detection to `markdown_to_html()` in telegram/client.rs — detect lines with `|` column separators [owner:api-engineer]
- [x] [2.10] [P-1] Convert detected tables to `<pre>` blocks with aligned columns — strip separator rows (`|------|`), pad columns with spaces [owner:api-engineer]

## Verify

- [x] [3.1] cargo build passes [owner:api-engineer]
- [x] [3.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [x] [3.3] Unit test: `strip_tool_call_artifacts()` removes tool_call fenced blocks, preserves normal text [owner:api-engineer]
- [x] [3.4] Unit test: `strip_tool_call_artifacts()` preserves non-tool code blocks (e.g., ```rust) [owner:api-engineer]
- [ ] [3.5] Unit test: JSON parse retry succeeds on second attempt (mock empty then valid response) [deferred — requires process mocking]
- [ ] [3.6] Unit test: JSON parse retry exhaustion returns structured error, no panic [deferred — requires process mocking]
- [ ] [3.7] Unit test: tool timeout fires after configured duration, returns error ToolResultBlock [deferred — requires async integration harness]
- [x] [3.8] Unit test: `markdown_to_html()` converts 3-column table to aligned `<pre>` block [owner:api-engineer]
- [x] [3.9] Unit test: `markdown_to_html()` strips separator row from table output [owner:api-engineer]
- [x] [3.10] Existing tests pass [owner:api-engineer]
