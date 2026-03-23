# Implementation Tasks

<!-- beads:epic:TBD -->

## Code Batch

- [ ] [1.1] [P-1] Add `ha_service_call` match arms in both `execute_tool` functions in mod.rs returning PendingAction [owner:api-engineer]
- [ ] [1.2] [P-1] Add `ActionType::HomeAssistant` variant to types.rs and detection in `detect_action_type()` in callbacks.rs [owner:api-engineer]
- [ ] [1.3] [P-1] Route approval callback to `ha_service_call_execute()` in callbacks.rs [owner:api-engineer]
- [ ] [1.4] [P-1] Remove `#[allow(dead_code)]` from `ha_service_call_execute` in ha.rs [owner:api-engineer]

## E2E Batch

- [ ] [2.1] Add test: ha_service_call returns PendingAction with correct description [owner:test-writer]
