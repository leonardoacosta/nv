# Implementation Tasks

<!-- beads:epic:TBD -->

## Dependencies

- `nv-tools-shared-deps`

## API Batch: Smoke Test

- [x] [1.1] [P-1] Add `integration` feature flag to `crates/nv-tools/Cargo.toml` [owner:api-engineer]
- [x] [1.2] [P-1] Create `crates/nv-tools/tests/smoke.rs` with `#[cfg(feature = "integration")]` gate [owner:api-engineer]
- [x] [1.3] [P-1] Test: spawn nv-tools subprocess, send `initialize` JSON-RPC request, assert server info response [owner:api-engineer]
- [x] [1.4] [P-1] Test: send `tools/list` request, assert >= 60 tools returned [owner:api-engineer]
- [x] [1.5] [P-2] Test: send `tools/call` for `docker_status`, assert result shape (not error) [owner:api-engineer]

## API Batch: Claude Code Configuration

- [x] [2.1] [P-1] Add nv-tools entry to `.claude/mcp.json`: `{ "type": "stdio", "command": ["cargo", "run", "-p", "nv-tools"] }` (or built binary path) [owner:api-engineer]

## Verify

- [x] [3.1] `cargo test -p nv-tools --features integration` -- smoke test passes [owner:api-engineer]
- [x] [3.2] [user] Manual: run `claude` with nv-tools MCP, verify tool list shows in session [owner:api-engineer]
