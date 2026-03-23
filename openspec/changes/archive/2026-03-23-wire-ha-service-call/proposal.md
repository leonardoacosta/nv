# Proposal: Wire HA Service Call

## Change ID
`wire-ha-service-call`

## Summary
Connect the existing `ha_service_call` tool definition and execution function to the
`execute_tool` dispatch, routing it through the PendingAction confirmation flow.

## Context
- Extends: `crates/nv-daemon/src/tools/mod.rs`, `crates/nv-daemon/src/tools/ha.rs`, `crates/nv-daemon/src/callbacks.rs`, `crates/nv-core/src/types.rs`
- Related: PendingAction flow already works for Jira, Nexus, Schedule, and Channel tools

## Motivation
The `ha_service_call` tool definition exists at `ha.rs:162-186` and the execute function exists
at `ha.rs:318-327`, but there is no match arm in `execute_tool` — the tool is dead code.
Claude sees the tool definition and attempts to call it, but execution silently fails.
The full PendingAction lifecycle (inline keyboard → callback_query → approve/edit/cancel →
execute → edit message) is already implemented — HA just needs to be wired in.

## Requirements

### Req-1: Tool dispatch wiring
Add `"ha_service_call"` match arms in both `execute_tool` functions returning
`ToolResult::PendingAction` with the service call description.

### Req-2: Action type detection and approval routing
Add `ActionType::HomeAssistant` variant to `detect_action_type()` in `callbacks.rs` and route
approval to `ha_service_call_execute()`.

### Req-3: Dead code cleanup
Remove `#[allow(dead_code)]` from `ha_service_call_execute()` since it will now be called.

## Scope
- **IN**: execute_tool match arms, ActionType variant, callbacks routing, dead code annotation removal
- **OUT**: New HA tools, HA config changes, new tool definitions, changes to PendingAction flow

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/tools/mod.rs` | Add `"ha_service_call"` match arms in both execute_tool functions |
| `crates/nv-daemon/src/callbacks.rs` | Add `ActionType::HomeAssistant` detection and execution routing |
| `crates/nv-core/src/types.rs` | Add `ActionType::HomeAssistant` enum variant |
| `crates/nv-daemon/src/tools/ha.rs` | Remove `#[allow(dead_code)]` from `ha_service_call_execute` |

## Risks
| Risk | Mitigation |
|------|-----------|
| HA service calls could be destructive (turn off security, open doors) | Confirmation gate ensures user approval before execution |
| `ha_service_call_execute` may need updated parameters | Verify function signature matches PendingAction payload shape |
