# Proposal: Extract HTTP Tools Wave A

## Change ID
`nv-tools-extract-wave-a`

## Summary

Move 5 stateless HTTP tool files (stripe, vercel, sentry, resend, upstash) from nv-daemon
to nv-tools. Each file uses only `reqwest` + `ToolDefinition` with zero daemon-specific imports.

## Context
- Depends on: `nv-tools-scaffold` (ToolDefinition must be in nv-core)
- These 5 tools follow identical patterns: client struct with `from_env()`, API calls via reqwest
- No SharedDeps, no daemon state, no `crate::` imports beyond ToolDefinition

## Why

Stateless HTTP tools are the easiest extraction target. Moving them proves the pattern works
and establishes the re-export bridge that keeps daemon tests passing.

## What Changes

Per tool (stripe, vercel, sentry, resend, upstash):
1. Move `.rs` file from `crates/nv-daemon/src/tools/` to `crates/nv-tools/src/tools/`
2. Update import: `nv_core::ToolDefinition`
3. Add `pub use nv_tools::tools::{module}::*` re-export in daemon's `tools/mod.rs`
4. Register tool definitions in nv-tools registry

## Scope
- **IN**: 5 tool files, re-exports, registry registration
- **OUT**: Daemon-coupled tools, process-shelling tools

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/tools/stripe.rs` | Moved to nv-tools |
| `crates/nv-daemon/src/tools/vercel.rs` | Moved to nv-tools |
| `crates/nv-daemon/src/tools/sentry.rs` | Moved to nv-tools |
| `crates/nv-daemon/src/tools/resend.rs` | Moved to nv-tools |
| `crates/nv-daemon/src/tools/upstash.rs` | Moved to nv-tools |
| `crates/nv-daemon/src/tools/mod.rs` | Add 5 re-exports |
| `crates/nv-tools/Cargo.toml` | Add `reqwest` dependency |
| `crates/nv-tools/src/tools/mod.rs` | New module with 5 tool re-exports |
