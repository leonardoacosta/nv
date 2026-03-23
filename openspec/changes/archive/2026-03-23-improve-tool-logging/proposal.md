# Proposal: Improve Tool Logging

## Change ID
`improve-tool-logging`

## Summary
Add structured `tracing` at the `execute_tool` entry/exit points, add per-handler traces to
silent tools, increase SQLite audit truncation limits, and add correlation IDs to the
PendingAction lifecycle.

## Context
- Extends: `crates/nv-daemon/src/tools/mod.rs`, `crates/nv-daemon/src/messages.rs`, `crates/nv-daemon/src/worker.rs`, `crates/nv-daemon/src/agent.rs`, `crates/nv-daemon/src/callbacks.rs`
- Related: SQLite `tool_usage` table in messages.rs, per-tool handlers in `tools/` directory

## Motivation
Tool execution is audited via SQLite (`log_tool_usage`) but not visible in `journalctl` output
because there is no `tracing::info!` at the `execute_tool` entry point. Many tool handlers
(stripe, doppler, teams, resend, posthog, cloudflare, vercel, calendar, check) have zero tracing,
making failures completely silent. The SQLite audit truncates input/output to 500 chars, losing
context on complex payloads. PendingAction creation and approval have no shared correlation ID,
making it impossible to trace the full confirmation lifecycle.

## Requirements

### Req-1: execute_tool entry/exit tracing
Add `tracing::info!` at execute_tool entry with tool_name and input key names (not values, to
avoid leaking secrets), and at exit with tool_name, success/failure, and duration_ms.

### Req-2: Silent tool handler tracing
Add `tracing::info!` or `tracing::warn!` to tool handlers that currently have no tracing:
stripe, doppler, teams, resend, posthog, cloudflare, vercel, calendar, check.

### Req-3: PendingAction correlation logging
Log the `action_id` UUID at PendingAction creation (worker.rs, agent.rs) and at approval/
cancel/expiry (callbacks.rs) to enable lifecycle tracing.

### Req-4: SQLite truncation increase
Increase the input_summary and result_summary truncation limit from 500 to 2000 chars in
`log_tool_usage()`.

## Scope
- **IN**: tracing additions to execute_tool, per-handler traces, SQLite truncation limit, PendingAction correlation
- **OUT**: New logging infrastructure, log aggregation, external log shipping, changes to tool behavior, new SQLite columns

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/tools/mod.rs` | Add tracing at execute_tool entry/exit |
| `crates/nv-daemon/src/tools/{stripe,doppler,teams,resend,posthog,cloudflare,vercel,calendar,check}.rs` | Add per-handler tracing |
| `crates/nv-daemon/src/worker.rs` | Add action_id to PendingAction creation log |
| `crates/nv-daemon/src/agent.rs` | Add action_id to PendingAction creation log |
| `crates/nv-daemon/src/callbacks.rs` | Add action_id to approval/cancel/expiry logs |
| `crates/nv-daemon/src/messages.rs` | Change truncation limit from 500 to 2000 |

## Risks
| Risk | Mitigation |
|------|-----------|
| Logging secrets in tool input | Log input KEY names only, never values |
| Increased SQLite storage from longer summaries | 2000 chars is still bounded; old data compacts over time |
| Log volume increase | Only info-level; production already runs at `RUST_LOG=info` |
