# Proposal: Fix Chat Bugs

## Change ID
`fix-chat-bugs`

## Summary

Bundle four critical chat bugs into one spec: strip raw tool_call blocks from response text,
handle empty/truncated CLI JSON with retry + fallback, add per-tool timeout (30s default) for
stalled execution, and convert markdown tables to `<pre>` blocks in Telegram HTML.

## Context
- Extends: `crates/nv-daemon/src/worker.rs` (tool loop, extract_text)
- Extends: `crates/nv-daemon/src/agent.rs` (extract_text, tool execution)
- Extends: `crates/nv-daemon/src/telegram/client.rs` (markdown_to_html)
- Extends: `crates/nv-daemon/src/claude.rs` (session JSON parsing)
- Related: PRD §4 (Phase 0: Fix Critical Bugs)

## Motivation

Four user-facing bugs degrade trust in Nova's chat:

1. **Tool call JSON leak** — raw `tool_call` blocks appear in Telegram when Claude outputs them
   in the response content. `extract_text()` passes them through unfiltered.
2. **Worker deserialization crash** — empty or truncated JSON from Claude subprocess causes
   `serde_json` parse failure. No retry, no fallback, worker just errors out.
3. **Stalled tool calls** — tool execution can hang indefinitely. Worker self-reports stalls but
   has no timeout mechanism to recover.
4. **Markdown table rendering** — tables render as raw `|------|` pipe separators in Telegram
   because `markdown_to_html()` has no table handling.

These are all Phase 0 blockers before adding new tools.

## Requirements

### Req-1: Strip Tool Call Blocks

Add regex filter to `extract_text()` in both `worker.rs` and `agent.rs` that removes markdown
fenced code blocks tagged as `tool_call`, `tool_use`, or `json` (when containing tool invocation
patterns). Only the final text response after all tool loops complete should reach Telegram.

### Req-2: Handle Empty CLI JSON

In the Claude session JSON parsing path, detect empty or unparseable stdout. On first failure,
retry once. If retry fails, log the error, send a user-facing message to Telegram
("Thinking failed — retrying..."), and fall back to cold-start mode for this turn. No panic,
no crash.

### Req-3: Per-Tool Timeout

Wrap every `execute_tool()` / `execute_tool_send()` call in `tokio::time::timeout(Duration)`.
Default: 30 seconds. If a tool call exceeds the timeout, return an error result to Claude
("Tool timed out after 30s") and let it continue without that data. The timeout value should
be configurable per tool category (read tools: 30s, write tools: 60s).

### Req-4: Markdown Table Rendering

Extend `markdown_to_html()` in `telegram/client.rs` to detect markdown table patterns
(lines with `|` delimiters) and convert them to `<pre>` blocks with aligned columns.
Telegram does not support `<table>` HTML, so `<pre>` with monospace alignment is the correct
approach. Strip the separator row (`|------|`) and pad columns for readability.

## Scope
- **IN**: extract_text filter, JSON parse retry, tool timeout wrapper, table-to-pre conversion
- **OUT**: Rich Telegram formatting (inline keyboards, embeds), streaming responses, new tools

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/worker.rs` | Add tool_call block stripping to `extract_text()`, wrap tool calls in `tokio::time::timeout` |
| `crates/nv-daemon/src/agent.rs` | Same extract_text filter, same timeout wrapper |
| `crates/nv-daemon/src/claude.rs` | Add retry logic for empty/truncated JSON responses |
| `crates/nv-daemon/src/telegram/client.rs` | Add table detection and `<pre>` conversion to `markdown_to_html()` |

## Risks
| Risk | Mitigation |
|------|-----------|
| Regex strips legitimate code blocks containing "tool_call" | Narrow regex to match only the exact fenced block format Claude uses |
| Timeout too aggressive for slow tools (Jira, Nexus) | Configurable per-category; 30s for reads is generous (current tools return in <5s) |
| Table alignment breaks with wide Unicode characters | Use byte-width alignment; acceptable for v1 since tables are typically ASCII |
