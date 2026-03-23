use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

use super::types::{
    ConnectionProperties, GatewayOpcode, GatewayPayload, HeartbeatPayload, HelloData,
    IdentifyData, IdentifyPayload, Message, ReadyData, BOT_INTENTS,
};

/// Discord gateway URL (API v10, JSON encoding).
const GATEWAY_URL: &str = "wss://gateway.discord.gg/?v=10&encoding=json";

type WsSink = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, WsMessage>;

/// Manages the Discord gateway WebSocket connection.
///
/// Handles the full gateway lifecycle: Hello → Identify → READY,
/// heartbeat loop, and MESSAGE_CREATE event buffering.
pub struct GatewayConnection {
    token: String,
    /// Bot's own user ID (set after READY). Used to filter self-messages.
    pub bot_user_id: Arc<Mutex<Option<String>>>,
    /// Session ID for resume (set after READY).
    session_id: Arc<Mutex<Option<String>>>,
    /// Last sequence number received from gateway.
    sequence: Arc<Mutex<Option<u64>>>,
    /// Buffered MESSAGE_CREATE events ready for poll_messages().
    pub message_buffer: Arc<Mutex<VecDeque<Message>>>,
    /// Write half of the WebSocket (for sending heartbeats + close).
    ws_sink: Arc<Mutex<Option<WsSink>>>,
}

impl GatewayConnection {
    pub fn new(token: &str) -> Self {
        Self {
            token: token.to_string(),
            bot_user_id: Arc::new(Mutex::new(None)),
            session_id: Arc::new(Mutex::new(None)),
            sequence: Arc::new(Mutex::new(None)),
            message_buffer: Arc::new(Mutex::new(VecDeque::new())),
            ws_sink: Arc::new(Mutex::new(None)),
        }
    }

    /// Connect to the Discord gateway and start the event loop.
    ///
    /// This spawns two background tasks:
    /// 1. Heartbeat loop (periodic keep-alive)
    /// 2. Event listener (reads gateway events, buffers MESSAGE_CREATE)
    ///
    /// Returns after READY is received or an error occurs.
    pub async fn connect(&self) -> anyhow::Result<()> {
        let (ws_stream, _) = connect_async(GATEWAY_URL).await?;
        let (sink, mut stream) = ws_stream.split();

        *self.ws_sink.lock().await = Some(sink);

        // Step 1: Receive Hello (op 10)
        let hello_msg = stream
            .next()
            .await
            .ok_or_else(|| anyhow::anyhow!("gateway closed before Hello"))??;

        let hello_text = hello_msg
            .into_text()
            .map_err(|e| anyhow::anyhow!("Hello was not text: {e}"))?;
        let hello_payload: GatewayPayload = serde_json::from_str(&hello_text)?;

        if hello_payload.op != GatewayOpcode::Hello as u8 {
            anyhow::bail!(
                "expected Hello (op 10), got op {}",
                hello_payload.op
            );
        }

        let hello_data: HelloData = serde_json::from_value(
            hello_payload
                .d
                .ok_or_else(|| anyhow::anyhow!("Hello payload missing data"))?,
        )?;

        let heartbeat_interval = Duration::from_millis(hello_data.heartbeat_interval);
        tracing::info!(
            interval_ms = hello_data.heartbeat_interval,
            "Discord gateway: received Hello"
        );

        // Step 2: Send Identify (op 2)
        let identify = IdentifyPayload {
            op: GatewayOpcode::Identify as u8,
            d: IdentifyData {
                token: self.token.clone(),
                intents: BOT_INTENTS,
                properties: ConnectionProperties {
                    os: "linux".to_string(),
                    browser: "nv".to_string(),
                    device: "nv".to_string(),
                },
            },
        };

        let identify_json = serde_json::to_string(&identify)?;
        {
            let mut sink = self.ws_sink.lock().await;
            if let Some(ref mut s) = *sink {
                s.send(WsMessage::Text(identify_json.into())).await?;
            }
        }
        tracing::info!("Discord gateway: sent Identify");

        // Step 3: Wait for READY event
        loop {
            let msg = stream
                .next()
                .await
                .ok_or_else(|| anyhow::anyhow!("gateway closed before READY"))??;

            let text = match msg {
                WsMessage::Text(t) => t.to_string(),
                WsMessage::Close(_) => anyhow::bail!("gateway closed before READY"),
                _ => continue,
            };

            let payload: GatewayPayload = serde_json::from_str(&text)?;

            // Update sequence
            if let Some(s) = payload.s {
                *self.sequence.lock().await = Some(s);
            }

            if payload.op == GatewayOpcode::Dispatch as u8
                && payload.t.as_deref() == Some("READY")
            {
                let ready: ReadyData = serde_json::from_value(
                    payload
                        .d
                        .ok_or_else(|| anyhow::anyhow!("READY missing data"))?,
                )?;
                *self.bot_user_id.lock().await = Some(ready.user.id.clone());
                *self.session_id.lock().await = Some(ready.session_id.clone());
                tracing::info!(
                    bot_user = %ready.user.username,
                    session_id = %ready.session_id,
                    "Discord gateway: READY"
                );
                break;
            }
        }

        // Step 4: Spawn heartbeat loop
        let hb_sequence = Arc::clone(&self.sequence);
        let hb_sink = Arc::clone(&self.ws_sink);
        tokio::spawn(async move {
            heartbeat_loop(heartbeat_interval, hb_sequence, hb_sink).await;
        });

        // Step 5: Spawn event listener
        let ev_sequence = Arc::clone(&self.sequence);
        let ev_buffer = Arc::clone(&self.message_buffer);
        tokio::spawn(async move {
            event_loop(stream, ev_sequence, ev_buffer).await;
        });

        Ok(())
    }

    /// Drain all buffered messages since the last poll.
    pub async fn drain_messages(&self) -> Vec<Message> {
        let mut buffer = self.message_buffer.lock().await;
        buffer.drain(..).collect()
    }

    /// Send a close frame to the gateway WebSocket.
    pub async fn close(&self) -> anyhow::Result<()> {
        let mut sink = self.ws_sink.lock().await;
        if let Some(ref mut s) = *sink {
            let _ = s
                .send(WsMessage::Close(Some(
                    tokio_tungstenite::tungstenite::protocol::CloseFrame {
                        code: tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode::Normal,
                        reason: "shutting down".into(),
                    },
                )))
                .await;
        }
        *sink = None;
        Ok(())
    }
}

/// Periodic heartbeat sender.
async fn heartbeat_loop(
    interval: Duration,
    sequence: Arc<Mutex<Option<u64>>>,
    sink: Arc<Mutex<Option<WsSink>>>,
) {
    let mut ticker = tokio::time::interval(interval);
    // Skip the immediate first tick
    ticker.tick().await;

    loop {
        ticker.tick().await;

        let seq = *sequence.lock().await;
        let payload = HeartbeatPayload {
            op: GatewayOpcode::Heartbeat as u8,
            d: seq,
        };

        let json = match serde_json::to_string(&payload) {
            Ok(j) => j,
            Err(e) => {
                tracing::error!(error = %e, "failed to serialize heartbeat");
                continue;
            }
        };

        let mut guard = sink.lock().await;
        if let Some(ref mut s) = *guard {
            if let Err(e) = s.send(WsMessage::Text(json.into())).await {
                tracing::error!(error = %e, "failed to send heartbeat — gateway disconnected");
                return;
            }
            tracing::trace!(seq = ?seq, "heartbeat sent");
        } else {
            tracing::warn!("heartbeat: sink is gone, stopping heartbeat loop");
            return;
        }
    }
}

/// Read gateway events, buffer MESSAGE_CREATE, handle heartbeat ACKs.
async fn event_loop(
    mut stream: futures_util::stream::SplitStream<
        WebSocketStream<MaybeTlsStream<TcpStream>>,
    >,
    sequence: Arc<Mutex<Option<u64>>>,
    buffer: Arc<Mutex<VecDeque<Message>>>,
) {
    while let Some(msg_result) = stream.next().await {
        let msg = match msg_result {
            Ok(m) => m,
            Err(e) => {
                tracing::error!(error = %e, "gateway read error");
                break;
            }
        };

        let text = match msg {
            WsMessage::Text(t) => t.to_string(),
            WsMessage::Close(frame) => {
                tracing::info!(?frame, "gateway sent close frame");
                break;
            }
            _ => continue,
        };

        let payload: GatewayPayload = match serde_json::from_str(&text) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(error = %e, "failed to parse gateway payload");
                continue;
            }
        };

        // Update sequence number
        if let Some(s) = payload.s {
            *sequence.lock().await = Some(s);
        }

        let op = GatewayOpcode::from_u8(payload.op);

        match op {
            Some(GatewayOpcode::Dispatch) => {
                if let Some(event_name) = &payload.t {
                    if event_name == "MESSAGE_CREATE" {
                        if let Some(data) = payload.d {
                            match serde_json::from_value::<Message>(data) {
                                Ok(message) => {
                                    buffer.lock().await.push_back(message);
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        error = %e,
                                        "failed to parse MESSAGE_CREATE"
                                    );
                                }
                            }
                        }
                    }
                    // Other dispatch events are ignored for now
                }
            }
            Some(GatewayOpcode::HeartbeatAck) => {
                tracing::trace!("heartbeat ACK received");
            }
            Some(GatewayOpcode::Reconnect) => {
                tracing::warn!("gateway requested reconnect");
                break;
            }
            Some(GatewayOpcode::InvalidSession) => {
                tracing::warn!("gateway: invalid session");
                break;
            }
            _ => {
                tracing::debug!(op = payload.op, "unhandled gateway opcode");
            }
        }
    }

    tracing::warn!("gateway event loop ended");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gateway_url_is_valid() {
        assert!(GATEWAY_URL.starts_with("wss://"));
        assert!(GATEWAY_URL.contains("gateway.discord.gg"));
        assert!(GATEWAY_URL.contains("v=10"));
        assert!(GATEWAY_URL.contains("encoding=json"));
    }

    #[tokio::test]
    async fn message_buffer_drain() {
        let conn = GatewayConnection::new("test-token");

        // Simulate buffering messages
        {
            let mut buf = conn.message_buffer.lock().await;
            buf.push_back(Message {
                id: "1".into(),
                channel_id: "ch-1".into(),
                guild_id: Some("g-1".into()),
                author: super::super::types::User {
                    id: "u-1".into(),
                    username: "user1".into(),
                    bot: Some(false),
                },
                content: "hello".into(),
                timestamp: "2024-01-01T00:00:00+00:00".into(),
            });
            buf.push_back(Message {
                id: "2".into(),
                channel_id: "ch-1".into(),
                guild_id: Some("g-1".into()),
                author: super::super::types::User {
                    id: "u-2".into(),
                    username: "user2".into(),
                    bot: Some(false),
                },
                content: "world".into(),
                timestamp: "2024-01-01T00:00:01+00:00".into(),
            });
        }

        let messages = conn.drain_messages().await;
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].content, "hello");
        assert_eq!(messages[1].content, "world");

        // Buffer is now empty
        let messages = conn.drain_messages().await;
        assert!(messages.is_empty());
    }
}
