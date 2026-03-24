# Proposal: Fix prompt bloat in Claude CLI integration

## Change ID
`fix-prompt-bloat`

## Summary
Stop embedding the full system prompt, 95 tool schemas, and conversation history into every
persistent-subprocess user message. Send only the user's actual message content, reducing per-turn
payload from 53KB to <1KB.

## Context
- Extends: `crates/nv-daemon/src/claude.rs` (build_stream_input, send_turn, cold-start path)
- Related: tonight's debugging session revealed 53KB prompts causing 2+ minute response times

## Motivation
The persistent Claude CLI subprocess (`--input-format stream-json`) maintains its own conversation
state internally. Despite this, `build_stream_input()` packs the full system prompt (7.6KB), all 95
tool definitions with JSON schemas (~40KB), and the entire conversation history into every turn's
user message content. This content is ALSO duplicated by the CC CLI's own context loading (CLAUDE.md,
hooks, `--tools` flag). The result is massive redundant context on every turn, causing:

- 2+ minute API response times for simple messages like "ping"
- Unnecessarily high token costs (53K+ characters of prompt per turn)
- Cache misses due to the prompt being embedded in user content rather than system context

## Requirements

### Req-1: Persistent path sends only user content
The persistent subprocess turn should send only the latest user message text, not the full
system+tools+history prompt. The CC CLI already receives `--tools` at spawn time and manages its
own conversation state.

### Req-2: Cold-start path passes system prompt via flag
The cold-start fallback already passes `--system-prompt` as a CLI flag. It should NOT also embed
the system prompt inside the user message content. Tool definitions should also be excluded from the
prompt text since `--tools` is already set.

### Req-3: Conversation history excluded from user content
The CC CLI in persistent mode maintains conversation history internally via `stream-json`. Sending
history in the user message duplicates it. The persistent path should send only the current turn's
content.

### Req-4: Log payload size on every turn
Add INFO-level logging of `prompt_bytes`, `system_bytes`, `messages`, and `tools` count on both
persistent and cold-start paths so prompt bloat is immediately visible in production logs. (Already
deployed as part of tonight's logging work -- verify it remains in place.)

## Scope
- **IN**: `build_stream_input()`, `send_turn()`, `send_messages_cold_start_with_image()`,
  `build_prompt()` usage in persistent path, payload size logging
- **OUT**: Tool registration refactoring (custom tools like jira/memory are handled by the worker's
  tool execution loop, not by CC's `--tools` flag), cold-start prompt for image attachments,
  changing the persistent subprocess spawn args

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/claude.rs` | `build_stream_input()` returns only latest user message instead of full prompt |
| `crates/nv-daemon/src/claude.rs` | `send_messages_cold_start_with_image()` removes tool schemas and system prompt from the prompt string (system prompt already passed via `--system-prompt` flag) |
| `crates/nv-daemon/src/claude.rs` | Payload size logging (already deployed, verify retained) |

## Risks
| Risk | Mitigation |
|------|-----------|
| CC CLI may not receive tool context without embedded definitions | CC already gets `--tools Read,Glob,Grep,Bash(git:*)` at spawn; custom tools (jira, memory) are executed by the daemon's tool loop, not by CC |
| Persistent subprocess may lose conversation context | CC's `stream-json` mode maintains state internally; verify with integration test |
| Cold-start may need system prompt in user content | Already passed via `--system-prompt` flag; verify CC receives it |
