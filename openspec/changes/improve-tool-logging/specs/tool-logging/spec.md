# Capability: Tool Logging

## ADDED Requirements

### Requirement: execute_tool entry and exit tracing
The `execute_tool` function MUST emit a `tracing::info!` at entry with the tool name and
input key names (not values), and at exit with tool name, success/failure status, and
duration in milliseconds.

#### Scenario: Tool call logged at entry
**Given** any tool is invoked via `execute_tool`
**When** execution begins
**Then** a structured log line is emitted: `tool_call_start tool=<name> input_keys=[<keys>]`

#### Scenario: Tool call logged at exit
**Given** any tool completes execution (success or failure)
**When** the result is returned
**Then** a structured log line is emitted: `tool_call_end tool=<name> success=<bool> duration_ms=<ms>`

### Requirement: Silent tool handler tracing
Tool handlers that currently have no tracing MUST emit at least one `tracing::info!` on
success with a summary of the operation performed.

#### Scenario: Stripe tool logs operation
**Given** a Stripe tool call completes successfully
**When** the result is returned
**Then** a `tracing::info!` with the operation type and key identifiers is emitted

#### Scenario: Previously silent tool now traces
**Given** any tool in the set {stripe, doppler, teams, resend, posthog, cloudflare, vercel, calendar, check}
**When** it executes successfully
**Then** at least one `tracing::info!` line appears in journalctl output

### Requirement: PendingAction correlation logging
PendingAction creation and resolution MUST log the `action_id` UUID to enable lifecycle
correlation across worker/agent creation and callback approval/cancel/expiry.

#### Scenario: Action creation logged with ID
**Given** a PendingAction is created in the worker or agent loop
**When** the action is saved to state
**Then** a log line includes `action_id=<uuid>` and `tool=<name>`

#### Scenario: Action approval logged with ID
**Given** a user approves a pending action via Telegram callback
**When** `handle_approve` executes
**Then** a log line includes the same `action_id=<uuid>` for correlation

## MODIFIED Requirements

### Requirement: SQLite audit truncation limit
The `log_tool_usage()` function MUST truncate input_summary and result_summary to 2000
characters instead of the current 500 characters.

#### Scenario: Long tool output preserved
**Given** a tool returns a result longer than 500 characters but shorter than 2000
**When** `log_tool_usage()` records the result
**Then** the full result is stored without truncation

#### Scenario: Very long output still truncated
**Given** a tool returns a result longer than 2000 characters
**When** `log_tool_usage()` records the result
**Then** it is truncated to 2000 characters
