# Implementation Tasks

## Phase 1: Proto and Build Setup

- [ ] [1.1] Copy `~/dev/nexus/proto/nexus.proto` to `proto/nexus.proto` — include service definition with `GetSessions`, `GetSession`, `StreamEvents` RPCs and all message types (Session, SessionEvent, SessionStatus, etc.) [owner:api-engineer]
- [ ] [1.2] Create `crates/nv-daemon/build.rs` — `tonic_build::configure().build_client(true).build_server(false).compile_protos(&["../../proto/nexus.proto"], &["../../proto/"])`. Generates client stubs only [owner:api-engineer]
- [ ] [1.3] Add gRPC dependencies to `crates/nv-daemon/Cargo.toml` — `tonic = "0.12"`, `prost = "0.13"`, `prost-types = "0.13"` in `[dependencies]`. `tonic-build = "0.12"`, `prost-build = "0.13"` in `[build-dependencies]` [owner:api-engineer]
- [ ] [1.4] Add Nexus types to `crates/nv-core/src/types.rs` — `SessionSummary` struct (id, project, status, agent_name, started_at, duration), `NexusEvent` enum variants (SessionCompleted, SessionError, SessionStarted) for `Trigger::NexusEvent`. `NexusAgentConfig` struct (name, host, port) for config deserialization [owner:api-engineer]

## Phase 2: NexusClient Core

- [ ] [2.1] Create `crates/nv-daemon/src/nexus/mod.rs` — module declarations for client, connection, tools, stream, notify submodules [owner:api-engineer]
- [ ] [2.2] Create `crates/nv-daemon/src/nexus/connection.rs` — `NexusAgentConnection` struct with `name: String`, `endpoint: String`, `client: Option<NexusServiceClient<Channel>>`, `status: ConnectionStatus` enum (Connected, Disconnected, Reconnecting), `last_seen: Option<DateTime<Utc>>`, `consecutive_failures: u32`. `connect()` method with 10s timeout via `tonic::transport::Channel::from_shared().connect_timeout()`. `reconnect()` with exponential backoff (1s, 2s, 4s, 8s, max 60s) (depends: 1.2, 1.3) [owner:api-engineer]
- [ ] [2.3] Create `crates/nv-daemon/src/nexus/client.rs` — `NexusClient` struct holding `Vec<NexusAgentConnection>`. `new(configs: &[NexusAgentConfig])` constructor. `connect_all()` uses `tokio::join!` for parallel connection attempts, logs warnings for unreachable agents. `is_connected()` returns true if any agent connected (depends: 2.2) [owner:api-engineer]

## Phase 3: Query Tools

- [ ] [3.1] Create `crates/nv-daemon/src/nexus/tools.rs` — `query_sessions()` method on `NexusClient`. Calls `GetSessions` RPC on all connected agents in parallel, merges into unified `Vec<SessionSummary>` sorted by start time. Failed agents return empty vec with warning logged. Includes agent name in each summary for attribution (depends: 2.3) [owner:api-engineer]
- [ ] [3.2] Implement `query_session(id: &str)` on `NexusClient` — calls `GetSession` RPC. Tries agent matching session ID prefix first, falls back to querying all connected agents. Returns detailed session info (project, status, command history, errors, duration) or error if session not found (depends: 3.1) [owner:api-engineer]

## Phase 4: Event Stream

- [ ] [4.1] Create `crates/nv-daemon/src/nexus/stream.rs` — `spawn_event_stream(agent: &NexusAgentConnection, tx: mpsc::Sender<Trigger>)` async fn. Calls `StreamEvents` RPC, iterates over server-streaming response. Filters for significant events: session completion, session error. Maps to `Trigger::NexusEvent` variants and pushes to mpsc channel (depends: 2.3) [owner:api-engineer]
- [ ] [4.2] Add stream disconnect handling — on tonic transport error or stream end, log warning, mark agent as Disconnected, trigger reconnect via connection manager. On reconnect success, re-spawn stream task for that agent (depends: 4.1, 2.2) [owner:api-engineer]
- [ ] [4.3] Implement event filtering logic — `SessionCompleted` events include session id, project, duration, agent name. `SessionError` events include session id, project, error message, agent name. `SessionStarted` events logged at debug level but not pushed to trigger channel (low signal in v1) (depends: 4.1) [owner:api-engineer]

## Phase 5: Telegram Notifications

- [ ] [5.1] Create `crates/nv-daemon/src/nexus/notify.rs` — `format_session_completed(event: &SessionCompleted)` returns Telegram message text with session summary (project, duration, agent). No action buttons (informational) (depends: 4.3) [owner:api-engineer]
- [ ] [5.2] Implement session error notification — `format_session_error(event: &SessionError)` returns Telegram message with error details and inline keyboard: "View Error" (callback: `nexus_err:view:{session_id}`), "Create Bug" (callback: `nexus_err:bug:{session_id}`). "View Error" sends full error as follow-up message. "Create Bug" triggers Jira creation flow with error context pre-filled in description (depends: 5.1) [owner:api-engineer]
- [ ] [5.3] Wire Nexus event callbacks in Telegram handler — match `callback_data` prefix `"nexus_err:"` and route to appropriate handler. `view` sends error details as reply, `bug` pushes a Jira create command Trigger with pre-filled context (depends: 5.2) [owner:api-engineer]

## Phase 6: Agent Loop Integration

- [ ] [6.1] Handle `Trigger::NexusEvent` in `crates/nv-daemon/src/agent_loop.rs` — match SessionCompleted → format and send notification to Telegram. Match SessionError → format alert with action buttons and send. No Claude call needed for notifications (direct formatting) (depends: 5.2) [owner:api-engineer]
- [ ] [6.2] Wire NexusClient into `crates/nv-daemon/src/main.rs` — create client from `config.nexus.agents`, call `connect_all()`, spawn event stream tasks for each connected agent, pass client reference to agent loop for tool access (depends: 4.1, 6.1) [owner:api-engineer]
- [ ] [6.3] Register Nexus tools in Claude tool definitions — add `query_sessions` and `query_session` to the tool list in the agent loop's Claude API request, so Claude can invoke them during message processing (depends: 3.1, 3.2) [owner:api-engineer]

## Phase 7: Backfill Digest and Query Stubs

- [ ] [7.1] Replace Nexus stub in `crates/nv-daemon/src/digest/gather.rs` — swap placeholder (spec-7 task 3.4) with `nexus_client.query_sessions()` call. Sessions section in digest now shows real session data with agent names and status (depends: 3.1) [owner:api-engineer]
- [ ] [7.2] Replace Nexus stub in `crates/nv-daemon/src/query/gather.rs` — swap placeholder (spec-8 task 2.5) with `nexus_client.query_sessions()` filtered to relevant project extracted from query context (depends: 3.1) [owner:api-engineer]

---

## Validation Gates

| Phase | Gate |
|-------|------|
| 1 Proto | `cargo build -p nv-daemon` — tonic-build compiles proto, generated types available |
| 2 Client | `cargo build -p nv-daemon` — NexusClient compiles with connection management |
| 3 Tools | `cargo test -p nv-daemon` — unit tests for session merging, error handling on partial connectivity |
| 4 Stream | `cargo build -p nv-daemon` — stream task compiles with event filtering and reconnect |
| 5 Notify | `cargo test -p nv-daemon` — unit tests for notification formatting, callback routing |
| 6 Integration | `cargo build -p nv-daemon` — full Nexus wired into agent loop and main |
| 7 Backfill | Manual: digest includes real Nexus session data. Query "what sessions are running?" returns live data |
