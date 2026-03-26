# Implementation Tasks

<!-- beads:epic:nv-tzd0 -->

## API Batch — Daemon Tool-Call Endpoint

- [ ] [1.1] [P-1] Add `POST /api/tool-call` endpoint to `http.rs` — accepts `{ "tool_name": "string", "input": {} }`, routes to `tools::execute_tool_send_with_backend`, returns `{ "result": "string", "error": null }` or error; restrict to 127.0.0.1 origin [owner:api-engineer]
- [ ] [1.2] [P-2] Unit test: POST /api/tool-call with `read_memory` tool returns memory content [owner:api-engineer]

## API Batch — Python Sidecar Scripts

- [ ] [2.1] [P-1] Create `scripts/agent-sidecar.py` — long-running process reading JSON from stdin, calling `claude_agent_sdk.query()` with system_prompt + prompt + custom MCP server, writing JSON response to stdout; handle timeouts via `max_turns` and asyncio timeout; log errors to stderr [owner:api-engineer]
- [ ] [2.2] [P-1] Create `scripts/nova-tools-mcp.py` — MCP server module using `claude_agent_sdk.create_sdk_mcp_server` that registers all Nova tools (from the tools list in the request) and executes them via HTTP POST to `http://127.0.0.1:8400/api/tool-call`; import as module in agent-sidecar.py [owner:api-engineer]
- [ ] [2.3] [P-2] Handle tool definitions dynamically — each sidecar request includes a `tools` array; the MCP server registers those tools for that request's `query()` call [owner:api-engineer]
- [ ] [2.4] [P-2] Add graceful shutdown — catch SIGTERM, cancel any in-flight query, flush stdout, exit cleanly [owner:api-engineer]

## API Batch — Rust Sidecar Manager

- [ ] [3.1] [P-1] Create `crates/nv-daemon/src/sidecar.rs` — `SidecarManager` struct with `spawn()`, `send_request()`, `shutdown()` methods; spawns `python3 scripts/agent-sidecar.py` as child process; holds stdin BufWriter + stdout BufReader handles [owner:api-engineer]
- [ ] [3.2] [P-1] Implement `SidecarManager::send_request(req: SidecarRequest) -> Result<SidecarResponse>` — serialize request as JSON line to stdin, read JSON line response from stdout, with configurable timeout [owner:api-engineer]
- [ ] [3.3] [P-2] Implement crash recovery — if sidecar process exits unexpectedly, log error, wait 5s, respawn; max 3 restarts before giving up and falling back to ClaudeClient [owner:api-engineer]
- [ ] [3.4] [P-2] Add `mod sidecar;` to lib.rs and main.rs; spawn sidecar on daemon startup; store in SharedDeps as `pub sidecar: Option<Arc<SidecarManager>>` [owner:api-engineer]

## API Batch — Worker + Executor Integration

- [ ] [4.1] [P-1] In `Worker::run`: if `deps.sidecar` is Some, send the Claude request through the sidecar instead of AnthropicClient/ClaudeClient; construct `SidecarRequest` from system_prompt + conversation + tool_definitions; parse `SidecarResponse` into the existing `ApiResponse` format for downstream processing [owner:api-engineer]
- [ ] [4.2] [P-1] In `obligation_executor.rs`: if `deps.sidecar` is Some, use it instead of AnthropicClient; the sidecar handles the entire tool loop internally, so the executor receives the final result without needing its own tool loop [owner:api-engineer]
- [ ] [4.3] [P-2] Fallback chain: sidecar (preferred) → AnthropicClient (if sidecar unavailable) → ClaudeClient (last resort); log which path is used at startup [owner:api-engineer]
- [ ] [4.4] [P-2] Remove the manual tool loop from obligation_executor — the Agent SDK handles tool execution via MCP; executor just sends the obligation context and receives the final summary [owner:api-engineer]

## Deploy

- [ ] [5.1] [P-1] Add `pip3 install claude-agent-sdk` to `deploy/install.sh` [owner:api-engineer]
- [ ] [5.2] [P-2] Add sidecar health check — on daemon startup, send a no-op test request to verify sidecar is responsive; log result [owner:api-engineer]

## Verify

- [ ] [6.1] `cargo build -p nv-daemon` passes [owner:api-engineer]
- [ ] [6.2] `python3 scripts/agent-sidecar.py --test` runs a self-test that verifies SDK import + MCP tool registration [owner:api-engineer]
- [ ] [6.3] [user] Send a Telegram message to Nova requiring tool use (e.g., "check Jira status"), verify the response includes real tool data (not a text-only summary)
- [ ] [6.4] [user] Verify obligation executor uses tools — create a Nova obligation, wait for idle execution, verify Telegram summary references actual tool results
- [ ] [6.5] [user] Kill the sidecar process manually, send a message, verify daemon restarts the sidecar and falls back to ClaudeClient in the meantime
