# Proposal: Worker DAG Events

## Change ID
`add-worker-dag-events`

## Summary

Workers emit structured progress events via `tokio::sync::mpsc` channel to the orchestrator.
Events cover stage lifecycle (started/complete), tool invocations, completion, and errors.
Orchestrator surfaces milestones to Telegram and implements long-task confirmation for tasks
estimated >1 minute.

## Context
- Extends: `crates/nv-daemon/src/worker.rs` (WorkerPool, worker processing loop)
- Extends: `crates/nv-daemon/src/orchestrator.rs` (trigger processing, Telegram dispatch)
- Related: PRD §5.2 (Worker DAG Events)

## Motivation

Workers currently operate as black boxes. The orchestrator dispatches a task and waits for the
final result — no intermediate visibility. This causes:

1. **User confusion** — long-running queries (Jira search across projects, multi-tool synthesis)
   show no progress. User wonders if Nova froze.
2. **No debugging telemetry** — when a worker fails mid-loop, there's no event trail showing
   which stage or tool call was executing.
3. **No long-task UX** — tasks that take >1 minute should confirm with the user before proceeding
   ("This will take ~2min. Searching Jira across all projects. Be right back.").

## Requirements

### Req-1: WorkerEvent Enum

Define in `worker.rs`:

```rust
#[derive(Debug, Clone)]
pub enum WorkerEvent {
    StageStarted { worker_id: Uuid, stage: String },
    ToolCalled { worker_id: Uuid, tool: String },
    StageComplete { worker_id: Uuid, stage: String, duration_ms: u64 },
    Complete { worker_id: Uuid, response_len: usize },
    Error { worker_id: Uuid, error: String },
}
```

### Req-2: Event Channel Wiring

Add `mpsc::UnboundedSender<WorkerEvent>` to `SharedDeps`. Workers send events at each stage
boundary: context build (StageStarted), each tool call (ToolCalled), tool loop complete
(StageComplete), final response (Complete), and on any error (Error).

### Req-3: Orchestrator Event Handling

Orchestrator receives `WorkerEvent` on a second `mpsc::UnboundedReceiver`. Processing rules:
- `StageStarted` / `StageComplete` — log at debug level, no Telegram message
- `ToolCalled` — log at trace level
- `Complete` — no action (result already routed via existing path)
- `Error` — log at warn level
- If no `Complete` or `Error` received within 30s of `StageStarted`, send brief status update
  to Telegram: "Still working on it... (running {stage})"

### Req-4: Long-Task Confirmation

When the orchestrator estimates a task will take >1 minute (based on trigger classification —
e.g., multi-project Jira search, digest synthesis), send a confirmation message before dispatching:
"This will take ~2min. {description}. Be right back." This is a send-and-proceed pattern, not
a blocking confirmation.

## Scope
- **IN**: WorkerEvent enum, mpsc channel, orchestrator event loop, long-task status messages
- **OUT**: Persistent event storage (use tool_audit_log for that), DAG visualization, event replay

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/worker.rs` | Add WorkerEvent enum, add event_tx to SharedDeps, emit events at stage boundaries in worker processing loop |
| `crates/nv-daemon/src/orchestrator.rs` | Add event_rx receiver, select! on both trigger_rx and event_rx, handle events per rules, add long-task confirmation logic |
| `crates/nv-daemon/src/main.rs` | Create mpsc channel pair, pass sender to SharedDeps, pass receiver to Orchestrator |

## Risks
| Risk | Mitigation |
|------|-----------|
| Unbounded channel memory growth if orchestrator stalls | Use unbounded channel (events are small); monitor in health check |
| Status messages spam Telegram on rapid tool loops | Only surface status after 30s silence; ToolCalled events are trace-only |
| Long-task estimation inaccurate | Start with simple heuristic (trigger type); refine with tool_usage duration data later |
