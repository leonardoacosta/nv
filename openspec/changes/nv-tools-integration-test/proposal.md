# Proposal: MCP Integration Test

## Change ID
`nv-tools-integration-test`

## Summary

End-to-end verification that nv-tools works as a Claude Code MCP server. Smoke test binary,
`.claude/mcp.json` configuration, and manual verification of 5 representative tools.

## Context
- Depends on: `nv-tools-shared-deps` (all tools must be registered)
- Final spec in the v6 pipeline
- Success gate: Claude Code can connect and execute tools via MCP

## What Changes

### New test
- `crates/nv-tools/tests/smoke.rs` -- integration test behind `#[cfg(feature = "integration")]`
- Spawns nv-tools subprocess, sends JSON-RPC via stdin, validates responses

### New config
- `.claude/mcp.json` entry for nv-tools (stdio transport)

## Scope
- **IN**: Smoke test, mcp.json config, manual e2e verification
- **OUT**: Performance testing, full tool coverage testing

## Impact
| Area | Change |
|------|--------|
| `crates/nv-tools/tests/smoke.rs` | New integration test |
| `crates/nv-tools/Cargo.toml` | Add `integration` feature flag |
| `.claude/mcp.json` | Add nv-tools server entry |
