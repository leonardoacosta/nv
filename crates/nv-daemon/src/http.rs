use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use nv_core::types::{CliCommand, CliRequest, CronEvent, Trigger};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};
use tokio::sync::Mutex as TokioMutex;

use crate::briefing_store::BriefingStore;
use crate::cold_start_store::ColdStartStore;
use crate::health::HealthState;
use crate::tools::jira::webhooks::{jira_webhook_handler, JiraWebhookState};
use crate::messages::MessageStore;
use crate::channels::teams::types::{ChangeNotificationCollection, ChatMessage};

/// Shared state for the HTTP server.
#[derive(Clone)]
pub struct HttpState {
    pub trigger_tx: mpsc::UnboundedSender<Trigger>,
    pub health: Arc<HealthState>,
    pub stats_db_path: PathBuf,
    /// Shared buffer for Teams webhook messages. None if Teams is not configured.
    pub teams_message_buffer: Option<Arc<TokioMutex<VecDeque<ChatMessage>>>>,
    /// Teams client for fetching full message content from notifications.
    pub teams_client: Option<Arc<crate::channels::teams::client::TeamsClient>>,
    /// Jira webhook shared state. None if Jira webhooks are not configured.
    pub jira_webhook_state: Option<Arc<JiraWebhookState>>,
    /// Weekly budget in USD for Claude API usage stats.
    pub weekly_budget_usd: f64,
    /// Teams clientState secret for webhook notification validation. None if Teams is not configured.
    pub teams_client_state: Option<String>,
    /// Morning briefing log store. None if not initialised.
    pub briefing_store: Option<Arc<BriefingStore>>,
    /// Cold-start timing event store. None if not initialised.
    pub cold_start_store: Option<Arc<std::sync::Mutex<ColdStartStore>>>,
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
    let mut router = Router::new()
        .route("/health", get(health_handler))
        .route("/ask", post(ask_handler))
        .route("/digest", post(digest_handler))
        .route("/test/ping", get(test_ping_handler))
        .route("/stats", get(stats_handler))
        .route("/webhooks/teams", post(teams_webhook_handler))
        .route("/api/briefing", get(get_briefing_handler))
        .route("/api/briefing/history", get(get_briefing_history_handler))
        .route("/api/cold-starts", get(get_cold_starts_handler));

    // Add Jira webhook route if configured (uses its own sub-state)
    if let Some(jira_state) = &state.jira_webhook_state {
        let jira_router = Router::new()
            .route("/webhooks/jira", post(jira_webhook_handler))
            .with_state(Arc::clone(jira_state));
        router = router.merge(jira_router);
    }

    router.with_state(state)
}

/// Query params for Teams webhook validation handshake.
#[derive(Debug, Deserialize)]
pub struct TeamsWebhookQuery {
    #[serde(rename = "validationToken")]
    pub validation_token: Option<String>,
}

/// POST /webhooks/teams — receive MS Graph subscription notifications.
///
/// Handles two cases:
/// 1. Subscription validation: returns validationToken as text/plain.
/// 2. Change notifications: parses the payload, fetches full message content,
///    and pushes to the shared Teams message buffer.
async fn teams_webhook_handler(
    State(state): State<Arc<HttpState>>,
    Query(query): Query<TeamsWebhookQuery>,
    body: String,
) -> impl IntoResponse {
    // Case 1: Subscription validation handshake
    if let Some(token) = query.validation_token {
        tracing::info!("Teams webhook validation handshake");
        return (
            StatusCode::OK,
            [("content-type", "text/plain")],
            token,
        ).into_response();
    }

    // Case 2: Change notification
    let (buffer, client) = match (&state.teams_message_buffer, &state.teams_client) {
        (Some(buf), Some(client)) => (buf, client),
        _ => {
            tracing::warn!("Teams webhook received but Teams is not configured");
            return StatusCode::OK.into_response();
        }
    };

    let notifications: ChangeNotificationCollection = match serde_json::from_str(&body) {
        Ok(n) => n,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to parse Teams change notification");
            // Return 200 to prevent MS Graph from retrying
            return StatusCode::OK.into_response();
        }
    };

    // Validate clientState on every notification (fail-closed).
    if let Some(expected) = &state.teams_client_state {
        for notification in &notifications.value {
            match &notification.client_state {
                Some(received) if received == expected => {}
                Some(received) => {
                    tracing::warn!(
                        received = %received,
                        "Teams webhook clientState mismatch — rejecting notification"
                    );
                    return StatusCode::UNAUTHORIZED.into_response();
                }
                None => {
                    tracing::warn!("Teams webhook notification missing clientState — rejecting");
                    return StatusCode::UNAUTHORIZED.into_response();
                }
            }
        }
    }

    for notification in &notifications.value {
        // Fetch the full message content via MS Graph API
        let resource = &notification.resource;
        match client.get_message(resource).await {
            Ok(msg) => {
                buffer.lock().await.push_back(msg);
            }
            Err(e) => {
                tracing::warn!(
                    resource = %resource,
                    error = %e,
                    "Failed to fetch Teams message from notification"
                );
            }
        }
    }

    StatusCode::OK.into_response()
}

/// Query parameters for GET /health.
#[derive(Debug, Deserialize, Default)]
pub struct HealthQuery {
    /// When `deep=true`, run connectivity probes for all configured services
    /// and include the results in the `tools` field of the response.
    pub deep: Option<bool>,
}

/// GET /health — returns JSON with daemon health state.
///
/// With `?deep=true`, runs read probes against all configured service clients
/// and attaches the results as `"tools": { "<name>": { "status": "healthy", ... } }`.
async fn health_handler(
    State(state): State<Arc<HttpState>>,
    Query(query): Query<HealthQuery>,
) -> impl IntoResponse {
    let resp = if query.deep.unwrap_or(false) {
        state.health.to_deep_health_response().await
    } else {
        state.health.to_health_response().await
    };
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

/// Response body for GET /test/ping.
#[derive(Debug, Serialize)]
pub struct TestPingResponse {
    pub ok: bool,
    pub elapsed_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_preview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Concurrency guard for /test/ping — only one test at a time.
static TEST_PING_LOCK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// GET /test/ping — e2e pipeline smoke test.
///
/// Injects a synthetic "ping" message into the worker pipeline via CliCommand::Ask,
/// waits up to 60s for the response, and returns pass/fail with timing metrics.
/// Only one test ping can run at a time (returns 429 if busy).
async fn test_ping_handler(
    State(state): State<Arc<HttpState>>,
) -> impl IntoResponse {
    // Concurrency guard — reject if another test is in progress
    if TEST_PING_LOCK.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(TestPingResponse {
                ok: false,
                elapsed_ms: 0,
                response_preview: None,
                error: Some("test already in progress".into()),
            }),
        );
    }

    let start = std::time::Instant::now();
    let (response_tx, response_rx) = oneshot::channel::<String>();

    let trigger = Trigger::CliCommand(CliRequest {
        command: CliCommand::Ask("ping".into()),
        response_tx: Some(response_tx),
    });

    if state.trigger_tx.send(trigger).is_err() {
        TEST_PING_LOCK.store(false, std::sync::atomic::Ordering::SeqCst);
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(TestPingResponse {
                ok: false,
                elapsed_ms: start.elapsed().as_millis() as u64,
                response_preview: None,
                error: Some("agent loop is not running".into()),
            }),
        );
    }

    let result = tokio::time::timeout(std::time::Duration::from_secs(60), response_rx).await;
    TEST_PING_LOCK.store(false, std::sync::atomic::Ordering::SeqCst);

    let elapsed_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(Ok(answer)) => {
            let preview = if answer.len() > 200 {
                format!("{}...", &answer[..200])
            } else {
                answer.clone()
            };
            tracing::info!(elapsed_ms, response_len = answer.len(), "test/ping: ok");
            (
                StatusCode::OK,
                Json(TestPingResponse {
                    ok: true,
                    elapsed_ms,
                    response_preview: Some(preview),
                    error: None,
                }),
            )
        }
        Ok(Err(_)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(TestPingResponse {
                ok: false,
                elapsed_ms,
                response_preview: None,
                error: Some("response channel closed".into()),
            }),
        ),
        Err(_) => (
            StatusCode::GATEWAY_TIMEOUT,
            Json(TestPingResponse {
                ok: false,
                elapsed_ms,
                response_preview: None,
                error: Some("timeout".into()),
            }),
        ),
    }
}

/// GET /stats — returns message store statistics as JSON.
///
/// Opens a read-only connection to the message store database
/// and returns aggregate stats (total messages, daily counts, tool usage, etc.).
async fn stats_handler(
    State(state): State<Arc<HttpState>>,
) -> impl IntoResponse {
    match MessageStore::init(&state.stats_db_path) {
        Ok(store) => {
            let msg_stats = match store.stats() {
                Ok(r) => serde_json::to_value(r).unwrap(),
                Err(e) => {
                    tracing::error!(error = %e, "failed to query message stats");
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"error": format!("Failed to query stats: {e}")})),
                    ).into_response();
                }
            };
            let tool_stats = match store.tool_stats() {
                Ok(r) => serde_json::to_value(r).unwrap(),
                Err(e) => {
                    tracing::error!(error = %e, "failed to query tool stats");
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"error": format!("Failed to query tool stats: {e}")})),
                    ).into_response();
                }
            };

            let usage_stats = match store.usage_stats() {
                Ok(r) => serde_json::to_value(r).unwrap(),
                Err(e) => {
                    tracing::error!(error = %e, "failed to query usage stats");
                    serde_json::json!(null)
                }
            };
            let budget_status = match store.usage_budget_status(state.weekly_budget_usd) {
                Ok(r) => serde_json::to_value(r).unwrap_or_default(),
                Err(e) => {
                    tracing::error!(error = %e, "failed to query budget status");
                    serde_json::json!(null)
                }
            };

            // Load cached account info (non-blocking — reads a local JSON file)
            let account_info = crate::account::load_cached()
                .map(|info| serde_json::to_value(info).unwrap_or_default());

            // Merge message stats + tool_usage + claude_usage + account sections
            let mut combined = msg_stats.as_object().cloned().unwrap_or_default();
            combined.insert("tool_usage".into(), tool_stats);
            combined.insert("claude_usage".into(), usage_stats);
            combined.insert("budget".into(), budget_status);
            if let Some(acct) = account_info {
                combined.insert("account".into(), acct);
            }
            (StatusCode::OK, Json(serde_json::Value::Object(combined))).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to open message store for stats");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to open message store: {e}")})),
            ).into_response()
        }
    }
}

// ── Briefing API ─────────────────────────────────────────────────────

/// Query parameters for `GET /api/briefing/history`.
#[derive(Debug, Deserialize)]
pub struct BriefingQuery {
    /// Maximum number of entries to return (default 10, max 30).
    pub limit: Option<usize>,
}

/// GET /api/briefing — return the most recent morning briefing entry.
///
/// Returns 200 with the latest `BriefingEntry` as JSON, or 404 if no
/// briefing has been stored yet.
async fn get_briefing_handler(
    State(state): State<Arc<HttpState>>,
) -> impl IntoResponse {
    let store = match &state.briefing_store {
        Some(s) => s,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "briefing store not configured"})),
            ).into_response();
        }
    };

    match store.latest() {
        Ok(Some(entry)) => (StatusCode::OK, Json(serde_json::to_value(entry).unwrap())).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "no briefing available"})),
        ).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to read briefing store");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to read briefing: {e}")})),
            ).into_response()
        }
    }
}

/// GET /api/briefing/history — return recent morning briefing entries.
///
/// Accepts `?limit=N` (1–30, default 10). Returns a JSON array of
/// `BriefingEntry` objects, newest first.
async fn get_briefing_history_handler(
    State(state): State<Arc<HttpState>>,
    Query(query): Query<BriefingQuery>,
) -> impl IntoResponse {
    let store = match &state.briefing_store {
        Some(s) => s,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "briefing store not configured"})),
            ).into_response();
        }
    };

    let limit = query.limit.unwrap_or(10).clamp(1, 30);

    match store.list(limit) {
        Ok(entries) => (StatusCode::OK, Json(serde_json::to_value(entries).unwrap())).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to read briefing history");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to read briefing history: {e}")})),
            ).into_response()
        }
    }
}

// ── Cold-Start API ─────────────────────────────────────────────────

/// Query parameters for `GET /api/cold-starts`.
#[derive(Debug, Deserialize)]
pub struct ColdStartsQuery {
    /// Maximum number of events to return (default 200, max 1000).
    pub limit: Option<usize>,
}

/// GET /api/cold-starts — return recent cold-start timing events plus
/// 24-hour percentile summary.
///
/// Accepts `?limit=N` (1–1000, default 200). Returns JSON:
/// `{ "events": [...], "percentiles": { "p50_ms", "p95_ms", "p99_ms", "sample_count" } }`.
async fn get_cold_starts_handler(
    State(state): State<Arc<HttpState>>,
    Query(query): Query<ColdStartsQuery>,
) -> impl IntoResponse {
    let cs_arc = match &state.cold_start_store {
        Some(s) => s,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "cold-start store not configured"})),
            )
                .into_response();
        }
    };

    let limit = query.limit.unwrap_or(200).clamp(1, 1000);

    let store = cs_arc.lock().unwrap();

    let events = match store.get_recent(limit) {
        Ok(e) => e,
        Err(e) => {
            tracing::error!(error = %e, "failed to read cold-start events");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to read events: {e}")})),
            )
                .into_response();
        }
    };

    let percentiles = match store.get_percentiles(24) {
        Ok(p) => p,
        Err(e) => {
            tracing::error!(error = %e, "failed to compute cold-start percentiles");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to compute percentiles: {e}")})),
            )
                .into_response();
        }
    };

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "events": events,
            "percentiles": percentiles,
        })),
    )
        .into_response()
}

/// Start the HTTP server on the given port.
///
/// Runs until the listener is dropped or the runtime shuts down.
#[allow(clippy::too_many_arguments)]
pub async fn run_http_server(
    port: u16,
    trigger_tx: mpsc::UnboundedSender<Trigger>,
    health: Arc<HealthState>,
    stats_db_path: PathBuf,
    teams_message_buffer: Option<Arc<TokioMutex<VecDeque<ChatMessage>>>>,
    teams_client: Option<Arc<crate::channels::teams::client::TeamsClient>>,
    jira_webhook_state: Option<Arc<JiraWebhookState>>,
    weekly_budget_usd: f64,
    teams_client_state: Option<String>,
    briefing_store: Option<Arc<BriefingStore>>,
    cold_start_store: Option<Arc<std::sync::Mutex<ColdStartStore>>>,
) -> anyhow::Result<()> {
    let state = Arc::new(HttpState {
        trigger_tx,
        health,
        stats_db_path,
        teams_message_buffer,
        teams_client,
        jira_webhook_state,
        weekly_budget_usd,
        teams_client_state,
        briefing_store,
        cold_start_store,
    });
    let app = build_router(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;
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

    fn setup() -> (Arc<HttpState>, mpsc::UnboundedReceiver<Trigger>, tempfile::TempDir) {
        let (tx, rx) = mpsc::unbounded_channel();
        let health = Arc::new(HealthState::new());
        let tmp = tempfile::TempDir::new().unwrap();
        let db_path = tmp.path().join("messages.db");
        // Initialize the message store so the DB file exists for /stats
        let _store = MessageStore::init(&db_path).unwrap();
        let state = Arc::new(HttpState {
            trigger_tx: tx,
            health,
            stats_db_path: db_path.clone(),
            teams_message_buffer: None,
            teams_client: None,
            jira_webhook_state: None,
            weekly_budget_usd: 50.0,
            teams_client_state: None,
            briefing_store: None,
            cold_start_store: None,
        });
        (state, rx, tmp)
    }

    #[tokio::test]
    async fn health_endpoint_returns_json() {
        let (state, _rx, _tmp) = setup();
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
        let (state, _rx, _tmp) = setup();
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
        let (state, mut rx, _tmp) = setup();
        let app = build_router(state);

        // Spawn a task to simulate the agent loop responding
        tokio::spawn(async move {
            if let Some(Trigger::CliCommand(req)) = rx.recv().await {
                if let CliCommand::Ask(q) = &req.command {
                    assert_eq!(q, "What's blocking OO?");
                }
                if let Some(tx) = req.response_tx {
                    tx.send("OO-42 is blocking the release.".into()).ok();
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
        let (state, mut rx, _tmp) = setup();
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
        let (state, rx, _tmp) = setup();
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
        let (state, rx, _tmp) = setup();
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

    #[tokio::test]
    async fn stats_endpoint_returns_json() {
        let (state, _rx, _tmp) = setup();
        let app = build_router(state);

        let request = Request::builder()
            .uri("/stats")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let resp: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(resp["total_messages"], 0);
        assert_eq!(resp["messages_today"], 0);
        assert_eq!(resp["total_tokens_in"], 0);
        assert_eq!(resp["total_tokens_out"], 0);
    }

    #[tokio::test]
    async fn teams_webhook_validation_returns_token() {
        let (state, _rx, _tmp) = setup();
        let app = build_router(state);

        let request = Request::builder()
            .method("POST")
            .uri("/webhooks/teams?validationToken=test-validation-token-123")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(
            std::str::from_utf8(&body).unwrap(),
            "test-validation-token-123"
        );
    }

    #[tokio::test]
    async fn teams_webhook_no_buffer_returns_ok() {
        // When Teams is not configured, webhook should still return 200
        let (state, _rx, _tmp) = setup();
        let app = build_router(state);

        let request = Request::builder()
            .method("POST")
            .uri("/webhooks/teams")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"value": []}"#))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    // ── Briefing API tests ────────────────────────────────────────────

    fn setup_with_briefing() -> (Arc<HttpState>, mpsc::UnboundedReceiver<Trigger>, tempfile::TempDir) {
        let (tx, rx) = mpsc::unbounded_channel();
        let health = Arc::new(HealthState::new());
        let tmp = tempfile::TempDir::new().unwrap();
        let db_path = tmp.path().join("messages.db");
        let _store = MessageStore::init(&db_path).unwrap();
        let briefing_store = Arc::new(BriefingStore::new(tmp.path()));
        let state = Arc::new(HttpState {
            trigger_tx: tx,
            health,
            stats_db_path: db_path.clone(),
            teams_message_buffer: None,
            teams_client: None,
            jira_webhook_state: None,
            weekly_budget_usd: 50.0,
            teams_client_state: None,
            briefing_store: Some(briefing_store),
            cold_start_store: None,
        });
        (state, rx, tmp)
    }

    // [5.1] GET /api/briefing returns 404 when store is empty
    #[tokio::test]
    async fn briefing_returns_404_when_empty() {
        let (state, _rx, _tmp) = setup_with_briefing();
        let app = build_router(state);

        let request = Request::builder()
            .uri("/api/briefing")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    // [5.2] GET /api/briefing returns 200 with the latest entry after one is appended
    #[tokio::test]
    async fn briefing_returns_latest_entry() {
        let (tx, rx) = mpsc::unbounded_channel::<Trigger>();
        let health = Arc::new(HealthState::new());
        let tmp = tempfile::TempDir::new().unwrap();
        let db_path = tmp.path().join("messages.db");
        let _store = MessageStore::init(&db_path).unwrap();
        let briefing_store = Arc::new(BriefingStore::new(tmp.path()));

        // Pre-populate with one entry.
        let entry = crate::briefing_store::BriefingEntry::new(
            "Good morning test",
            vec![],
            std::collections::HashMap::new(),
        );
        briefing_store.append(&entry).unwrap();

        let state = Arc::new(HttpState {
            trigger_tx: tx,
            health,
            stats_db_path: db_path,
            teams_message_buffer: None,
            teams_client: None,
            jira_webhook_state: None,
            weekly_budget_usd: 50.0,
            teams_client_state: None,
            briefing_store: Some(Arc::clone(&briefing_store)),
            cold_start_store: None,
        });
        drop(rx);

        let app = build_router(state);

        let request = Request::builder()
            .uri("/api/briefing")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let resp: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(resp["content"], "Good morning test");
    }

    // [5.3] GET /api/briefing/history returns array of entries
    #[tokio::test]
    async fn briefing_history_returns_entries() {
        let (tx, rx) = mpsc::unbounded_channel::<Trigger>();
        let health = Arc::new(HealthState::new());
        let tmp = tempfile::TempDir::new().unwrap();
        let db_path = tmp.path().join("messages.db");
        let _store = MessageStore::init(&db_path).unwrap();
        let briefing_store = Arc::new(BriefingStore::new(tmp.path()));

        for i in 0..5_usize {
            let e = crate::briefing_store::BriefingEntry::new(
                format!("Briefing {i}"),
                vec![],
                std::collections::HashMap::new(),
            );
            briefing_store.append(&e).unwrap();
        }

        let state = Arc::new(HttpState {
            trigger_tx: tx,
            health,
            stats_db_path: db_path,
            teams_message_buffer: None,
            teams_client: None,
            jira_webhook_state: None,
            weekly_budget_usd: 50.0,
            teams_client_state: None,
            briefing_store: Some(Arc::clone(&briefing_store)),
            cold_start_store: None,
        });
        drop(rx);

        let app = build_router(state);

        let request = Request::builder()
            .uri("/api/briefing/history?limit=3")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let entries: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let arr = entries.as_array().unwrap();
        assert_eq!(arr.len(), 3);
        // Newest first — last appended was "Briefing 4"
        assert_eq!(arr[0]["content"], "Briefing 4");
    }

    // [5.4] GET /api/briefing returns 404 when store is not configured (None)
    #[tokio::test]
    async fn briefing_returns_404_when_store_not_configured() {
        let (state, _rx, _tmp) = setup(); // uses briefing_store: None
        let app = build_router(state);

        let request = Request::builder()
            .uri("/api/briefing")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
