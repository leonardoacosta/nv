# Implementation Tasks

<!-- beads:epic:TBD -->

## Proto Definitions

- [ ] [1.1] [P-1] Add StartSessionRequest/Response, SendCommandRequest/Response, StopSessionRequest/Response messages to proto/nexus.proto [owner:api-engineer]
- [ ] [1.2] [P-1] Add StartSession, SendCommand, StopSession RPCs to the NexusService definition in proto/nexus.proto [owner:api-engineer]
- [ ] [1.3] [P-1] Regenerate Rust gRPC bindings (cargo build triggers tonic-build) [owner:api-engineer]

## Nexus Client RPCs

- [ ] [2.1] [P-1] Add start_session(project, cwd, command) method to NexusClient — calls StartSession RPC on the agent managing that project [owner:api-engineer]
- [ ] [2.2] [P-1] Add send_command(session_id, text) method to NexusClient — calls SendCommand RPC [owner:api-engineer]
- [ ] [2.3] [P-1] Add stop_session(session_id) method to NexusClient — calls StopSession RPC [owner:api-engineer]

## Project-Scoped Queries

- [ ] [3.1] [P-2] Add format_project_ready(project_code) to nexus/tools.rs — runs bd ready via scoped bash, formats output for Telegram [owner:api-engineer]
- [ ] [3.2] [P-2] Add format_project_proposals(project_code) to nexus/tools.rs — lists openspec/changes/ dirs via scoped bash, formats as proposal list [owner:api-engineer]

## Tool Definitions

- [ ] [4.1] [P-1] Register nexus_project_ready tool in tools.rs — input: project_code [owner:api-engineer]
- [ ] [4.2] [P-1] Register nexus_project_proposals tool in tools.rs — input: project_code [owner:api-engineer]
- [ ] [4.3] [P-1] Register start_session tool in tools.rs — input: project, command; requires confirmation [owner:api-engineer]
- [ ] [4.4] [P-1] Register send_command tool in tools.rs — input: session_id, text [owner:api-engineer]
- [ ] [4.5] [P-1] Register stop_session tool in tools.rs — input: session_id; requires confirmation [owner:api-engineer]

## Tool Execution

- [ ] [5.1] [P-2] Handle nexus_project_ready in worker tool execution loop — call format_project_ready [owner:api-engineer]
- [ ] [5.2] [P-2] Handle nexus_project_proposals in worker tool execution loop — call format_project_proposals [owner:api-engineer]
- [ ] [5.3] [P-2] Handle start_session in worker — create PendingAction with confirmation keyboard, return "Awaiting confirmation" [owner:api-engineer]
- [ ] [5.4] [P-2] Handle send_command in worker — call NexusClient.send_command(), return result [owner:api-engineer]
- [ ] [5.5] [P-2] Handle stop_session in worker — create PendingAction with confirmation keyboard, return "Awaiting confirmation" [owner:api-engineer]

## Callback Handlers

- [ ] [6.1] [P-2] Add approve handler for start_session PendingAction in callbacks.rs — calls NexusClient.start_session(), reports session ID [owner:api-engineer]
- [ ] [6.2] [P-2] Add approve handler for stop_session PendingAction in callbacks.rs — calls NexusClient.stop_session(), reports result [owner:api-engineer]

## Verify

- [ ] [7.1] cargo build passes (including proto regeneration) [owner:api-engineer]
- [ ] [7.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [ ] [7.3] cargo test — existing tests pass, new tests for project-scoped query formatters [owner:api-engineer]
