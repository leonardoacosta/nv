# Implementation Tasks

<!-- beads:epic:TBD -->

## Config

- [x] [1.1] [P-1] Add `DopplerConfig` struct to `crates/nv-core/src/config.rs` — `projects: HashMap<String, String>` mapping aliases to Doppler project names [owner:api-engineer]
- [x] [1.2] [P-1] Add `pub doppler: Option<DopplerConfig>` field to the `Config` struct in `crates/nv-core/src/config.rs` [owner:api-engineer]

## DopplerClient

- [x] [2.1] [P-1] Create `crates/nv-daemon/src/doppler_tools.rs` with `DopplerClient` struct — `from_env()` reads `DOPPLER_API_TOKEN`, shared `reqwest::Client` with 15s timeout, Bearer auth, `map_status()` for 401/403/404/429 errors [owner:api-engineer]
- [x] [2.2] [P-1] Add project alias resolution helper — accepts `Option<&DopplerConfig>`, returns resolved Doppler project name or passes through the raw input [owner:api-engineer]

## Tool: doppler_secrets

- [x] [3.1] [P-1] Implement `doppler_secrets()` — calls `GET /v3/configs/config/secrets?project={}&config={}`, extracts only JSON object keys (secret names), sorts alphabetically, formats as readable list with count. Never deserialize values. [owner:api-engineer]
- [x] [3.2] [P-1] Add `doppler_secrets` tool definition in `doppler_tool_definitions()` — params: `project` (string, required), `environment` (string, required) [owner:api-engineer]

## Tool: doppler_compare

- [x] [4.1] [P-1] Implement `doppler_compare()` — calls secrets API twice (env_a, env_b), collects name sets, computes symmetric difference, formats three sections: "Only in env_a", "Only in env_b", "Common (N secrets)" [owner:api-engineer]
- [x] [4.2] [P-1] Add `doppler_compare` tool definition in `doppler_tool_definitions()` — params: `project` (string, required), `env_a` (string, required), `env_b` (string, required) [owner:api-engineer]

## Tool: doppler_activity

- [x] [5.1] [P-1] Implement `doppler_activity()` — calls `GET /v3/logs?project={}`, formats recent entries with timestamp, user, and action text. Clamp count to 1..=25, default 10. [owner:api-engineer]
- [x] [5.2] [P-1] Add `doppler_activity` tool definition in `doppler_tool_definitions()` — params: `project` (string, required), `count` (integer, optional) [owner:api-engineer]

## Registration & Wiring

- [x] [6.1] [P-1] Add `mod doppler_tools;` to `crates/nv-daemon/src/main.rs` [owner:api-engineer]
- [x] [6.2] [P-1] In `crates/nv-daemon/src/tools.rs`: import `doppler_tools`, call `tools.extend(doppler_tools::doppler_tool_definitions())` in `register_tools()` [owner:api-engineer]
- [x] [6.3] [P-1] Add dispatch arms for `doppler_secrets`, `doppler_compare`, `doppler_activity` in both `execute_tool` and `execute_tool_send` — instantiate `DopplerClient::from_env()`, resolve project alias from config, call tool function, return `ToolResult::Immediate` [owner:api-engineer]

## Verify

- [x] [7.1] `cargo build` passes [owner:api-engineer]
- [x] [7.2] `cargo clippy -- -D warnings` passes [owner:api-engineer]
- [x] [7.3] `cargo test` — existing tests pass [owner:api-engineer]
