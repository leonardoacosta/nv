# Proposal: Fix Tool Result Strip Pattern

## Change ID
`fix-tool-result-strip`

## Summary

Harden `strip_tool_call_artifacts()` in worker.rs to catch five categories of tool-related
artifacts that currently leak through to Telegram: `<tool_response>` XML blocks, tool error result
lines, score-annotated search results, Claude "thinking out loud" preamble about tool failures, and
empty-after-stripping messages that should be suppressed (including cleaning up the thinking
indicator).

## Context
- Modifies: `crates/nv-daemon/src/worker.rs` — `strip_tool_call_artifacts()`, `strip_preamble()`,
  `extract_text()`, and the response-send block in `process_task()`
- Related: `strip_called_tool_lines()`, `strip_one_tool_block()` — existing stripping helpers

## Motivation

Nova forwards Claude's responses to Telegram. The existing strip functions catch fenced tool blocks
(`tool_call`, `tool_use`, `json`), single-line `[Called tool: ...]` patterns, and some preamble
phrases. However, five artifact categories still leak through to the user:

1. **`<tool_response>...</tool_response>` XML blocks** — Claude sometimes wraps tool output in XML
   tags that are not fenced code blocks.
2. **Tool error result lines** — lines like `Error: project 'nv' not found in registry. Known
   projects: oo, tc...` echoed after a tool fails.
3. **Score-annotated search results** — memory/search metadata lines matching
   `filename.ext (score: N):` followed by raw content.
4. **Claude reasoning about tool failures** — phrases like "Nexus is unreachable", "can't inspect
   the nv source directly", "no such tool", which are internal reasoning, not user-facing.
5. **Empty messages after stripping** — when all content is stripped, an empty message is sent to
   Telegram, or worse, the "..." thinking indicator is left hanging without being deleted.

## Requirements

### Req-1: Strip `<tool_response>` XML Blocks

Remove `<tool_response>...</tool_response>` blocks (multi-line) from the response text. The regex
should handle nested content and be non-greedy. Must not strip other XML-like tags that Claude may
legitimately use in responses (e.g., code examples with XML).

Pattern: `<tool_response>` ... `</tool_response>` — remove the tags and everything between them.

### Req-2: Strip Tool Error Result Lines

Remove lines that begin with `Error:` and contain tool-infrastructure keywords (registry, not
found, unreachable, timeout, connection refused, etc.). Must NOT strip user-facing error messages
like "Error: your Jira ticket was not found" — only infrastructure/tool-dispatch errors.

Heuristic: strip lines matching `^Error:` that also contain one or more of: `not found in registry`,
`known projects`, `unreachable`, `connection refused`, `timed out`, `no such tool`,
`tool execution failed`, `failed to execute`.

### Req-3: Strip Score-Annotated Search Results

Remove lines matching the pattern `<filename>.<ext> (score: <number>):` and any immediately
following content lines that are clearly raw search result output (indented or continuation lines
until a blank line or new heading).

Pattern: a line matching `^\S+\.\w+ \(score: \d+(\.\d+)?\):` signals the start of a search result
block. Strip that line and subsequent non-empty lines until a blank line, a line starting with a
common sentence pattern (uppercase letter followed by lowercase), or end of text.

### Req-4: Expand Preamble / Internal Reasoning Stripping

Extend `strip_preamble()` and/or add a new `strip_internal_reasoning()` pass to catch Claude's
internal reasoning about tool failures. New patterns to match (case-insensitive, as standalone
sentences or short lines < 150 chars):

- "nexus is unreachable"
- "can't inspect" / "cannot inspect"
- "no such tool"
- "let me check what I can"
- "let me try" / "I'll try"
- "that tool" (+ "isn't available" / "failed" / "doesn't exist")
- "falling back to"
- "the tool returned an error"
- "I don't have access to"
- "unable to access"

These should be stripped only when they appear as standalone short lines (< 150 chars) that are
clearly transitional/internal. Longer paragraphs containing these phrases should NOT be stripped.

### Req-5: Empty Message Suppression + Thinking Indicator Cleanup

After all stripping, if `response_text` is empty (or whitespace-only):

1. Do NOT send to Telegram (existing `!response_text.is_empty()` guard handles this).
2. If a thinking indicator message was sent (thinking_msg_id is Some), DELETE it from Telegram
   instead of leaving "..." hanging. Use `tg.delete_message(chat_id, msg_id)`.
3. Log at debug level that the response was suppressed after stripping.

## Scope
- **IN**: Five new stripping patterns in `strip_tool_call_artifacts()`, thinking indicator cleanup
  on empty, unit tests for each new pattern, conservative heuristics that prefer showing extra
  over hiding real content
- **OUT**: ML-based classification of tool artifacts, per-tool-type stripping rules, changes to
  how Claude is prompted (system prompt changes), changes to tool execution code

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/worker.rs` — `strip_tool_call_artifacts()` | Add XML block stripping, error line stripping, score-result stripping passes |
| `crates/nv-daemon/src/worker.rs` — `strip_preamble()` | Extend with internal-reasoning patterns |
| `crates/nv-daemon/src/worker.rs` — response send block | Add thinking indicator deletion when response is empty after stripping |
| `crates/nv-daemon/src/worker.rs` — tests | New unit tests for each pattern category |

## Risks
| Risk | Mitigation |
|------|-----------|
| Over-stripping hides legitimate user-facing content | Conservative heuristics: require tool-infrastructure keywords for error lines, short-line constraint for reasoning, specific XML tag names |
| `<tool_response>` tag appears in code examples | Only strip bare `<tool_response>` tags, not those inside fenced code blocks — strip XML before the fenced-block pass would break this, so XML stripping runs after fenced-block stripping and skips content inside triple-backtick regions |
| Score pattern matches non-search content | Pattern requires `(score: N):` suffix which is specific to memory/search output |
| New preamble patterns match legitimate short responses | Length cap (< 150 chars) + must match known infrastructure phrases — "I don't have access to your calendar" could be legitimate, but that's > 150 chars in most cases; keep threshold conservative |
| Thinking indicator delete fails silently | Already in a best-effort block; log at debug level |
