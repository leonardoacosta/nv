use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use nv_core::types::{CliCommand, CliRequest, CronEvent, Trigger};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};

use crate::health::HealthState;

/// Shared state for the HTTP server.
#[derive(Clone)]
pub struct HttpState {
    pub trigger_tx: mpsc::UnboundedSender<Trigger>,
    pub health: Arc<HealthState>,
}

/// Request body for POST /ask.
#[derive(Debug, Deserialize)]
pub struct AskRequest {
    pub question: String,
}

/// Response body for POST /ask.
#[derive(Debug, Serialize, Deserialize)]
pub struct AskResponse {
    pub answer: String,
}

/// Response body for POST /digest.
#[derive(Debug, Serialize, Deserialize)]
pub struct DigestResponse {
    pub status: String,
    pub message: String,
}

/// Build the axum router with all HTTP endpoints.
pub fn build_router(state: Arc<HttpState>) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/ask", post(ask_handler))
        .route("/digest", post(digest_handler))
        .with_state(state)
}

/// GET /health — returns JSON with daemon health state.
async fn health_handler(
    State(state): State<Arc<HttpState>>,
) -> impl IntoResponse {
    let resp = state.health.to_health_response().await;
    (StatusCode::OK, Json(resp))
}

/// POST /ask — send a question through the agent loop and return the answer.
///
/// Pushes a `Trigger::CliCommand(Ask(question))` into the agent's mpsc channel
/// with a oneshot response channel, then waits for the agent to respond.
/// 60-second timeout.
async fn ask_handler(
    State(state): State<Arc<HttpState>>,
    Json(req): Json<AskRequest>,
) -> impl IntoResponse {
    if req.question.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(AskResponse {
                answer: "Question cannot be empty.".into(),
            }),
        );
    }

    let (response_tx, response_rx) = oneshot::channel::<String>();

    let trigger = Trigger::CliCommand(CliRequest {
        command: CliCommand::Ask(req.question.clone()),
        response_tx: Some(response_tx),
    });

    if state.trigger_tx.send(trigger).is_err() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(AskResponse {
                answer: "Agent loop is not running.".into(),
            }),
        );
    }

    // Wait for the agent loop to process and respond (60s timeout)
    match tokio::time::timeout(std::time::Duration::from_secs(60), response_rx).await {
        Ok(Ok(answer)) => (StatusCode::OK, Json(AskResponse { answer })),
        Ok(Err(_)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AskResponse {
                answer: "Agent response channel closed unexpectedly.".into(),
            }),
        ),
        Err(_) => (
            StatusCode::GATEWAY_TIMEOUT,
            Json(AskResponse {
                answer: "Query timed out after 60 seconds.".into(),
            }),
        ),
    }
}

/// POST /digest — trigger an immediate digest.
///
/// Pushes `Trigger::Cron(CronEvent::Digest)` into the agent channel.
/// Returns 202 Accepted immediately (digest is generated asynchronously).
async fn digest_handler(
    State(state): State<Arc<HttpState>>,
) -> impl IntoResponse {
    if state.trigger_tx.send(Trigger::Cron(CronEvent::Digest)).is_err() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(DigestResponse {
                status: "error".into(),
                message: "Agent loop is not running.".into(),
            }),
        );
    }

    tracing::info!("digest triggered via HTTP POST /digest");

    (
        StatusCode::ACCEPTED,
        Json(DigestResponse {
            status: "accepted".into(),
            message: "Digest triggered. It will arrive on Telegram shortly.".into(),
        }),
    )
}

/// Start the HTTP server on the given port.
///
/// Runs until the listener is dropped or the runtime shuts down.
pub async fn run_http_server(
    port: u16,
    trigger_tx: mpsc::UnboundedSender<Trigger>,
    health: Arc<HealthState>,
) -> anyhow::Result<()> {
    let state = Arc::new(HttpState { trigger_tx, health });
    let app = build_router(state);

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{port}")).await?;
    tracing::info!(port, "HTTP server listening");

    axum::serve(listener, app).await?;
    Ok(())
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    fn setup() -> (Arc<HttpState>, mpsc::UnboundedReceiver<Trigger>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let health = Arc::new(HealthState::new());
        let state = Arc::new(HttpState {
            trigger_tx: tx,
            health,
        });
        (state, rx)
    }

    #[tokio::test]
    async fn health_endpoint_returns_json() {
        let (state, _rx) = setup();
        let app = build_router(state);

        let request = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let resp: crate::health::HealthResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(resp.status, "ok");
        assert!(!resp.version.is_empty());
    }

    #[tokio::test]
    async fn ask_endpoint_empty_question_returns_bad_request() {
        let (state, _rx) = setup();
        let app = build_router(state);

        let request = Request::builder()
            .method("POST")
            .uri("/ask")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"question": ""}"#))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn ask_endpoint_sends_trigger_and_returns_answer() {
        let (state, mut rx) = setup();
        let app = build_router(state);

        // Spawn a task to simulate the agent loop responding
        tokio::spawn(async move {
            if let Some(trigger) = rx.recv().await {
                if let Trigger::CliCommand(req) = trigger {
                    if let CliCommand::Ask(q) = &req.command {
                        assert_eq!(q, "What's blocking OO?");
                    }
                    if let Some(tx) = req.response_tx {
                        tx.send("OO-42 is blocking the release.".into()).ok();
                    }
                }
            }
        });

        let request = Request::builder()
            .method("POST")
            .uri("/ask")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"question": "What's blocking OO?"}"#))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let resp: AskResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(resp.answer, "OO-42 is blocking the release.");
    }

    #[tokio::test]
    async fn digest_endpoint_returns_accepted() {
        let (state, mut rx) = setup();
        let app = build_router(state);

        let request = Request::builder()
            .method("POST")
            .uri("/digest")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let resp: DigestResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(resp.status, "accepted");

        // Verify trigger was sent
        let trigger = rx.try_recv().unwrap();
        match trigger {
            Trigger::Cron(CronEvent::Digest) => {} // Expected
            other => panic!("unexpected trigger: {other:?}"),
        }
    }

    #[tokio::test]
    async fn digest_endpoint_returns_unavailable_when_closed() {
        let (state, rx) = setup();
        drop(rx);
        let app = build_router(state);

        let request = Request::builder()
            .method("POST")
            .uri("/digest")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn ask_endpoint_returns_unavailable_when_channel_closed() {
        let (state, rx) = setup();
        drop(rx); // Close the receiver immediately

        let app = build_router(state);

        let request = Request::builder()
            .method("POST")
            .uri("/ask")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"question": "test"}"#))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
