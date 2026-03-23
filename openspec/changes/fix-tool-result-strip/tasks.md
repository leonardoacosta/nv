# Implementation Tasks

<!-- beads:epic:TBD -->

## Strip New Artifact Patterns

- [x] [1.1] [P-1] Add `strip_tool_response_xml()` — remove `<tool_response>...</tool_response>` blocks (multi-line, non-greedy) from response text; skip content inside fenced code blocks (triple-backtick regions) [owner:api-engineer]
- [x] [1.2] [P-1] Add `strip_tool_error_lines()` — remove lines matching `^Error:` that also contain infrastructure keywords (`not found in registry`, `known projects`, `unreachable`, `connection refused`, `timed out`, `no such tool`, `tool execution failed`, `failed to execute`) [owner:api-engineer]
- [x] [1.3] [P-1] Add `strip_score_annotated_results()` — remove search result blocks starting with a line matching `^\S+\.\w+ \(score: \d+(\.\d+)?\):` plus subsequent non-empty continuation lines until a blank line or a new sentence [owner:api-engineer]
- [x] [1.4] [P-1] Extend `strip_preamble()` with internal-reasoning patterns — add "nexus is unreachable", "can't inspect", "cannot inspect", "no such tool", "let me try", "i'll try", "falling back to", "the tool returned", "i don't have access to", "unable to access", "that tool" + failure variants; apply only to short lines (< 150 chars) [owner:api-engineer]
- [x] [1.5] [P-2] Wire new strip functions into `strip_tool_call_artifacts()` — call in order: existing fenced-block loop, `strip_called_tool_lines()`, `strip_tool_response_xml()`, `strip_tool_error_lines()`, `strip_score_annotated_results()`, preamble pass, then final trim [owner:api-engineer]

## Empty Message Suppression

- [x] [2.1] [P-1] Add thinking indicator cleanup in `process_task()` — when `response_text` is empty after stripping and `thinking_msg_id` is Some, call `tg.delete_message(chat_id, msg_id)` to remove the "..." indicator; log at debug level [owner:api-engineer]
- [x] [2.2] [P-2] Confirm `TelegramClient` has a `delete_message()` method; if not, add it (simple `deleteMessage` Bot API call) [owner:api-engineer]

## Tests

- [x] [3.1] [P-1] Add unit tests for `strip_tool_response_xml()` — basic block, nested content, multiple blocks, inside-code-fence preservation, no-match passthrough [owner:api-engineer]
- [x] [3.2] [P-1] Add unit tests for `strip_tool_error_lines()` — infrastructure error removed, user-facing error preserved, mixed content, multi-line [owner:api-engineer]
- [x] [3.3] [P-1] Add unit tests for `strip_score_annotated_results()` — single result block, multiple blocks, non-matching lines preserved, result followed by real content [owner:api-engineer]
- [x] [3.4] [P-1] Add unit tests for expanded preamble — each new pattern matched, long lines preserved, legitimate short responses preserved [owner:api-engineer]
- [x] [3.5] [P-2] Add integration test for full `strip_tool_call_artifacts()` pipeline — input containing all 5 artifact types produces clean output with only the real response [owner:api-engineer]

## Verify

- [x] [4.1] cargo build passes [owner:api-engineer]
- [x] [4.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [x] [4.3] cargo test — all new + existing strip tests pass [owner:api-engineer]
