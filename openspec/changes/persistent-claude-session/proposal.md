# Proposal: Persistent Claude Session

## Change ID
`persistent-claude-session`

## Summary

Replace cold-start `claude -p` per turn with a long-lived CLI subprocess using
`--input-format stream-json` + `--output-format stream-json`. Keep the subprocess alive between
turns. Pipe messages via stdin, parse events from stdout. Falls back to cold-start mode if the
persistent session fails.

## Context
- Extends: `crates/nv-daemon/src/claude.rs` (refactor `ClaudeClient` from spawn-per-turn to persistent subprocess)
- Related: `crates/nv-daemon/src/agent.rs` (calls `ClaudeClient::send_messages()` each turn)

## Motivation

Each agent turn currently spawns a new `claude -p` subprocess (~8-14s cold start). This is the
single biggest UX pain. The Claude CLI supports `--input-format stream-json` and
`--output-format stream-json` for long-lived sessions where messages are piped via stdin and
events stream from stdout. Switching to a persistent subprocess eliminates cold-start latency,
bringing response time from 8-14s down to ~2s.

Benefits:
1. **Latency** -- eliminate cold-start overhead per turn
2. **Session continuity** -- the CLI maintains conversation state across turns
3. **OAuth compatibility** -- no API key needed, works with existing CLI auth
4. **Graceful degradation** -- falls back to current cold-start mode if subprocess dies

## Requirements

### Req-1: Persistent Subprocess Management

Refactor `ClaudeClient` to spawn a single `claude` subprocess on first use (or daemon startup)
with `--input-format stream-json` and `--output-format stream-json` flags. The subprocess is kept
alive as a `tokio::process::Child` behind an `Arc<Mutex<>>` or similar. The subprocess is reused
across all `send_messages()` calls.

### Req-2: Stdin Writer (Stream-JSON Format)

Messages are sent to the subprocess via stdin in the Claude stream-json input format. Each message
is a JSON object written as a single line followed by a newline. The writer must handle
serialization of the conversation (system prompt, user messages, tool results) into the expected
format.

### Req-3: Stdout Reader (Stream-JSON Events)

Stdout emits stream-json events: `message_start`, `content_block_start`, `content_block_delta`,
`content_block_stop`, `message_delta`, `message_stop`, and `result`. The reader parses these
incrementally, accumulating text content and detecting tool_use blocks. The reader produces an
`ApiResponse` compatible with the existing interface so the agent loop requires no changes to
response handling.

### Req-4: Auto-Restart with Backoff

If the subprocess exits unexpectedly (crash, signal, OOM), the client detects the dead process
and restarts it with exponential backoff (1s, 2s, 4s, max 30s). A counter tracks consecutive
failures. After 5 consecutive restart failures, the client switches to fallback mode.

### Req-5: Fallback to Cold-Start Mode

If the persistent subprocess cannot be maintained (repeated crashes, unsupported CLI version),
fall back to the existing `claude -p` per-turn invocation. The fallback is transparent to the
agent loop -- `send_messages()` returns the same `ApiResponse` regardless of mode. A tracing log
warns when fallback is active.

### Req-6: Agent Loop Integration

Update the agent loop to use the persistent client. The `send_messages()` signature stays the
same -- internal implementation switches from spawn-per-turn to write-to-stdin. No changes to
tool call parsing, response routing, or error handling in the agent loop.

## Scope
- **IN**: Persistent subprocess lifecycle, stdin writer, stdout reader, auto-restart, fallback, agent loop wiring
- **OUT**: Session resume (`--continue`), direct API calls, Agent SDK integration, streaming to Telegram (progressive message updates)

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/claude.rs` | Refactor ClaudeClient: persistent Child, stdin writer, stdout reader, restart logic, fallback |
| `crates/nv-daemon/src/agent.rs` | Minimal: use same `send_messages()` interface, pass startup config |
| `crates/nv-daemon/src/main.rs` | Initialize persistent client at startup, pass to agent loop |

## Risks
| Risk | Mitigation |
|------|-----------|
| CLI stream-json format undocumented or changes between versions | Pin CLI version; parse defensively with serde; fallback to cold-start |
| Subprocess memory leak over long uptime | Monitor RSS via /proc; restart if exceeding threshold |
| Stdin/stdout deadlock (full pipe buffer) | Use async readers with bounded buffers; timeout on writes |
| Concurrent send_messages() calls while subprocess is single-threaded | Serialize access via Mutex; queue requests |
