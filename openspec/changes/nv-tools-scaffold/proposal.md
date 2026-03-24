# Proposal: nv-tools MCP Server Scaffold

## Change ID
`nv-tools-scaffold`

## Summary

Create the `crates/nv-tools` workspace crate with an MCP server skeleton (stdio transport),
move `ToolDefinition` from `nv-daemon/src/claude.rs` to `nv-core`, and update all imports.

## Context
- Phase 2 of cc-native-nova migration (nv-k86)
- `ToolDefinition` is currently in `nv-daemon/src/claude.rs` line 144
- Every tool file imports `crate::claude::ToolDefinition` -- moving it to nv-core unblocks extraction
- MCP protocol: JSON-RPC over stdio (initialize, tools/list, tools/call)

## Why

The MCP server needs `ToolDefinition` to register tools. Currently it's in nv-daemon which would
create a circular dependency. Moving to nv-core makes it available to both crates.

## What Changes

### New crate: `crates/nv-tools`
- `Cargo.toml` depending on `nv-core`, `tokio`, `serde_json`, `anyhow`, `tracing`
- `src/main.rs` -- stdio MCP entry point (read stdin JSON-RPC, write stdout)
- `src/server.rs` -- `McpServer` struct with `handle_request()` dispatcher
- `src/registry.rs` -- `ToolRegistry` for tool definitions + dispatch functions

### Modified: `nv-core`
- New `src/tool.rs` with `ToolDefinition` struct (moved from claude.rs)
- Re-export from `lib.rs`

### Modified: `nv-daemon`
- `claude.rs`: remove `ToolDefinition` struct, add `pub use nv_core::ToolDefinition`
- All `tools/*.rs`: update `use crate::claude::ToolDefinition` to `use nv_core::ToolDefinition`

## Scope
- **IN**: Crate scaffold, ToolDefinition migration, empty MCP server, protocol skeleton
- **OUT**: Actual tool registration (that's specs 2-5), HTTP transport

## Impact
| Area | Change |
|------|--------|
| `Cargo.toml` | Add `crates/nv-tools` to workspace members |
| `crates/nv-core/src/tool.rs` | New file -- ToolDefinition struct |
| `crates/nv-core/src/lib.rs` | Add `pub mod tool; pub use tool::ToolDefinition;` |
| `crates/nv-daemon/src/claude.rs` | Remove struct, re-export from nv-core |
| `crates/nv-daemon/src/tools/*.rs` (20 files) | Update import path |
| `crates/nv-tools/` (new) | Entire new crate |

## Risks
| Risk | Mitigation |
|------|-----------|
| ToolDefinition move breaks daemon compile | Gate on `cargo check --workspace` before any other changes |
| MCP protocol implementation wrong | Use minimal JSON-RPC subset -- just initialize + tools/list + tools/call |
