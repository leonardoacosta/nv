# Proposal: Replace AnthropicClient with Agent SDK Sidecar

## Change ID
`replace-anthropic-with-agent-sdk`

## Summary

Replace the raw `AnthropicClient` HTTP client with a Python Agent SDK sidecar process that
accepts OAuth tokens from Claude Code auth. Nova's custom tools are exposed to the Agent SDK
as an MCP server, giving Claude native tool_use with zero API key management.

## Context
- Extends: `crates/nv-daemon/src/anthropic.rs` (replaced), `crates/nv-daemon/src/worker.rs`
  (Claude call path), `crates/nv-daemon/src/obligation_executor.rs` (executor call path)
- Related: CC CLI v2.1.84 installed, OAuth credentials at `~/.claude/.credentials.json`,
  existing `ClaudeClient` wraps CC CLI via stdin/stdout
- Replaces: `AnthropicClient` (raw HTTP to api.anthropic.com, requires `sk-ant-api03` key)

## Motivation

The Anthropic Messages API does not accept OAuth tokens (`sk-ant-oat`). The Claude Code CLI on
this machine lacks `--tools-json` support, so tool definitions aren't passed to Claude. The result:
Nova's tools fail silently on every interactive message and autonomous obligation.

The Agent SDK (Python) uses the Claude Code CLI internally, which handles OAuth auth natively.
It also provides a proper tool execution protocol via MCP. By running a Python Agent SDK process
as a sidecar, Nova gets:

1. **OAuth token support** — no separate API key needed, uses existing Claude Code credentials
2. **Native tool_use protocol** — tools defined via MCP, Claude calls them properly
3. **Built-in retry/backoff** — SDK handles rate limits and errors
4. **Streaming** — SDK supports streaming responses natively

## Requirements

### Req-1: Python Sidecar Script

Create `scripts/agent-sidecar.py` — a long-running Python process that:

1. Reads JSON requests from stdin (one per line, newline-delimited)
2. For each request, calls `claude_agent_sdk.query()` with:
   - `prompt` from the request
   - `system_prompt` from the request
   - Custom MCP server exposing Nova's tools
   - `permission_mode: "bypassPermissions"` (Nova is autonomous)
   - `max_turns` from the request (default 30)
3. Collects the response (tool calls are handled by the MCP server)
4. Writes JSON response to stdout (one per line)

Request format:
```json
{
  "id": "req-uuid",
  "system": "You are Nova...",
  "prompt": "Work on this obligation: ...",
  "tools": [{"name": "teams_list_chats", "description": "...", "input_schema": {...}}],
  "max_turns": 30,
  "timeout_secs": 300
}
```

Response format:
```json
{
  "id": "req-uuid",
  "content": [{"type": "text", "text": "I completed the work..."}],
  "stop_reason": "end_turn",
  "tool_calls": [{"name": "teams_list_chats", "input": {...}, "result": "..."}],
  "error": null
}
```

### Req-2: MCP Tool Bridge

Create `scripts/nova-tools-mcp.py` — an MCP server that bridges Agent SDK tool calls to
the Rust daemon's tool dispatch.

When Claude (via Agent SDK) calls a tool:
1. The MCP server receives the tool call
2. It forwards the call to the Rust daemon via HTTP: `POST http://localhost:8400/api/tool-call`
3. The daemon executes the tool using existing `tools::execute_tool_send_with_backend`
4. The result is returned to the MCP server, then to Claude

This requires a new daemon endpoint: `POST /api/tool-call` that accepts
`{ "tool_name": "...", "input": {...} }` and returns `{ "result": "..." }`.

### Req-3: Daemon Tool-Call Endpoint

Add `POST /api/tool-call` to `http.rs` that:
1. Accepts `{ "tool_name": "string", "input": {} }` JSON body
2. Routes to `tools::execute_tool_send_with_backend` using the same dispatch as Worker::run
3. Returns `{ "result": "string", "error": null }` or `{ "result": null, "error": "message" }`
4. Requires a local-only guard (only accept from 127.0.0.1 / Docker internal)

### Req-4: Sidecar Process Manager

Add sidecar lifecycle management to the daemon:

1. On daemon startup: spawn `python3 scripts/agent-sidecar.py` as a child process
2. Hold stdin/stdout handles for communication
3. On daemon shutdown: send SIGTERM to sidecar, wait 5s, SIGKILL if needed
4. If sidecar crashes: log error, restart after 5s delay, max 3 restarts

The sidecar replaces both `AnthropicClient` and the `ClaudeClient` cold-start path for
Claude API calls. The existing `ClaudeClient` persistent session path stays as fallback.

### Req-5: Worker Integration

Replace the Claude call in `Worker::run`:

```rust
// Before (AnthropicClient):
let response = anthropic.send_message(&conversation, &system, &tools).await?;

// After (sidecar):
let response = sidecar.send_request(SidecarRequest {
    system: system_prompt,
    prompt: user_message,
    tools: tool_definitions,
    max_turns: 30,
    timeout_secs: 300,
}).await?;
```

The sidecar response contains the final text + list of tool calls that were made.
The worker no longer runs its own tool loop — the Agent SDK handles that via MCP.

### Req-6: Obligation Executor Integration

Same pattern as Worker — the obligation executor sends a request to the sidecar instead
of calling `AnthropicClient` directly. The sidecar handles the tool loop autonomously.

### Req-7: Installation

Add to `deploy/install.sh`:
```bash
pip3 install claude-agent-sdk --break-system-packages 2>/dev/null || \
  pipx install claude-agent-sdk
```

## Scope
- **IN**: Python sidecar script, MCP tool bridge, daemon tool-call endpoint, sidecar process
  manager, Worker integration, obligation executor integration, install script
- **OUT**: Removing ClaudeClient entirely (stays as fallback), dashboard changes, new tools

## Impact

| Area | Change |
|------|--------|
| `scripts/agent-sidecar.py` | New: Agent SDK sidecar process |
| `scripts/nova-tools-mcp.py` | New: MCP server bridging to daemon tools |
| `crates/nv-daemon/src/http.rs` | Add POST /api/tool-call endpoint |
| `crates/nv-daemon/src/worker.rs` | Replace Claude call with sidecar communication |
| `crates/nv-daemon/src/obligation_executor.rs` | Replace Claude call with sidecar |
| `crates/nv-daemon/src/main.rs` | Spawn and manage sidecar process |
| `deploy/install.sh` | Install claude-agent-sdk Python package |

## Risks

| Risk | Mitigation |
|------|-----------|
| Python dependency adds complexity | Single pip install, no venv needed |
| Sidecar process crashes | Auto-restart with 3-attempt limit, ClaudeClient fallback |
| MCP tool bridge latency | Local HTTP (127.0.0.1), typically <5ms per tool call |
| OAuth token expiry | Claude Code CLI handles token refresh automatically |
| Agent SDK version compatibility | Pin version in install script |
