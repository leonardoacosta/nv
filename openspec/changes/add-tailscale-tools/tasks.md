# Implementation Tasks

<!-- beads:epic:TBD -->

## API Batch

- [x] [2.1] [P-1] Create `crates/nv-daemon/src/tailscale.rs` module — define `TailscaleClient` struct, define `TailscaleStatus` and `TailscalePeer` structs with serde Deserialize for JSON parsing [owner:api-engineer]
- [x] [2.2] [P-1] Implement `TailscaleClient::run_status_cmd()` — execute `docker exec tailscale tailscale status --json` via tokio::process::Command, capture stdout, 5s timeout [owner:api-engineer]
- [x] [2.3] [P-1] Implement `TailscaleClient::status()` — call run_status_cmd(), parse JSON into TailscaleStatus, format as text table: Hostname | Online | IP | OS | Last Seen (sorted: online first) [owner:api-engineer]
- [x] [2.4] [P-1] Implement `TailscaleClient::node()` — accept name param, case-insensitive match against Peer hostnames, return detailed info (hostname, DNSName, online, active, all IPs, OS, relay, last_seen) or error if not found [owner:api-engineer]
- [x] [2.5] [P-1] Register `tailscale_status` and `tailscale_node` tool definitions in tools.rs register_tools() with input schemas [owner:api-engineer]
- [x] [2.6] [P-1] Add execution handlers for tailscale_status and tailscale_node in execute_tool()/execute_tool_send() — instantiate TailscaleClient, call method, return ToolResult::Immediate [owner:api-engineer]
- [x] [2.7] [P-2] Add audit logging after each tailscale tool invocation — call log_tool_usage() with tool name, params, truncated result, duration [owner:api-engineer]
- [x] [2.8] [P-2] Add `mod tailscale;` declaration in main.rs or lib.rs [owner:api-engineer]
- [x] [2.9] [P-2] Add startup check — verify `docker inspect tailscale` succeeds, log warning and skip tool registration if container not found [owner:api-engineer]

## Verify

- [x] [3.1] cargo build passes [owner:api-engineer]
- [x] [3.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [x] [3.3] Unit test: TailscaleStatus JSON deserialization from sample `tailscale status --json` output [owner:api-engineer]
- [x] [3.4] Unit test: status() formats table with online nodes sorted first [owner:api-engineer]
- [x] [3.5] Unit test: node() finds peer by case-insensitive hostname match [owner:api-engineer]
- [x] [3.6] Unit test: node() returns error for unknown hostname [owner:api-engineer]
- [x] [3.7] Unit test: tool registration includes tailscale_status and tailscale_node [owner:api-engineer]
- [x] [3.8] Existing tests pass [owner:api-engineer]
