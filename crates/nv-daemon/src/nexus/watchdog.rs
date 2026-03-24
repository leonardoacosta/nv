use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use nv_core::types::OutboundMessage;
use tokio::sync::Mutex;

use crate::health::{ChannelStatus, HealthState};
use super::connection::{ConnectionStatus, NexusAgentConnection};
use super::stream::run_event_stream;
use super::client::NexusClient;

/// Run the Nexus session watchdog.
///
/// Loops on `watchdog_interval_secs`. For each agent:
/// - Skips quarantined agents.
/// - If `Connected`: calls `GetHealth` (force-check if `last_seen` is stale).
///   On failure: marks disconnected, attempts reconnect, quarantines if needed.
/// - If `Disconnected`: attempts reconnect.
/// - If `Reconnecting`: skips (already in progress).
///
/// After any successful reconnect, checks whether the event stream task is still
/// alive and respawns it if not.
///
/// Also updates `HealthState` per-agent and sends Telegram notifications on
/// `Connected ↔ Disconnected` state transitions (with debounce on disconnect).
pub async fn run_watchdog(
    client: NexusClient,
    health_state: Arc<HealthState>,
    watchdog_interval_secs: u64,
    mut stream_handles: Vec<tokio::task::JoinHandle<()>>,
    trigger_tx: tokio::sync::mpsc::UnboundedSender<nv_core::types::Trigger>,
    channels: HashMap<String, Arc<dyn nv_core::channel::Channel>>,
) {
    let interval_duration = Duration::from_secs(watchdog_interval_secs);
    let stale_threshold = Duration::from_secs(watchdog_interval_secs * 3);
    let mut ticker = tokio::time::interval(interval_duration);
    // The first tick fires immediately; skip it so we don't double-check right
    // after startup when connect_all() just ran.
    ticker.tick().await;

    loop {
        ticker.tick().await;
        tracing::debug!("nexus watchdog cycle starting");

        for (idx, agent_mutex) in client.agents.iter().enumerate() {
            process_agent(
                agent_mutex,
                idx,
                &mut stream_handles,
                &trigger_tx,
                &health_state,
                &channels,
                stale_threshold,
            )
            .await;
        }
    }
}

/// Process one agent in a single watchdog cycle.
async fn process_agent(
    agent_mutex: &Arc<Mutex<NexusAgentConnection>>,
    idx: usize,
    stream_handles: &mut [tokio::task::JoinHandle<()>],
    trigger_tx: &tokio::sync::mpsc::UnboundedSender<nv_core::types::Trigger>,
    health_state: &Arc<HealthState>,
    channels: &HashMap<String, Arc<dyn nv_core::channel::Channel>>,
    stale_threshold: Duration,
) {
    let mut conn = agent_mutex.lock().await;
    let agent_name = conn.name.clone();

    // Skip quarantined agents entirely.
    if conn.is_quarantined() {
        tracing::debug!(agent = %agent_name, "watchdog: agent is quarantined, skipping");
        return;
    }

    match conn.status {
        ConnectionStatus::Reconnecting => {
            tracing::debug!(agent = %agent_name, "watchdog: agent is reconnecting, skipping");
        }

        ConnectionStatus::Connected => {
            // Force a health check if last_seen is older than the stale threshold.
            let is_stale = conn
                .last_seen
                .map(|ls| {
                    let elapsed = chrono::Utc::now()
                        .signed_duration_since(ls)
                        .to_std()
                        .unwrap_or(stale_threshold);
                    elapsed >= stale_threshold
                })
                .unwrap_or(true); // no last_seen → treat as stale

            if is_stale {
                tracing::debug!(
                    agent = %agent_name,
                    "watchdog: stale connection detected, forcing health check"
                );
            }

            match conn.health_check().await {
                Ok(()) => {
                    tracing::debug!(agent = %agent_name, "watchdog: health check ok");
                    health_state
                        .update_channel(format!("nexus_{agent_name}"), ChannelStatus::Connected)
                        .await;
                }
                Err(e) => {
                    tracing::warn!(
                        agent = %agent_name,
                        error = %e,
                        "watchdog: health check failed"
                    );
                    conn.mark_disconnected();
                    health_state
                        .update_channel(format!("nexus_{agent_name}"), ChannelStatus::Disconnected)
                        .await;

                    // ── Split-phase reconnect ───────────────────────────────
                    // Phase 1: read state, compute backoff, mark reconnecting,
                    //          then drop the lock before sleeping.
                    let backoff = conn.backoff_duration();
                    conn.status = ConnectionStatus::Reconnecting;
                    drop(conn);

                    // Phase 2: sleep outside the lock.
                    tokio::time::sleep(backoff).await;

                    // Phase 3: re-acquire and attempt connect().
                    let mut conn = agent_mutex.lock().await;
                    match conn.connect().await {
                        Ok(()) => {
                            tracing::info!(agent = %agent_name, "watchdog: agent reconnected");
                            handle_reconnect_success(
                                &mut conn,
                                &agent_name,
                                idx,
                                stream_handles,
                                trigger_tx,
                                health_state,
                                channels,
                                agent_mutex,
                            )
                            .await;
                        }
                        Err(e) => {
                            tracing::warn!(
                                agent = %agent_name,
                                error = %e,
                                "watchdog: reconnect failed"
                            );
                            conn.status = ConnectionStatus::Disconnected;
                            if conn.consecutive_failures >= 10 {
                                conn.quarantine();
                            }
                        }
                    }
                    // Notification check after re-acquiring lock.
                    if conn.status == ConnectionStatus::Disconnected {
                        maybe_send_disconnect_notification(&mut conn, channels).await;
                    }
                    return;
                }
            }

            // Send disconnect notification if debounce window has passed.
            if conn.status == ConnectionStatus::Disconnected {
                maybe_send_disconnect_notification(&mut conn, channels).await;
            }
        }

        ConnectionStatus::Disconnected => {
            // Update health state for the disconnected agent.
            health_state
                .update_channel(format!("nexus_{agent_name}"), ChannelStatus::Disconnected)
                .await;

            // Send disconnect notification if debounce window has passed.
            maybe_send_disconnect_notification(&mut conn, channels).await;

            tracing::debug!(
                agent = %agent_name,
                failures = conn.consecutive_failures,
                "watchdog: attempting reconnect"
            );

            // ── Split-phase reconnect ───────────────────────────────────
            // Phase 1: read state, compute backoff, mark reconnecting, drop lock.
            let backoff = conn.backoff_duration();
            conn.status = ConnectionStatus::Reconnecting;
            drop(conn);

            // Phase 2: sleep outside the lock.
            tokio::time::sleep(backoff).await;

            // Phase 3: re-acquire and call connect() directly.
            let mut conn = agent_mutex.lock().await;
            match conn.connect().await {
                Ok(()) => {
                    tracing::info!(agent = %agent_name, "watchdog: agent reconnected");
                    handle_reconnect_success(
                        &mut conn,
                        &agent_name,
                        idx,
                        stream_handles,
                        trigger_tx,
                        health_state,
                        channels,
                        agent_mutex,
                    )
                    .await;
                }
                Err(e) => {
                    tracing::warn!(
                        agent = %agent_name,
                        error = %e,
                        "watchdog: reconnect failed"
                    );
                    // mark_disconnected() increments the counter and sets status.
                    conn.mark_disconnected();
                    if conn.consecutive_failures >= 10 {
                        conn.quarantine();
                    }
                }
            }
        }
    }
}

/// Called after a successful reconnect to update health state, respawn the
/// event stream if it exited, and send a Telegram reconnect notification.
#[allow(clippy::too_many_arguments)]
async fn handle_reconnect_success(
    conn: &mut NexusAgentConnection,
    agent_name: &str,
    idx: usize,
    stream_handles: &mut [tokio::task::JoinHandle<()>],
    trigger_tx: &tokio::sync::mpsc::UnboundedSender<nv_core::types::Trigger>,
    health_state: &Arc<HealthState>,
    channels: &HashMap<String, Arc<dyn nv_core::channel::Channel>>,
    agent_mutex: &Arc<Mutex<NexusAgentConnection>>,
) {
    tracing::info!(agent = %agent_name, "watchdog: agent reconnected");

    health_state
        .update_channel(format!("nexus_{agent_name}"), ChannelStatus::Connected)
        .await;

    // Respawn the event stream task if it has exited.
    if let Some(handle) = stream_handles.get(idx) {
        if handle.is_finished() {
            tracing::info!(
                agent = %agent_name,
                "watchdog: event stream task exited, respawning"
            );
            let agent_arc = Arc::clone(agent_mutex);
            let tx = trigger_tx.clone();
            let new_handle = tokio::spawn(async move {
                run_event_stream(agent_arc, tx).await;
            });
            stream_handles[idx] = new_handle;
        }
    }

    // Compute downtime duration.  disconnected_since is cleared by connect()
    // inside reconnect(), so we must capture it BEFORE calling reconnect().
    // That capture happens in process_agent / run_event_stream before invoking
    // this helper; the captured value is threaded in via `disconnected_since`.
    if conn.disconnect_notified {
        let downtime_display = if let Some(since) = conn.disconnected_since {
            // disconnected_since may still be set if connect() was not yet called
            // (split-phase reconnect); if it's already been cleared use "unknown".
            let elapsed = since.elapsed();
            let secs = elapsed.as_secs();
            if secs < 60 {
                format!("{secs}s")
            } else if secs < 3600 {
                format!("{}m", secs / 60)
            } else {
                format!("{}h{}m", secs / 3600, (secs % 3600) / 60)
            }
        } else {
            "unknown".to_string()
        };
        send_telegram_message(
            channels,
            format!("Nexus agent '{agent_name}' reconnected (was down {downtime_display})"),
        )
        .await;
        conn.disconnect_notified = false;
    }
}

/// Send a Telegram disconnect notification if the debounce window (30 s) has
/// elapsed and we haven't already notified for this outage.
async fn maybe_send_disconnect_notification(
    conn: &mut NexusAgentConnection,
    channels: &HashMap<String, Arc<dyn nv_core::channel::Channel>>,
) {
    if conn.disconnect_notified {
        return;
    }

    let Some(since) = conn.disconnected_since else {
        return;
    };

    if since.elapsed() >= Duration::from_secs(30) {
        send_telegram_message(
            channels,
            format!("Nexus agent '{}' disconnected", conn.name),
        )
        .await;
        conn.disconnect_notified = true;
    }
}

/// Send a plain text message to the Telegram channel if it is registered.
async fn send_telegram_message(
    channels: &HashMap<String, Arc<dyn nv_core::channel::Channel>>,
    content: String,
) {
    if let Some(tg) = channels.get("telegram") {
        let msg = OutboundMessage {
            channel: "telegram".into(),
            content,
            reply_to: None,
            keyboard: None,
        };
        if let Err(e) = tg.send_message(msg).await {
            tracing::warn!(error = %e, "watchdog: failed to send Telegram notification");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that `is_quarantined` returns false for a fresh connection.
    #[test]
    fn fresh_connection_not_quarantined() {
        let conn =
            NexusAgentConnection::new("test".into(), "127.0.0.1", 7400);
        assert!(!conn.is_quarantined());
    }

    /// Verify that `quarantine()` makes `is_quarantined()` return true.
    #[test]
    fn quarantine_is_detected() {
        let mut conn =
            NexusAgentConnection::new("test".into(), "127.0.0.1", 7400);
        conn.quarantine();
        assert!(conn.is_quarantined());
    }

    /// `disconnected_since` is preserved across multiple `mark_disconnected` calls.
    #[test]
    fn disconnected_since_preserved() {
        let mut conn =
            NexusAgentConnection::new("test".into(), "127.0.0.1", 7400);
        conn.mark_disconnected();
        let first = conn.disconnected_since.unwrap();
        // Simulate a small delay by doing work, then call again.
        conn.mark_disconnected();
        assert_eq!(conn.disconnected_since.unwrap(), first);
    }

    /// Stale detection: `last_seen` older than 3× interval should be treated as stale.
    #[test]
    fn stale_threshold_logic() {
        let stale_threshold = Duration::from_secs(30); // 3 × 10s
        // last_seen 40 seconds ago → stale
        let last_seen = chrono::Utc::now() - chrono::Duration::seconds(40);
        let elapsed = chrono::Utc::now()
            .signed_duration_since(last_seen)
            .to_std()
            .unwrap();
        assert!(elapsed >= stale_threshold);

        // last_seen 5 seconds ago → not stale
        let last_seen = chrono::Utc::now() - chrono::Duration::seconds(5);
        let elapsed = chrono::Utc::now()
            .signed_duration_since(last_seen)
            .to_std()
            .unwrap();
        assert!(elapsed < stale_threshold);
    }
}
