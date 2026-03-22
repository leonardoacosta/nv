# Implementation Tasks

<!-- beads:epic:TBD -->

## API Batch

- [ ] [2.1] [P-1] Define `WorkerEvent` enum in worker.rs — StageStarted { worker_id: Uuid, stage: String }, ToolCalled { worker_id, tool }, StageComplete { worker_id, stage, duration_ms: u64 }, Complete { worker_id, response_len: usize }, Error { worker_id, error: String } [owner:api-engineer]
- [ ] [2.2] [P-1] Add `event_tx: mpsc::UnboundedSender<WorkerEvent>` field to SharedDeps struct in worker.rs [owner:api-engineer]
- [ ] [2.3] [P-1] Emit StageStarted("context_build") at start of worker processing, StageComplete("context_build") after system context is built [owner:api-engineer]
- [ ] [2.4] [P-1] Emit ToolCalled event before each execute_tool()/execute_tool_send() call in the tool loop [owner:api-engineer]
- [ ] [2.5] [P-1] Emit StageComplete("tool_loop") after tool loop exits, Complete event after final response extracted [owner:api-engineer]
- [ ] [2.6] [P-1] Emit Error event in all worker error paths (Claude API failure, tool failure, timeout) [owner:api-engineer]
- [ ] [2.7] [P-1] Create `mpsc::unbounded_channel::<WorkerEvent>()` in main.rs, pass sender to SharedDeps, pass receiver to Orchestrator::new() [owner:api-engineer]
- [ ] [2.8] [P-1] Add `event_rx: mpsc::UnboundedReceiver<WorkerEvent>` to Orchestrator struct, accept in constructor [owner:api-engineer]
- [ ] [2.9] [P-1] Add `tokio::select!` branch in orchestrator run loop to receive WorkerEvent — log StageStarted/StageComplete at debug, ToolCalled at trace, Error at warn [owner:api-engineer]
- [ ] [2.10] [P-2] Add 30s inactivity timer in orchestrator — if no Complete/Error received within 30s of last StageStarted, send Telegram status: "Still working on it... (running {stage})" [owner:api-engineer]
- [ ] [2.11] [P-2] Add long-task confirmation — before dispatching tasks classified as multi-project or digest, send "This will take ~{est}. {description}. Be right back." to Telegram [owner:api-engineer]

## Verify

- [ ] [3.1] cargo build passes [owner:api-engineer]
- [ ] [3.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [ ] [3.3] Unit test: WorkerEvent enum serialization/construction for all variants [owner:api-engineer]
- [ ] [3.4] Unit test: worker emits StageStarted → ToolCalled → StageComplete → Complete sequence for normal flow [owner:api-engineer]
- [ ] [3.5] Unit test: worker emits Error event on tool failure [owner:api-engineer]
- [ ] [3.6] Unit test: orchestrator handles all WorkerEvent variants without panic [owner:api-engineer]
- [ ] [3.7] Existing tests pass [owner:api-engineer]
