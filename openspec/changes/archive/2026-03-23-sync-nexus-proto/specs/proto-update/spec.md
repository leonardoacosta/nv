# Proto Update

## MODIFIED Requirements

### Requirement: EventFilter MUST support event type filtering and initial snapshot

The `EventFilter` message SHALL gain two fields enabling server-side noise reduction
and bootstrap state delivery.

#### Scenario: Subscribe with event type filter
Given Nova subscribes to StreamEvents with `event_types: [STATUS_CHANGED, SESSION_STOPPED]`
When a heartbeat event occurs on the Nexus agent
Then the event is NOT delivered over the stream (filtered server-side)

#### Scenario: Subscribe with initial snapshot
Given Nova subscribes with `initial_snapshot: true`
When the stream connects
Then Nexus sends `SessionStarted` events for all currently active sessions with `is_snapshot: true`
And Nova can distinguish snapshots from real session starts via the `is_snapshot` flag

### Requirement: SessionEvent MUST include agent_name

The `SessionEvent` message SHALL include `agent_name` (field 7), set by the Nexus agent.
Nova currently tracks agent_name via the connection — this field provides it in the event payload.

#### Scenario: Event carries agent name
Given a SessionStopped event is received from the "homelab" agent
Then `event.agent_name` equals "homelab"
And `map_event_to_trigger` can use it instead of the connection-level name

## ADDED Requirements

### Requirement: EventType enum MUST exist in proto

The proto SHALL define `EventType` with 5 values: UNSPECIFIED, SESSION_STARTED,
HEARTBEAT_RECEIVED, STATUS_CHANGED, SESSION_STOPPED.

#### Scenario: EventType values match upstream
Given the proto is synced from upstream
Then the EventType enum has exactly 5 variants with values 0-4
