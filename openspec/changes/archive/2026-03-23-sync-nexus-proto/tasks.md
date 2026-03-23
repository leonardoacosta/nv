# Implementation Tasks

<!-- beads:epic:nv-dx9 -->

## DB Batch

(no database tasks)

## API Batch

- [x] [2.1] [P-1] Copy upstream nexus.proto and regenerate Rust types [owner:api-engineer] [beads:nv-9u4]
- [x] [2.2] [P-1] Update stream.rs EventFilter with event_types and initial_snapshot [owner:api-engineer] [beads:nv-6rc]
- [x] [2.3] [P-1] Handle is_snapshot flag in map_event_to_trigger [owner:api-engineer] [beads:nv-up3]
- [x] [2.4] [P-2] Add optional agent param to start_session tool and client [owner:api-engineer] [beads:nv-4v6]
- [x] [2.5] [P-2] Add get_health() to NexusClient calling GetHealth RPC [owner:api-engineer] [beads:nv-xlf]
- [x] [2.6] [P-2] Add list_projects() to NexusClient calling ListProjects RPC [owner:api-engineer] [beads:nv-e3u]
- [x] [2.7] [P-1] Add tool definitions for query_nexus_health, query_nexus_projects, query_nexus_agents [owner:api-engineer] [beads:nv-kqp]
- [x] [2.8] [P-1] Add tool dispatch cases for 3 new Nexus tools [owner:api-engineer] [beads:nv-uc3]
- [x] [2.9] [P-2] Add format functions for health, projects, agents tool output [owner:api-engineer] [beads:nv-zsa]
- [x] [2.10] [P-3] Update stream.rs tests for new EventFilter fields and is_snapshot [owner:api-engineer] [beads:nv-guy]

## UI Batch

(no UI tasks)

## E2E Batch

(no E2E tasks — Nexus requires live agents for integration testing)
