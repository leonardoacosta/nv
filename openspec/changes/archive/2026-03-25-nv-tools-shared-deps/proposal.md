# Proposal: SharedDeps Wiring for Daemon-Coupled Tools

## Change ID
`nv-tools-shared-deps`

## Summary

Define a `SharedDeps` trait in nv-tools that abstracts daemon-internal state (memory, nexus,
schedules, reminders, channels, aggregation, bash, jira). Implement in nv-daemon. Wire MCP
server to dispatch all 28 daemon-coupled tools via this trait.

## Context
- Depends on: `nv-tools-extract-wave-c` (all stateless tools already in nv-tools)
- 28 tools remain in nv-daemon because they reference `crate::memory`, `crate::nexus`,
  `crate::channels`, `crate::reminders`, etc.
- Moving these files would create circular dependencies -- instead, expose them via a trait

## Why

The trait boundary lets the MCP server call daemon-coupled tools without owning daemon state.
When Phase 4 deprecates the daemon, these implementations move to standalone services and the
trait impls change -- but the MCP interface stays stable.

## What Changes

### New in nv-tools
- `src/shared.rs` -- `SharedDeps` trait with async methods for each daemon-coupled tool
- Registry wiring for all 28 tools via `Box<dyn SharedDeps>`

### Modified in nv-daemon
- Implement `SharedDeps` trait on a concrete struct holding `Arc` references
- Wire the MCP server's SharedDeps when daemon starts nv-tools in-process

## Scope
- **IN**: SharedDeps trait, daemon implementation, MCP dispatch for all 28 coupled tools
- **OUT**: Moving daemon-coupled files to nv-tools (stays in daemon)

## Impact
| Area | Change |
|------|--------|
| `crates/nv-tools/src/shared.rs` | New trait definition |
| `crates/nv-tools/src/registry.rs` | Dispatch 28 tools via SharedDeps |
| `crates/nv-daemon/src/tools/mod.rs` | Implement SharedDeps trait |
| `crates/nv-daemon/src/main.rs` | Wire SharedDeps when starting MCP |
