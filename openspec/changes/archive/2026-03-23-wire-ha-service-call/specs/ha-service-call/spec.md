# Capability: HA Service Call Execution

## ADDED Requirements

### Requirement: ha_service_call tool dispatch
The `execute_tool` function MUST include a match arm for `"ha_service_call"` that returns
`ToolResult::PendingAction` with domain, service, and data from the input payload.

#### Scenario: Claude calls ha_service_call
**Given** Claude invokes the `ha_service_call` tool with `{"domain": "light", "service": "turn_off", "data": {"entity_id": "light.living_room"}}`
**When** `execute_tool` processes the call
**Then** it returns `ToolResult::PendingAction` with a description like "HA: light.turn_off (light.living_room)"

#### Scenario: Confirmation prompt appears in Telegram
**Given** a `PendingAction` is created for `ha_service_call`
**When** the worker/agent processes the pending action
**Then** a Telegram message with Approve/Edit/Cancel inline keyboard is sent to the user

### Requirement: HomeAssistant action type detection
The `detect_action_type()` function in `callbacks.rs` MUST recognize `ha_service_call` actions
and route approval to `ha_service_call_execute()`.

#### Scenario: User approves HA service call
**Given** a pending `ha_service_call` action exists with status `AwaitingConfirmation`
**When** the user presses the Approve button in Telegram
**Then** `ha_service_call_execute()` is called with the stored domain, service, and data
**And** the original Telegram message is edited to show the execution result

#### Scenario: User cancels HA service call
**Given** a pending `ha_service_call` action exists
**When** the user presses Cancel
**Then** the action is marked `Cancelled` and no HA API call is made

### Requirement: Dead code cleanup
The `ha_service_call_execute()` function MUST NOT carry `#[allow(dead_code)]` since it is now
reachable through the approval callback flow.

#### Scenario: No dead code warnings
**Given** the codebase compiles with `cargo check`
**When** `ha_service_call_execute` is connected to callbacks
**Then** no `dead_code` warning is emitted for this function
