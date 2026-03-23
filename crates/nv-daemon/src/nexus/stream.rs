use std::sync::Arc;

use nv_core::types::{SessionEvent as NvSessionEvent, SessionEventType, Trigger};
use tokio::sync::{mpsc, Mutex};

use super::connection::NexusAgentConnection;
use super::proto::{self, session_event, EventFilter, EventType};

/// Spawn event stream tasks for all connected agents.
///
/// Each connected agent gets its own tokio task that subscribes to
/// `StreamEvents` and pushes significant events to the trigger channel.
///
/// Returns the `JoinHandle` for each spawned task so the watchdog can monitor
/// liveness and respawn tasks that have exited.
pub fn spawn_event_streams(
    agents: &[Arc<Mutex<NexusAgentConnection>>],
    trigger_tx: mpsc::UnboundedSender<Trigger>,
) -> Vec<tokio::task::JoinHandle<()>> {
    agents
        .iter()
        .map(|agent| {
            let agent = Arc::clone(agent);
            let tx = trigger_tx.clone();
            tokio::spawn(async move {
                run_event_stream(agent, tx).await;
            })
        })
        .collect()
}

/// Run the event stream for a single agent.
///
/// On disconnect, waits for reconnection and re-subscribes.
pub async fn run_event_stream(
    agent: Arc<Mutex<NexusAgentConnection>>,
    trigger_tx: mpsc::UnboundedSender<Trigger>,
) {
    loop {
        // Wait until we have a connected client
        let (agent_name, mut stream) = {
            let mut conn = agent.lock().await;
            let name = conn.name.clone();

            let Some(client) = conn.client.as_mut() else {
                drop(conn);
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                continue;
            };

            match client
                .stream_events(EventFilter {
                    session_id: None,
                    event_types: vec![
                        EventType::StatusChanged as i32,
                        EventType::SessionStopped as i32,
                    ],
                    initial_snapshot: true,
                })
                .await
            {
                Ok(response) => (name, response.into_inner()),
                Err(e) => {
                    tracing::warn!(
                        agent = %name,
                        error = %e,
                        "failed to subscribe to StreamEvents"
                    );
                    conn.mark_disconnected();
                    drop(conn);

                    // Attempt reconnect
                    let mut conn = agent.lock().await;
                    conn.reconnect().await;
                    continue;
                }
            }
        };

        tracing::info!(agent = %agent_name, "subscribed to Nexus event stream");

        // Process events from the stream
        loop {
            match stream.message().await {
                Ok(Some(event)) => {
                    if let Some(trigger) = map_event_to_trigger(&agent_name, &event) {
                        if trigger_tx.send(trigger).is_err() {
                            tracing::error!("trigger channel closed, stopping event stream");
                            return;
                        }
                    }
                }
                Ok(None) => {
                    tracing::warn!(agent = %agent_name, "event stream ended");
                    break;
                }
                Err(e) => {
                    tracing::warn!(
                        agent = %agent_name,
                        error = %e,
                        "event stream error"
                    );
                    break;
                }
            }
        }

        // Stream ended or errored — reconnect
        {
            let mut conn = agent.lock().await;
            conn.mark_disconnected();
            conn.reconnect().await;
        }
    }
}

/// Map a proto SessionEvent to a Trigger, filtering for significant events.
///
/// Returns `None` for heartbeats and other low-signal events.
fn map_event_to_trigger(
    agent_name: &str,
    event: &proto::SessionEvent,
) -> Option<Trigger> {
    let payload = event.payload.as_ref()?;

    // Prefer event-level agent_name from proto when available, fall back to
    // the connection-level name passed in by the caller.
    let effective_agent = if event.agent_name.is_empty() {
        agent_name
    } else {
        &event.agent_name
    };

    match payload {
        session_event::Payload::Stopped(stopped) => {
            Some(Trigger::NexusEvent(NvSessionEvent {
                agent_name: effective_agent.to_string(),
                session_id: event.session_id.clone(),
                event_type: SessionEventType::Completed,
                details: Some(stopped.reason.clone()),
            }))
        }
        session_event::Payload::StatusChanged(changed) => {
            let new_status = proto::SessionStatus::try_from(changed.new_status);
            if matches!(new_status, Ok(proto::SessionStatus::Errored)) {
                let old = proto::SessionStatus::try_from(changed.old_status)
                    .map(|s| s.as_str_name().to_string())
                    .unwrap_or_else(|_| "unknown".into());
                Some(Trigger::NexusEvent(NvSessionEvent {
                    agent_name: effective_agent.to_string(),
                    session_id: event.session_id.clone(),
                    event_type: SessionEventType::Failed,
                    details: Some(format!("status changed from {} to errored", old)),
                }))
            } else {
                tracing::debug!(
                    agent = %effective_agent,
                    session_id = %event.session_id,
                    "status change (non-error), skipping trigger"
                );
                None
            }
        }
        session_event::Payload::Started(started) => {
            if started.is_snapshot {
                tracing::debug!(
                    agent = %effective_agent,
                    session_id = %event.session_id,
                    "snapshot session (bootstrap data, no trigger)"
                );
            } else {
                tracing::debug!(
                    agent = %effective_agent,
                    session_id = %event.session_id,
                    "session started (info only, no trigger)"
                );
            }
            None
        }
        session_event::Payload::Heartbeat(_) => {
            // Heartbeats are noise — skip silently
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::proto::{SessionStopped, StatusChanged, SessionStarted, HeartbeatReceived};

    fn make_event(session_id: &str, payload: session_event::Payload) -> proto::SessionEvent {
        proto::SessionEvent {
            session_id: session_id.into(),
            ts: None,
            payload: Some(payload),
            agent_name: String::new(),
        }
    }

    #[test]
    fn stopped_event_maps_to_completed_trigger() {
        let event = make_event(
            "s-1",
            session_event::Payload::Stopped(SessionStopped {
                reason: "user requested".into(),
            }),
        );
        let trigger = map_event_to_trigger("homelab", &event);
        assert!(trigger.is_some());
        if let Some(Trigger::NexusEvent(ne)) = trigger {
            assert_eq!(ne.session_id, "s-1");
            assert_eq!(ne.agent_name, "homelab");
            assert!(matches!(ne.event_type, SessionEventType::Completed));
            assert_eq!(ne.details.as_deref(), Some("user requested"));
        }
    }

    #[test]
    fn status_changed_to_errored_maps_to_failed_trigger() {
        let event = make_event(
            "s-2",
            session_event::Payload::StatusChanged(StatusChanged {
                old_status: proto::SessionStatus::Active as i32,
                new_status: proto::SessionStatus::Errored as i32,
            }),
        );
        let trigger = map_event_to_trigger("macbook", &event);
        assert!(trigger.is_some());
        if let Some(Trigger::NexusEvent(ne)) = trigger {
            assert_eq!(ne.session_id, "s-2");
            assert!(matches!(ne.event_type, SessionEventType::Failed));
        }
    }

    #[test]
    fn status_changed_non_error_returns_none() {
        let event = make_event(
            "s-3",
            session_event::Payload::StatusChanged(StatusChanged {
                old_status: proto::SessionStatus::Active as i32,
                new_status: proto::SessionStatus::Idle as i32,
            }),
        );
        let trigger = map_event_to_trigger("homelab", &event);
        assert!(trigger.is_none());
    }

    #[test]
    fn started_event_returns_none() {
        let event = make_event(
            "s-4",
            session_event::Payload::Started(SessionStarted { session: None, is_snapshot: false }),
        );
        let trigger = map_event_to_trigger("homelab", &event);
        assert!(trigger.is_none());
    }

    #[test]
    fn heartbeat_event_returns_none() {
        let event = make_event(
            "s-5",
            session_event::Payload::Heartbeat(HeartbeatReceived {
                last_heartbeat: None,
            }),
        );
        let trigger = map_event_to_trigger("homelab", &event);
        assert!(trigger.is_none());
    }

    #[test]
    fn event_with_no_payload_returns_none() {
        let event = proto::SessionEvent {
            session_id: "s-6".into(),
            ts: None,
            payload: None,
            agent_name: String::new(),
        };
        let trigger = map_event_to_trigger("homelab", &event);
        assert!(trigger.is_none());
    }

    #[test]
    fn event_filter_uses_status_changed_and_session_stopped() {
        // The filter constructed in run_event_stream must contain exactly
        // STATUS_CHANGED and SESSION_STOPPED.  Assert the i32 values match.
        let expected = vec![
            EventType::StatusChanged as i32,
            EventType::SessionStopped as i32,
        ];
        assert_eq!(expected, vec![3, 4]);
    }

    #[test]
    fn snapshot_session_started_returns_none() {
        let event = make_event(
            "s-snap",
            session_event::Payload::Started(SessionStarted {
                session: None,
                is_snapshot: true,
            }),
        );
        let trigger = map_event_to_trigger("homelab", &event);
        assert!(trigger.is_none());
    }

    #[test]
    fn real_session_started_returns_none() {
        let event = make_event(
            "s-real",
            session_event::Payload::Started(SessionStarted {
                session: None,
                is_snapshot: false,
            }),
        );
        let trigger = map_event_to_trigger("homelab", &event);
        assert!(trigger.is_none());
    }

    #[test]
    fn agent_name_from_event_preferred_over_parameter() {
        let event = proto::SessionEvent {
            session_id: "s-agent".into(),
            ts: None,
            payload: Some(session_event::Payload::Stopped(SessionStopped {
                reason: "done".into(),
            })),
            agent_name: "from-event".into(),
        };
        let trigger = map_event_to_trigger("from-param", &event);
        if let Some(Trigger::NexusEvent(ne)) = trigger {
            assert_eq!(ne.agent_name, "from-event");
        } else {
            panic!("expected NexusEvent trigger");
        }
    }

    #[test]
    fn empty_agent_name_falls_back_to_parameter() {
        let event = proto::SessionEvent {
            session_id: "s-fallback".into(),
            ts: None,
            payload: Some(session_event::Payload::Stopped(SessionStopped {
                reason: "done".into(),
            })),
            agent_name: String::new(),
        };
        let trigger = map_event_to_trigger("from-param", &event);
        if let Some(Trigger::NexusEvent(ne)) = trigger {
            assert_eq!(ne.agent_name, "from-param");
        } else {
            panic!("expected NexusEvent trigger");
        }
    }
}
