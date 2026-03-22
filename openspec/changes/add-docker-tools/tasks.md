# Implementation Tasks

<!-- beads:epic:TBD -->

## API Batch

- [ ] [2.1] [P-1] Create `crates/nv-daemon/src/docker.rs` module — define `DockerClient` struct with unix socket path field (default `/var/run/docker.sock`) [owner:api-engineer]
- [ ] [2.2] [P-1] Add unix socket HTTP client setup in DockerClient — use hyperlocal or reqwest unix feature to issue HTTP requests to Docker Engine API via socket [owner:api-engineer]
- [ ] [2.3] [P-1] Implement `DockerClient::status()` — GET /v1.43/containers/json, parse JSON response into Vec of container summaries (name, image, state, status, ports), accept `all` bool param [owner:api-engineer]
- [ ] [2.4] [P-1] Format docker_status output as concise text table — columns: Name | Image | State | Uptime | Ports [owner:api-engineer]
- [ ] [2.5] [P-1] Implement `DockerClient::logs()` — GET /v1.43/containers/{name}/logs?stdout=true&stderr=true&tail={lines}, strip 8-byte Docker log frame headers, return raw text [owner:api-engineer]
- [ ] [2.6] [P-1] Add input validation for docker_logs — container name required, lines capped at 200, default 50 [owner:api-engineer]
- [ ] [2.7] [P-1] Register `docker_status` and `docker_logs` tool definitions in tools.rs register_tools() with input schemas [owner:api-engineer]
- [ ] [2.8] [P-1] Add execution handlers for docker_status and docker_logs in execute_tool()/execute_tool_send() — instantiate DockerClient, call method, return ToolResult::Immediate [owner:api-engineer]
- [ ] [2.9] [P-2] Add audit logging after each docker tool invocation — call log_tool_usage() with tool name, params, truncated result, duration [owner:api-engineer]
- [ ] [2.10] [P-2] Add `mod docker;` declaration in main.rs or lib.rs [owner:api-engineer]
- [ ] [2.11] [P-2] Add socket availability check on startup — if /var/run/docker.sock missing or unreadable, log warning and skip docker tool registration [owner:api-engineer]

## Verify

- [ ] [3.1] cargo build passes [owner:api-engineer]
- [ ] [3.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [ ] [3.3] Unit test: DockerClient::status() parses sample Docker API JSON response correctly [owner:api-engineer]
- [ ] [3.4] Unit test: docker_status output format contains expected columns [owner:api-engineer]
- [ ] [3.5] Unit test: DockerClient::logs() strips 8-byte frame headers from log lines [owner:api-engineer]
- [ ] [3.6] Unit test: docker_logs caps lines at 200 when larger value requested [owner:api-engineer]
- [ ] [3.7] Unit test: tool registration includes docker_status and docker_logs [owner:api-engineer]
- [ ] [3.8] Existing tests pass [owner:api-engineer]
