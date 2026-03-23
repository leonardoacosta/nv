# Proposal: Nexus Session Watchdog

## Change ID
`add-nexus-session-watchdog`

## Summary

Add a background watchdog task that proactively monitors Nexus agent health and triggers
reconnection. Currently, disconnections are only detected when an RPC call fails (reactive).
The watchdog makes detection proactive by periodically calling `GetHealth()` on each agent,
detecting stale connections, recovering dead event streams, and quarantining persistently
unreachable agents.

## Context
- Extends: `crates/nv-daemon/src/nexus/connection.rs` (reconnect/backoff), `crates/nv-daemon/src/nexus/client.rs` (NexusClient, RPC methods), `crates/nv-daemon/src/nexus/stream.rs` (event stream lifecycle), `crates/nv-daemon/src/main.rs` (initialization)
- Related: The `GetHealth` RPC already exists in `nexus.proto` (line 263) and is generated in `proto::nexus_agent_client`. The `NexusAgentConnection` already tracks `last_seen`, `consecutive_failures`, and `status`. Event streams in `stream.rs` have internal reconnect logic but are spawned fire-and-forget with no external liveness tracking.
- Depends on: nothing — standalone enhancement

## Motivation

**Reactive-only failure detection:** When a Nexus agent goes down silently (e.g., TCP half-open,
network partition, remote process crash without RST), the daemon only discovers this on the next
user-triggered RPC. This means the orchestrator may attempt to dispatch work to a dead agent,
causing user-visible delays and confusing error messages.

**Silent TCP drops:** gRPC channels can appear healthy even when the underlying TCP connection has
been silently dropped. The `last_seen` field is only updated on successful RPCs, so a connected
agent that receives no queries can appear alive indefinitely.

**Event stream fragility:** Event streams are spawned once at startup via `spawn_event_streams()`.
While `run_event_stream()` has internal reconnect logic, there is no external monitoring to detect
if the entire task panicked or got stuck. After a successful reconnect at the connection level, the
event stream may be dead, causing missed session events (completions, errors).

**Operator visibility:** When an agent goes down and comes back, the operator currently has no
notification. For a daemon designed to run unattended, state transitions should be surfaced via
Telegram so the operator can investigate if needed.

## Requirements

### Req-1: Background Watchdog Task

Spawn a tokio task in `main.rs` that runs on a configurable interval (default 10 seconds). For
each configured agent:

1. If `status == Connected`: call `GetHealth()` RPC with a 5-second timeout
   - Success: update `last_seen` timestamp
   - Timeout/error: call `mark_disconnected()`, trigger reconnect via `reconnect()`
2. If `status == Disconnected`: attempt `reconnect()` (reuses existing exponential backoff)
3. If `status == Reconnecting`: skip (already being handled)

The watchdog interval is configurable via `[nexus] watchdog_interval_secs` in `nv.toml`, default 10.
Add `watchdog_interval_secs` to `NexusConfig` with `#[serde(default = "default_watchdog_interval")]`
and a default function returning `10`.

### Req-2: Stale Connection Detection

If `last_seen` is older than 30 seconds (3x watchdog interval) AND `status == Connected`, force
a health check regardless of the normal cycle. This catches silent TCP connection drops where the
gRPC channel looks alive but the remote is unreachable.

The stale threshold is derived as `3 * watchdog_interval_secs` — not a separate config field.

### Req-3: Event Stream Recovery

After a successful reconnect, check if the event stream task for that agent is still alive.
If not, respawn it.

- Store the `JoinHandle<()>` returned by `tokio::spawn` alongside each agent connection
- On reconnect success, check `handle.is_finished()` — if true, respawn via a new `tokio::spawn`
  of `run_event_stream()`
- The `trigger_tx` clone must be passed through to enable respawn
- Refactor `spawn_event_streams()` to return `Vec<JoinHandle<()>>` and store them in a struct
  alongside the agent `Arc<Mutex<NexusAgentConnection>>` references

### Req-4: Health State Updates

On each watchdog cycle, update the daemon's `HealthState` with per-agent connection status:

- `Connected` agent with successful health check: `ChannelStatus::Connected`
- `Disconnected` or failed health check: `ChannelStatus::Disconnected`

This feeds into the `/health` endpoint and `nv status` via the existing `health_state.update_channel(format!("nexus_{}", name), status)` pattern already used in `main.rs`.

### Req-5: Quarantine After Repeated Failures

If an agent's `consecutive_failures` reaches 10, quarantine it: the watchdog skips that agent
for 5 minutes (300 seconds) instead of checking every 10 seconds.

- Add `quarantined_until: Option<Instant>` to `NexusAgentConnection`
- When `consecutive_failures >= 10` after a failed reconnect, set `quarantined_until = Some(Instant::now() + Duration::from_secs(300))`
- Watchdog checks `quarantined_until` before processing: if `Some(t)` and `Instant::now() < t`, skip
- On first successful reconnect, clear quarantine: `quarantined_until = None`, reset `consecutive_failures = 0` (already done in `connect()`)

### Req-6: Telegram Notification on State Change

When an agent transitions `Connected -> Disconnected` or `Disconnected -> Connected`, send a
Telegram notification via the existing channel registry:

- Disconnect: `"Nexus agent '{name}' disconnected"`
- Reconnect: `"Nexus agent '{name}' reconnected (was down {duration})"`

**Debounce:** Only notify on disconnect if the agent has been disconnected for >30 seconds. Track
`disconnected_since: Option<Instant>` on the connection. On the first watchdog cycle that sees
`Disconnected` with no `disconnected_since`, set it. On subsequent cycles, if
`Instant::now() - disconnected_since > 30s` and no notification sent yet, send it.

On reconnect, always notify (with downtime duration computed from `disconnected_since`), then
clear `disconnected_since`.

## Scope
- **IN**: Background watchdog tokio task, `GetHealth` RPC ping with timeout, stale connection detection, event stream JoinHandle tracking and respawn, per-agent health state updates, quarantine logic on `NexusAgentConnection`, Telegram state-change notifications with debounce, `watchdog_interval_secs` config field on `NexusConfig`, `disconnected_since` and `quarantined_until` fields on `NexusAgentConnection`
- **OUT**: Watchdog for non-Nexus channels (Telegram, Discord, etc.), automatic session migration between agents on failover, changes to the `GetHealth` RPC definition or Nexus agent-side code, UI dashboard for agent health history, persistent health history logging

## Impact
| Area | Change |
|------|--------|
| `crates/nv-core/src/config.rs` | Add `watchdog_interval_secs: u64` to `NexusConfig` with default 10 |
| `crates/nv-daemon/src/nexus/connection.rs` | Add `quarantined_until: Option<Instant>`, `disconnected_since: Option<Instant>` fields; add `is_quarantined()` and `quarantine()` methods |
| `crates/nv-daemon/src/nexus/client.rs` | Add `health_check()` method that calls `GetHealth` RPC with timeout; add `run_watchdog()` standalone async fn |
| `crates/nv-daemon/src/nexus/stream.rs` | Refactor `spawn_event_streams()` to return `Vec<JoinHandle<()>>`; export `run_event_stream` as `pub` for respawn |
| `crates/nv-daemon/src/nexus/mod.rs` | Export new watchdog module or re-export `run_watchdog` |
| `crates/nv-daemon/src/main.rs` | Spawn watchdog task after Nexus connect_all, pass `health_state`, `trigger_tx`, channel registry |
| `crates/nv-daemon/src/health.rs` | No structural changes — uses existing `update_channel()` API |

## Risks
| Risk | Mitigation |
|------|-----------|
| Watchdog lock contention with RPC callers | Each `GetHealth` call acquires the agent mutex briefly (~5s max with timeout). The watchdog runs every 10s and processes agents sequentially, so contention is bounded. If needed, can switch to `try_lock` with skip. |
| Quarantine hides a recoverable agent | 5-minute quarantine window is a compromise. The agent is still checked every 5 minutes, and any successful reconnect immediately clears quarantine. The operator also gets the Telegram disconnect notification. |
| Event stream respawn races with internal reconnect logic | `run_event_stream` already loops and retries internally. The watchdog only respawns if `is_finished()` is true (task exited entirely), not if it's still running its internal retry loop. |
| Telegram notification spam during network flaps | 30-second debounce on disconnect notifications prevents spam from transient blips. Reconnect notifications always fire but only after a real disconnect was notified. |
| `GetHealth` RPC not implemented on older agents | If the agent returns `Unimplemented`, treat it as healthy (the connection itself is alive). Log at debug level and skip health assessment for that agent. |
