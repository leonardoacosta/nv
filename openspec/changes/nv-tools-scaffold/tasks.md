# Implementation Tasks

<!-- beads:epic:TBD -->

## DB Batch: ToolDefinition Migration

- [ ] [1.1] [P-1] Move `ToolDefinition` struct from `crates/nv-daemon/src/claude.rs` to new `crates/nv-core/src/tool.rs` -- include all fields (name, description, input_schema) and derives (Clone, Debug, Serialize) [owner:api-engineer]
- [ ] [1.2] [P-1] Add `pub mod tool; pub use tool::ToolDefinition;` to `crates/nv-core/src/lib.rs` [owner:api-engineer]
- [ ] [1.3] [P-1] Update `crates/nv-daemon/src/claude.rs` -- remove `ToolDefinition` struct definition, add `pub use nv_core::ToolDefinition;` re-export [owner:api-engineer]
- [ ] [1.4] [P-1] Update all `use crate::claude::ToolDefinition` imports across `crates/nv-daemon/src/tools/*.rs` (20 files + jira/) to `use nv_core::ToolDefinition` [owner:api-engineer]

## API Batch: MCP Server Skeleton

- [ ] [2.1] [P-1] Add `crates/nv-tools` to workspace `members` in root `Cargo.toml` [owner:api-engineer]
- [ ] [2.2] [P-1] Create `crates/nv-tools/Cargo.toml` -- depend on `nv-core`, `tokio`, `serde_json`, `serde`, `anyhow`, `tracing`, `tracing-subscriber` from workspace [owner:api-engineer]
- [ ] [2.3] [P-1] Create `crates/nv-tools/src/main.rs` -- tokio main, init tracing, create McpServer, run stdio loop (read lines from stdin, parse JSON-RPC, dispatch, write response to stdout) [owner:api-engineer]
- [ ] [2.4] [P-1] Create `crates/nv-tools/src/server.rs` -- `McpServer` struct with `handle_request(&self, request: Value) -> Value` dispatching to `initialize`, `tools/list`, `tools/call` methods [owner:api-engineer]
- [ ] [2.5] [P-2] Implement `initialize` handler -- return server info (name: "nv-tools", version from Cargo.toml, capabilities: { tools: {} }) [owner:api-engineer]
- [ ] [2.6] [P-2] Implement `tools/list` handler -- return empty tools array (populated in later specs) [owner:api-engineer]
- [ ] [2.7] [P-2] Implement `tools/call` handler -- return "tool not found" error for any call (populated in later specs) [owner:api-engineer]
- [ ] [2.8] [P-2] Create `crates/nv-tools/src/registry.rs` -- `ToolRegistry` struct holding `Vec<ToolDefinition>` + `HashMap<String, Box<dyn ToolHandler>>` for dispatch [owner:api-engineer]

## Verify

- [ ] [3.1] `cargo check --workspace` passes with zero errors [owner:api-engineer]
- [ ] [3.2] `cargo test -p nv-daemon --lib` -- all 1,032 tests still pass (ToolDefinition move is mechanical) [owner:api-engineer]
- [ ] [3.3] `cargo build -p nv-tools` -- nv-tools binary compiles [owner:api-engineer]
- [ ] [3.4] `cargo clippy --workspace -- -D warnings` passes [owner:api-engineer]
