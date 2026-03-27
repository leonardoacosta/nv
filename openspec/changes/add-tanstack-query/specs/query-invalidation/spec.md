# Spec: Query Invalidation Strategy

## ADDED Requirements

### Requirement: Mutation-Based Invalidation
When a mutation succeeds, it MUST invalidate all query keys for data affected by the write operation. The invalidation map SHALL be documented in `query-keys.ts`.

#### Scenario: Obligation CRUD invalidation
Given the user creates, updates, or deletes an obligation
When the mutation succeeds
Then queries keyed to `["api", "/api/obligations"]` are invalidated
And queries keyed to `["api", "/api/activity-feed"]` are invalidated (feed reflects obligation changes)

#### Scenario: Automation toggle invalidation
Given the user enables or disables an automation (watcher, schedule, reminder)
When the mutation succeeds
Then queries keyed to `["api", "/api/automations"]` are invalidated

### Requirement: WebSocket-Triggered Invalidation
When a WebSocket event arrives via `DaemonEventContext`, it MUST trigger targeted query invalidation for the relevant data type instead of (or in addition to) prepending to a local state array.

#### Scenario: Real-time activity feed update
Given the dashboard receives a WebSocket event of type "message"
When the event handler fires
Then `["api", "/api/messages"]` and `["api", "/api/activity-feed"]` queries are invalidated
And the queries refetch in the background

#### Scenario: Session status change
Given the dashboard receives a WebSocket event of type "session"
When the event handler fires
Then `["api", "/api/sessions"]` queries are invalidated

### Requirement: Query Key Convention
All query keys MUST follow the `["api", path, params?]` convention where `path` matches the API route and `params` is an optional object of query string parameters. This convention SHALL be documented to enable straightforward migration to tRPC's `queryKey()` pattern.

#### Scenario: Key uniqueness with params
Given two queries to the same endpoint with different params
When both are active (e.g., sessions page 1 and page 2)
Then each has a distinct cache entry
And invalidating `["api", "/api/sessions"]` clears both entries

#### Scenario: Partial key invalidation
Given the user performs an action that affects multiple endpoints
When `queryClient.invalidateQueries({ queryKey: ["api"] })` is called
Then all API-backed queries are invalidated at once
