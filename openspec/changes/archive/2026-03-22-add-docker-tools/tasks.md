# Implementation Tasks

<!-- beads:epic:TBD -->

## API Batch

- [x] [2.1] [P-1] Create `crates/nv-daemon/src/docker_tools.rs` module — define docker_status() and docker_logs() using Command::new("docker") [owner:api-engineer]
- [x] [2.2] [P-1] Add CLI-based docker client (Command::new("docker") with tokio::process) — no socket/hyperlocal dependency needed [owner:api-engineer]
- [x] [2.3] [P-1] Implement `docker_status()` — runs `docker ps --format` with Go template, parses tab-separated output into container summaries (name, image, state, status, ports), accepts `all` bool param [owner:api-engineer]
- [x] [2.4] [P-1] Format docker_status output as concise text table — columns: Name | Image | State | Uptime | Ports [owner:api-engineer]
- [x] [2.5] [P-1] Implement `docker_logs()` — runs `docker logs --tail {lines} --timestamps {container}`, merges stdout+stderr, truncates to 10KB [owner:api-engineer]
- [x] [2.6] [P-1] Add input validation for docker_logs — container name required, rejects shell metacharacters, lines capped at 200, default 50 [owner:api-engineer]
- [x] [2.7] [P-1] Register `docker_status` and `docker_logs` tool definitions in tools.rs register_tools() with input schemas [owner:api-engineer]
- [x] [2.8] [P-1] Add execution handlers for docker_status and docker_logs in execute_tool()/execute_tool_send() — call docker_tools functions, return ToolResult::Immediate [owner:api-engineer]
- [x] [2.9] [P-2] Audit logging handled by existing worker infrastructure — worker.rs log_tool_usage() already logs every tool invocation with name, params, result, duration, success [owner:api-engineer]
- [x] [2.10] [P-2] Add `mod docker_tools;` declaration in main.rs [owner:api-engineer]
- [x] [2.11] [P-2] Add `is_docker_available()` check function (available for startup gating) [owner:api-engineer]

## Verify

- [x] [3.1] cargo build passes [owner:api-engineer]
- [x] [3.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [x] [3.3] Unit test: truncate helper works correctly (short unchanged, long adds ..) [owner:api-engineer]
- [x] [3.4] Unit test: docker_logs rejects empty container name [owner:api-engineer]
- [x] [3.5] Unit test: docker_logs rejects shell metacharacters (;, $, |, &, `) [owner:api-engineer]
- [x] [3.6] Unit test: docker_logs caps lines at 200 when larger value requested [owner:api-engineer]
- [x] [3.7] Unit test: tool registration includes docker_status and docker_logs (count=29) [owner:api-engineer]
- [x] [3.8] Existing tests pass (568 total, 0 failures) [owner:api-engineer]
