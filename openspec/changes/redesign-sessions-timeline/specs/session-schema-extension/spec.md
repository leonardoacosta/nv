# session-schema-extension

## ADDED Requirements

### Requirement: Session trigger and aggregate columns

The system SHALL add `trigger_type` (text, nullable), `message_count` (integer, default 0), and `tool_count` (integer, default 0) columns to the `sessions` table. These MUST provide filterable metadata and summary counts displayed in the timeline list without requiring a join to `session_events`.

#### Scenario: New session with trigger metadata

Given a session is created with trigger_type "watcher",
when the session row is inserted,
then `trigger_type` is "watcher", `message_count` is 0, and `tool_count` is 0.

#### Scenario: Session without trigger metadata

Given a session is created without specifying trigger_type,
when the session row is inserted,
then `trigger_type` is null, `message_count` is 0, and `tool_count` is 0.

### Requirement: Session events table

The system SHALL create a `session_events` table with columns: `id` (uuid PK), `session_id` (uuid FK to sessions.id, not null), `event_type` (text, not null -- one of "message", "tool_call", "api_request"), `direction` (text, nullable -- "inbound"/"outbound" for messages), `content` (text, nullable), `metadata` (jsonb, nullable -- tool name/inputs/outputs for tool calls, method/endpoint/status for API requests), `created_at` (timestamp with timezone, not null, default now). The table MUST have an index on `(session_id, created_at)` for timeline queries.

#### Scenario: Message event stored

Given an assistant message is sent during a session,
when a session_event is inserted with event_type "message" and direction "outbound",
then the event is queryable by session_id ordered by created_at.

#### Scenario: Tool call event stored

Given a tool call is executed during a session,
when a session_event is inserted with event_type "tool_call" and metadata containing tool name, inputs, and outputs,
then the event metadata is retrievable as structured JSON.

#### Scenario: API request event stored

Given an external API request is made during a session,
when a session_event is inserted with event_type "api_request" and metadata containing method, endpoint, and status_code,
then the event is included in the session's timeline query.

## MODIFIED Requirements

### Requirement: Sessions table export

The `@nova/db` package index MUST export the new `sessionEvents` table, `SessionEvent` type, and `NewSessionEvent` type alongside the existing sessions exports.

#### Scenario: Session events importable from package

Given a consumer imports from `@nova/db`,
when they reference `sessionEvents`, `SessionEvent`, or `NewSessionEvent`,
then the imports resolve without errors and the types match the session_events table schema.
