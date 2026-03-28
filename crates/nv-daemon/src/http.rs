use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::ws::{Message as WsMessage, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use nv_core::channel::Channel;
use nv_core::types::{CliCommand, CliRequest, CronEvent, ObligationOwner, ObligationStatus, Trigger};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::sync::Mutex as TokioMutex;

use crate::cc_sessions::CcSessionManager;
use crate::cold_start_store::ColdStartStore;
use crate::contact_store::ContactStore;
use crate::diary::DiaryWriter;
use crate::health::HealthState;
use crate::obligation_store::ObligationStore;
use crate::team_agent::TeamAgentDispatcher;
use crate::tools::jira::webhooks::{jira_webhook_handler, JiraWebhookState};
use crate::messages::MessageStore;
use crate::channels::teams::types::{ChangeNotificationCollection, ChatMessage};

// ── Activity Feed Types ───────────────────────────────────────────

/// A single obligation lifecycle event stored in the activity ring buffer.
#[derive(Debug, Clone, Serialize)]
pub struct ObligationActivityEvent {
    /// Unique event ID (UUID v4).
    pub id: String,
    /// Event type string, e.g. "obligation.detected", "obligation.execution_started".
    pub event_type: String,
    /// ID of the obligation this event relates to.
    pub obligation_id: String,
    /// Human-readable description for the activity feed.
    pub description: String,
    /// When the event occurred.
    pub timestamp: DateTime<Utc>,
    /// Optional structured metadata (tool name, duration, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// In-memory ring buffer for the last N obligation activity events.
///
/// Capacity is fixed at 200 events. When full, the oldest event is evicted
/// to make room for the newest. Thread-safe via `Arc<Mutex<_>>`.
#[derive(Clone)]
pub struct ActivityRingBuffer {
    inner: Arc<std::sync::Mutex<VecDeque<ObligationActivityEvent>>>,
    capacity: usize,
}

impl ActivityRingBuffer {
    /// Create a new ring buffer with capacity 200.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(std::sync::Mutex::new(VecDeque::with_capacity(200))),
            capacity: 200,
        }
    }

    /// Push a new event, evicting the oldest if the buffer is full.
    pub fn push(&self, event: ObligationActivityEvent) {
        if let Ok(mut buf) = self.inner.lock() {
            if buf.len() >= self.capacity {
                buf.pop_front();
            }
            buf.push_back(event);
        }
    }

    /// Return the most recent `limit` events in newest-first order.
    pub fn recent(&self, limit: usize) -> Vec<ObligationActivityEvent> {
        if let Ok(buf) = self.inner.lock() {
            buf.iter().rev().take(limit).cloned().collect()
        } else {
            vec![]
        }
    }
}

impl Default for ActivityRingBuffer {
    fn default() -> Self {
        Self::new()
    }
}

// ── Dashboard Event Types ─────────────────────────────────────────

/// Event broadcast over the `/ws/events` WebSocket endpoint.
///
/// Each variant corresponds to a daemon lifecycle event the dashboard
/// subscribes to for real-time updates. Variants not yet wired to a
/// producer are intentionally stubbed so the dashboard client can subscribe
/// without the daemon emitting them yet.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(dead_code)]
pub enum DaemonEvent {
    /// A message was logged (inbound or outbound).
    Message {
        id: i64,
        channel: String,
        direction: String,
        sender: String,
        preview: String,
        timestamp: String,
    },
    /// An obligation/approval changed status.
    ApprovalUpdated {
        id: String,
        status: String,
        owner: String,
    },
    /// A new obligation was created (triggers badge updates).
    ApprovalCreated {
        id: String,
        detected_action: String,
        priority: i32,
        owner: String,
    },
    /// A periodic health ping so the client can detect stale connections.
    HealthPing {
        timestamp: String,
    },
    /// An obligation activity event for the real-time feed.
    ObligationActivity(ObligationActivityEvent),
}

// ── Tool Dispatch State ───────────────────────────────────────────

/// Dependencies required to execute Nova tools via POST /api/tool-call.
///
/// Held behind `Arc` in `HttpState` because these fields are not `Clone`.
/// `None` when the daemon does not have tool dispatch configured (e.g. in
/// unit tests that don't need the tool-call endpoint).
pub struct ToolDispatch {
    pub memory: crate::memory::Memory,
    pub jira_registry: Option<crate::tools::jira::JiraRegistry>,
    pub channels: HashMap<String, Arc<dyn Channel>>,
    pub project_registry: HashMap<String, PathBuf>,
    pub calendar_credentials: Option<String>,
    pub calendar_id: String,
    pub stripe_registry: Option<crate::tools::ServiceRegistry<crate::tools::stripe::StripeClient>>,
    pub vercel_registry: Option<crate::tools::ServiceRegistry<crate::tools::vercel::VercelClient>>,
    pub sentry_registry: Option<crate::tools::ServiceRegistry<crate::tools::sentry::SentryClient>>,
    pub resend_registry: Option<crate::tools::ServiceRegistry<crate::tools::resend::ResendClient>>,
    pub ha_registry: Option<crate::tools::ServiceRegistry<crate::tools::ha::HAClient>>,
    pub upstash_registry: Option<crate::tools::ServiceRegistry<crate::tools::upstash::UpstashClient>>,
    pub ado_registry: Option<crate::tools::ServiceRegistry<crate::tools::ado::AdoClient>>,
    pub cloudflare_registry: Option<crate::tools::ServiceRegistry<crate::tools::cloudflare::CloudflareClient>>,
    pub doppler_registry: Option<crate::tools::ServiceRegistry<crate::tools::doppler::DopplerClient>>,
    pub teams_client: Option<Arc<crate::channels::teams::client::TeamsClient>>,
}

/// Request body for POST /api/tool-call.
#[derive(Debug, Deserialize)]
pub struct ToolCallRequest {
    pub tool_name: String,
    pub input: serde_json::Value,
}

/// Response body for POST /api/tool-call.
#[derive(Debug, Serialize, Deserialize)]
pub struct ToolCallResponse {
    pub result: Option<String>,
    pub error: Option<String>,
}

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
    /// Cold-start timing event store. None if not initialised.
    pub cold_start_store: Option<Arc<std::sync::Mutex<ColdStartStore>>>,
    /// Broadcast channel for dashboard WebSocket events (`/ws/events`).
    ///
    /// Handlers that produce events (approvals, messages, health changes) send
    /// on this sender; the WebSocket upgrade handler subscribes each new client
    /// to a receiver. Uses `broadcast` so multiple concurrent dashboard tabs
    /// all receive the same events.
    pub event_tx: broadcast::Sender<DaemonEvent>,
    /// CC session manager for the /api/cc-sessions endpoint.
    pub cc_session_manager: Option<CcSessionManager>,
    /// Contact store for /api/contacts CRUD.
    pub contact_store: Option<Arc<ContactStore>>,
    /// Diary writer for reading daily diary files via GET /api/diary.
    pub diary: Option<Arc<std::sync::Mutex<DiaryWriter>>>,
    /// TeamAgentDispatcher for GET /api/sessions. None if team_agents not configured.
    pub dispatcher: Option<TeamAgentDispatcher>,
    /// Project code to filesystem path registry (from config.projects).
    /// Used by GET /api/projects.
    pub project_registry: HashMap<String, PathBuf>,
    /// Path to the daemon config file (nv.toml). Used by GET /api/config.
    pub config_path: Option<PathBuf>,
    /// Base path of the memory directory (`~/.nv/memory`). Used by GET/PUT /api/memory.
    pub memory_base_path: Option<PathBuf>,
    /// In-memory ring buffer for obligation activity events (GET /api/obligations/activity).
    pub activity_buffer: ActivityRingBuffer,
    /// Tool dispatch dependencies for POST /api/tool-call.
    ///
    /// `None` in unit tests and when the daemon is in a minimal config mode.
    pub tool_dispatch: Option<Arc<ToolDispatch>>,
    /// Postgres-backed contact store for dual-write migration.
    pub pg_contact_store: Option<crate::pg_contact_store::PgContactStore>,
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
        .route("/api/cold-starts", get(get_cold_starts_handler))
        .route("/api/latency", get(get_latency_handler))
        // Dashboard API
        .route("/api/messages", get(get_messages_handler))
        .route("/api/obligations", get(get_obligations_handler))
        .route("/api/obligations/activity", get(get_obligation_activity_handler))
        .route("/api/obligations/stats", get(get_obligation_stats_handler))
        .route("/api/obligations/{id}", patch(patch_obligation_handler))
        .route("/api/projects", get(get_projects_handler))
        .route("/api/config", get(get_config_handler).put(put_config_handler))
        .route("/api/memory", get(get_memory_handler).put(put_memory_handler))
        .route("/api/solve", post(post_solve_handler))
        .route("/api/approvals/{id}/approve", post(approve_obligation_handler))
        .route("/api/cc-sessions", get(get_cc_sessions_handler))
        // Contact discovery (must be before /api/contacts/{id} to avoid path conflicts)
        .route("/api/contacts/discovered", get(discovered_contacts_handler))
        .route("/api/contacts/relationships", get(relationships_handler))
        // Contact CRUD
        .route("/api/contacts", get(list_contacts_handler).post(create_contact_handler))
        .route(
            "/api/contacts/{id}",
            get(get_contact_handler)
                .put(update_contact_handler)
                .delete(delete_contact_handler),
        )
        // WebSocket event stream
        .route("/ws/events", get(ws_events_handler))
        // Diary
        .route("/api/diary", get(get_diary_handler))
        // Sessions (TeamAgentDispatcher)
        .route("/api/sessions", get(get_sessions_handler))
        // MCP tool bridge — local-only
        .route("/api/tool-call", post(tool_call_handler));

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

// ── GET /api/messages ─────────────────────────────────────────────

/// Query parameters for `GET /api/messages`.
#[derive(Debug, Deserialize)]
pub struct MessagesQuery {
    /// Rows per page (1–200, default 50).
    pub limit: Option<i64>,
    /// Row offset for pagination (default 0).
    pub offset: Option<i64>,
    /// Optional channel filter (e.g. "telegram", "discord").
    pub channel: Option<String>,
    /// Optional full-text search query.
    pub search: Option<String>,
}

/// Response body for `GET /api/messages`.
#[derive(Debug, Serialize)]
pub struct MessagesResponse {
    pub messages: Vec<crate::messages::StoredMessage>,
    pub limit: i64,
    pub offset: i64,
}

/// GET /api/messages — paginated message history for the dashboard.
///
/// Supports optional `channel` and `search` filters. Results are ordered
/// newest-first. Default limit is 50; max is 200.
async fn get_messages_handler(
    State(state): State<Arc<HttpState>>,
    Query(query): Query<MessagesQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let offset = query.offset.unwrap_or(0).max(0);

    match MessageStore::init(&state.stats_db_path) {
        Ok(store) => {
            match store.paginate(
                limit,
                offset,
                query.channel.as_deref(),
                query.search.as_deref(),
            ) {
                Ok(messages) => (
                    StatusCode::OK,
                    Json(serde_json::to_value(MessagesResponse {
                        messages,
                        limit,
                        offset,
                    })
                    .unwrap()),
                )
                    .into_response(),
                Err(e) => {
                    tracing::error!(error = %e, "failed to paginate messages");
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"error": format!("Failed to query messages: {e}")})),
                    )
                        .into_response()
                }
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to open message store for /api/messages");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to open message store: {e}")})),
            )
                .into_response()
        }
    }
}

// ── POST /api/approvals/:id/approve ──────────────────────────────

// ── GET /api/obligations ──────────────────────────────────────────────

/// Query parameters for GET /api/obligations.
#[derive(Debug, Deserialize, Default)]
pub struct ObligationsQuery {
    pub status: Option<String>,
    pub owner: Option<String>,
}

/// GET /api/obligations — list obligations with optional status/owner filters.
///
/// Returns `{ obligations: [...] }`. Returns an empty list gracefully when the
/// obligation store is not initialised.
async fn get_obligations_handler(
    State(state): State<Arc<HttpState>>,
    Query(params): Query<ObligationsQuery>,
) -> impl IntoResponse {
    match ObligationStore::new(&state.stats_db_path) {
        Ok(store) => {
            let result = match (&params.status, &params.owner) {
                (Some(status_str), Some(owner_str)) => {
                    // Filter by both status and owner: fetch by owner then filter status in memory.
                    match owner_str.parse::<ObligationOwner>() {
                        Ok(owner) => store
                            .list_by_owner(&owner)
                            .map(|list| {
                                list.into_iter()
                                    .filter(|o| o.status.as_str() == status_str.as_str())
                                    .collect::<Vec<_>>()
                            }),
                        Err(_) => {
                            return (
                                StatusCode::BAD_REQUEST,
                                Json(serde_json::json!({"error": format!("unknown owner: {owner_str}")})),
                            )
                                .into_response();
                        }
                    }
                }
                (Some(status_str), None) => {
                    match status_str.parse::<ObligationStatus>() {
                        Ok(status) => store.list_by_status(&status),
                        Err(_) => {
                            return (
                                StatusCode::BAD_REQUEST,
                                Json(serde_json::json!({"error": format!("unknown status: {status_str}")})),
                            )
                                .into_response();
                        }
                    }
                }
                (None, Some(owner_str)) => {
                    match owner_str.parse::<ObligationOwner>() {
                        Ok(owner) => store.list_by_owner(&owner),
                        Err(_) => {
                            return (
                                StatusCode::BAD_REQUEST,
                                Json(serde_json::json!({"error": format!("unknown owner: {owner_str}")})),
                            )
                                .into_response();
                        }
                    }
                }
                (None, None) => store.list_all(),
            };

            match result {
                Ok(obligations) => {
                    // Enrich each obligation with notes, attempt_count, last_attempt_at.
                    let enriched: Vec<serde_json::Value> = obligations
                        .into_iter()
                        .map(|ob| {
                            let notes = store.list_notes(&ob.id).unwrap_or_default();
                            let attempt_count = store
                                .count_execution_attempts(&ob.id)
                                .unwrap_or(0) as u32;
                            let last_attempt_at = ob.last_attempt_at.clone();
                            let mut val = serde_json::to_value(&ob).unwrap_or_default();
                            if let serde_json::Value::Object(ref mut map) = val {
                                map.insert(
                                    "notes".to_string(),
                                    serde_json::to_value(&notes).unwrap_or_default(),
                                );
                                map.insert(
                                    "attempt_count".to_string(),
                                    serde_json::Value::Number(attempt_count.into()),
                                );
                                map.insert(
                                    "last_attempt_at".to_string(),
                                    last_attempt_at
                                        .map(serde_json::Value::String)
                                        .unwrap_or(serde_json::Value::Null),
                                );
                            }
                            val
                        })
                        .collect();
                    (
                        StatusCode::OK,
                        Json(serde_json::json!({ "obligations": enriched })),
                    )
                        .into_response()
                }
                Err(e) => {
                    tracing::error!(error = %e, "get_obligations: query failed");
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"error": format!("Query failed: {e}")})),
                    )
                        .into_response()
                }
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, "get_obligations: obligation store not available");
            // Graceful degradation: return empty list so dashboard doesn't break
            (
                StatusCode::OK,
                Json(serde_json::json!({ "obligations": [] })),
            )
                .into_response()
        }
    }
}

// ── GET /api/obligations/activity ─────────────────────────────────────

/// Query params for GET /api/obligations/activity.
#[derive(Debug, Deserialize, Default)]
pub struct ObligationActivityQuery {
    /// Maximum number of events to return (default 50, max 200).
    pub limit: Option<usize>,
}

/// GET /api/obligations/activity?limit=50
///
/// Returns the most recent obligation activity events from the in-memory
/// ring buffer, newest first.
async fn get_obligation_activity_handler(
    State(state): State<Arc<HttpState>>,
    Query(params): Query<ObligationActivityQuery>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(50).min(200);
    let events = state.activity_buffer.recent(limit);
    (
        StatusCode::OK,
        Json(serde_json::json!({ "events": events })),
    )
}

// ── GET /api/obligations/stats ─────────────────────────────────────────

/// GET /api/obligations/stats — aggregate counts by status and owner.
async fn get_obligation_stats_handler(
    State(state): State<Arc<HttpState>>,
) -> impl IntoResponse {
    match ObligationStore::new(&state.stats_db_path) {
        Ok(store) => match store.get_stats() {
            Ok(stats) => (
                StatusCode::OK,
                Json(serde_json::to_value(stats).unwrap_or_default()),
            )
                .into_response(),
            Err(e) => {
                tracing::error!(error = %e, "get_obligation_stats: query failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": format!("Stats query failed: {e}")})),
                )
                    .into_response()
            }
        },
        Err(e) => {
            tracing::warn!(error = %e, "get_obligation_stats: store not available");
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "open_nova": 0,
                    "open_leo": 0,
                    "in_progress": 0,
                    "proposed_done": 0,
                    "done_today": 0,
                })),
            )
                .into_response()
        }
    }
}

// ── PATCH /api/obligations/:id ────────────────────────────────────────

/// Request body for PATCH /api/obligations/:id.
#[derive(Debug, Deserialize)]
pub struct PatchObligationRequest {
    pub status: String,
}

/// PATCH /api/obligations/:id — update obligation status.
///
/// Accepts `{ "status": "dismissed" | "open" | "in_progress" | "done" }`,
/// updates the obligation, broadcasts `ApprovalUpdated`, returns `{ id, status }`.
async fn patch_obligation_handler(
    State(state): State<Arc<HttpState>>,
    Path(id): Path<String>,
    Json(body): Json<PatchObligationRequest>,
) -> impl IntoResponse {
    let new_status = match body.status.parse::<ObligationStatus>() {
        Ok(s) => s,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("unknown status: {}", body.status)})),
            )
                .into_response();
        }
    };

    match ObligationStore::new(&state.stats_db_path) {
        Ok(store) => {
            match store.get_by_id(&id) {
                Ok(None) => {
                    return (
                        StatusCode::NOT_FOUND,
                        Json(serde_json::json!({"error": format!("obligation {id} not found")})),
                    )
                        .into_response();
                }
                Err(e) => {
                    tracing::error!(id = %id, error = %e, "patch_obligation: get_by_id failed");
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"error": format!("Failed to fetch obligation: {e}")})),
                    )
                        .into_response();
                }
                Ok(Some(_)) => {}
            }

            match store.update_status(&id, &new_status) {
                Ok(true) => {
                    let _ = state.event_tx.send(DaemonEvent::ApprovalUpdated {
                        id: id.clone(),
                        status: new_status.as_str().to_string(),
                        owner: String::new(),
                    });
                    tracing::info!(id = %id, status = %new_status, "obligation status updated via dashboard");
                    (
                        StatusCode::OK,
                        Json(serde_json::json!({ "id": id, "status": new_status.as_str() })),
                    )
                        .into_response()
                }
                Ok(false) => (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({"error": format!("obligation {id} not found")})),
                )
                    .into_response(),
                Err(e) => {
                    tracing::error!(id = %id, error = %e, "patch_obligation: update_status failed");
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"error": format!("Failed to update obligation: {e}")})),
                    )
                        .into_response()
                }
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "patch_obligation: failed to open obligation store");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to open obligation store: {e}")})),
            )
                .into_response()
        }
    }
}

// ── GET /api/projects ─────────────────────────────────────────────────

/// GET /api/projects — return the configured project registry.
///
/// Returns `{ projects: [{ code, path }] }`. The list is derived from the
/// `project_registry` field in `HttpState` (populated from `config.projects`).
async fn get_projects_handler(State(state): State<Arc<HttpState>>) -> impl IntoResponse {
    let projects: Vec<serde_json::Value> = state
        .project_registry
        .iter()
        .map(|(code, path)| {
            serde_json::json!({
                "code": code,
                "path": path.to_string_lossy(),
            })
        })
        .collect();

    (
        StatusCode::OK,
        Json(serde_json::json!({ "projects": projects })),
    )
}

// ── GET /api/config + PUT /api/config ────────────────────────────────

/// Secret key patterns — values whose keys match any of these are masked.
const SECRET_KEY_PATTERNS: &[&str] = &[
    "token", "secret", "password", "key", "api_key", "auth", "webhook",
];

fn is_secret_key(key: &str) -> bool {
    let lower = key.to_lowercase();
    SECRET_KEY_PATTERNS.iter().any(|p| lower.contains(p))
}

/// Recursively mask secret values in a JSON object.
fn mask_secrets(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let masked = map
                .into_iter()
                .map(|(k, v)| {
                    let new_v = if is_secret_key(&k) && v.is_string() && !v.as_str().unwrap_or("").is_empty() {
                        serde_json::Value::String("***".to_string())
                    } else {
                        mask_secrets(v)
                    };
                    (k, new_v)
                })
                .collect();
            serde_json::Value::Object(masked)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.into_iter().map(mask_secrets).collect())
        }
        other => other,
    }
}

/// GET /api/config — return daemon config from disk as JSON, masking secrets.
///
/// Returns `{}` (HTTP 200) when no config file exists, so the dashboard
/// never gets a 502 error just because the config is absent.
async fn get_config_handler(State(state): State<Arc<HttpState>>) -> impl IntoResponse {
    let config_path = match &state.config_path {
        Some(p) => p.clone(),
        None => match nv_core::config::Config::default_path() {
            Ok(p) => p,
            Err(_) => {
                return (StatusCode::OK, Json(serde_json::json!({}))).into_response();
            }
        },
    };

    let contents = match std::fs::read_to_string(&config_path) {
        Ok(c) => c,
        Err(_) => {
            // Config file missing — return empty object gracefully (HTTP 200).
            return (StatusCode::OK, Json(serde_json::json!({}))).into_response();
        }
    };

    let raw: serde_json::Value = match toml::from_str(&contents) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, "get_config: failed to parse config toml");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to parse config: {e}")})),
            )
                .into_response();
        }
    };

    let masked = mask_secrets(raw);
    (StatusCode::OK, Json(masked)).into_response()
}

/// Request body for PUT /api/config.
#[derive(Debug, Deserialize)]
pub struct PutConfigRequest {
    pub fields: serde_json::Value,
}

/// PUT /api/config — merge fields into the daemon config on disk.
///
/// Reads the existing config, merges in the provided fields (top-level merge),
/// writes back to disk. Returns `{ applied: [keys], note: "..." }`.
async fn put_config_handler(
    State(state): State<Arc<HttpState>>,
    Json(body): Json<PutConfigRequest>,
) -> impl IntoResponse {
    let config_path = match &state.config_path {
        Some(p) => p.clone(),
        None => match nv_core::config::Config::default_path() {
            Ok(p) => p,
            Err(_) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": "Cannot determine config path"})),
                )
                    .into_response();
            }
        },
    };

    // Read existing config (or start fresh if missing)
    let existing_toml = std::fs::read_to_string(&config_path).unwrap_or_default();
    let mut config_map: toml::Value = if existing_toml.is_empty() {
        toml::Value::Table(toml::map::Map::new())
    } else {
        match toml::from_str(&existing_toml) {
            Ok(v) => v,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": format!("Failed to parse existing config: {e}")})),
                )
                    .into_response();
            }
        }
    };

    // Merge top-level fields from the request body
    let mut applied: Vec<String> = Vec::new();
    if let (serde_json::Value::Object(fields), toml::Value::Table(ref mut table)) =
        (&body.fields, &mut config_map)
    {
        for (key, value) in fields {
            // Convert JSON value to TOML value
            let toml_val: toml::Value = match serde_json::to_string(value)
                .ok()
                .and_then(|s| toml::from_str(&format!("x = {s}")).ok())
                .and_then(|mut t: toml::value::Table| t.remove("x"))
            {
                Some(v) => v,
                None => continue,
            };
            table.insert(key.clone(), toml_val);
            applied.push(key.clone());
        }
    }

    // Write back
    let new_toml = match toml::to_string_pretty(&config_map) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to serialize config: {e}")})),
            )
                .into_response();
        }
    };

    if let Err(e) = std::fs::write(&config_path, new_toml) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to write config: {e}")})),
        )
            .into_response();
    }

    tracing::info!(keys = ?applied, "config updated via dashboard PUT /api/config");

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "applied": applied,
            "note": "Config updated. Daemon restart may be required for some changes.",
        })),
    )
        .into_response()
}

// ── GET /api/memory + PUT /api/memory ────────────────────────────────

/// Query parameters for GET /api/memory.
#[derive(Debug, Deserialize, Default)]
pub struct MemoryQuery {
    pub topic: Option<String>,
}

/// Request body for PUT /api/memory.
#[derive(Debug, Deserialize)]
pub struct PutMemoryRequest {
    pub topic: String,
    pub content: String,
}

/// GET /api/memory — list topics or read a specific topic.
///
/// Without `?topic=`: returns `{ "topics": ["<name>", ...] }`.
/// With `?topic=<name>`: returns `{ "topic": "<name>", "content": "<text>" }`.
/// Returns 404 if the requested topic does not exist.
/// Returns 503 if the memory path is not configured.
async fn get_memory_handler(
    State(state): State<Arc<HttpState>>,
    Query(query): Query<MemoryQuery>,
) -> impl IntoResponse {
    let base_path = match &state.memory_base_path {
        Some(p) => p.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "Memory not configured"})),
            )
                .into_response();
        }
    };

    let memory = crate::memory::Memory::from_base_path(base_path);

    match query.topic {
        Some(topic) => {
            // Check if the topic file exists before reading.
            let filename = topic
                .chars()
                .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
                .collect::<String>()
                .to_lowercase();
            let topic_path = memory.base_path.join(format!("{filename}.md"));
            if !topic_path.exists() {
                return (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({"error": "Topic not found"})),
                )
                    .into_response();
            }
            match memory.read(&topic) {
                Ok(content) => (
                    StatusCode::OK,
                    Json(serde_json::json!({"topic": topic, "content": content})),
                )
                    .into_response(),
                Err(e) => {
                    tracing::warn!(topic = %topic, error = %e, "get_memory: read failed");
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"error": format!("Failed to read topic: {e}")})),
                    )
                        .into_response()
                }
            }
        }
        None => match memory.list_topics() {
            Ok(topics) => (
                StatusCode::OK,
                Json(serde_json::json!({"topics": topics})),
            )
                .into_response(),
            Err(e) => {
                tracing::warn!(error = %e, "get_memory: list_topics failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": format!("Failed to list topics: {e}")})),
                )
                    .into_response()
            }
        },
    }
}

/// PUT /api/memory — write content to a memory topic.
///
/// Accepts `{ "topic": "<name>", "content": "<text>" }`.
/// Returns `{ "topic": "<name>", "written": <byte_count> }` on success.
/// Returns 503 if the memory path is not configured.
async fn put_memory_handler(
    State(state): State<Arc<HttpState>>,
    Json(body): Json<PutMemoryRequest>,
) -> impl IntoResponse {
    let base_path = match &state.memory_base_path {
        Some(p) => p.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "Memory not configured"})),
            )
                .into_response();
        }
    };

    let memory = crate::memory::Memory::from_base_path(base_path);
    let byte_count = body.content.len();

    match memory.write(&body.topic, &body.content) {
        Ok(_) => {
            tracing::info!(topic = %body.topic, bytes = byte_count, "memory topic written via PUT /api/memory");
            (
                StatusCode::OK,
                Json(serde_json::json!({"topic": body.topic, "written": byte_count})),
            )
                .into_response()
        }
        Err(e) => {
            tracing::warn!(topic = %body.topic, error = %e, "put_memory: write failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to write topic: {e}")})),
            )
                .into_response()
        }
    }
}

// ── POST /api/solve ───────────────────────────────────────────────────

/// Request body for POST /api/solve.
#[derive(Debug, Deserialize)]
pub struct SolveRequest {
    pub project: String,
    pub error: String,
    pub context: Option<String>,
}

/// POST /api/solve — start a solve session for a project error.
///
/// Accepts `{ "project": "<code>", "error": "<message>", "context": "<optional>" }`.
/// Returns `{ "session_id": "<uuid>" }` immediately.
/// Full session wiring (Claude Code invocation) is out of scope for this iteration.
async fn post_solve_handler(
    State(_state): State<Arc<HttpState>>,
    Json(body): Json<SolveRequest>,
) -> impl IntoResponse {
    let session_id = uuid::Uuid::new_v4().to_string();
    tracing::info!(
        project = %body.project,
        session_id = %session_id,
        "solve session initiated via POST /api/solve"
    );
    (
        StatusCode::OK,
        Json(serde_json::json!({"session_id": session_id})),
    )
}

/// Response body for `POST /api/approvals/:id/approve`.
#[derive(Debug, Serialize)]
pub struct ApproveResponse {
    pub id: String,
    pub status: String,
}

/// POST /api/approvals/:id/approve — mark an obligation as approved (done).
///
/// Opens the obligation store, transitions status from any state to `done`,
/// and broadcasts an `ApprovalUpdated` WebSocket event.
async fn approve_obligation_handler(
    State(state): State<Arc<HttpState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match ObligationStore::new(&state.stats_db_path) {
        Ok(store) => {
            // Verify the obligation exists before updating.
            match store.get_by_id(&id) {
                Ok(None) => {
                    return (
                        StatusCode::NOT_FOUND,
                        Json(serde_json::json!({"error": format!("obligation {id} not found")})),
                    )
                        .into_response();
                }
                Err(e) => {
                    tracing::error!(id = %id, error = %e, "approve: get_by_id failed");
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"error": format!("Failed to fetch obligation: {e}")})),
                    )
                        .into_response();
                }
                Ok(Some(_)) => {}
            }

            match store.update_status(&id, &ObligationStatus::Done) {
                Ok(true) => {
                    // Broadcast the update — ignore send error (no subscribers is fine).
                    let _ = state.event_tx.send(DaemonEvent::ApprovalUpdated {
                        id: id.clone(),
                        status: "done".to_string(),
                        owner: String::new(), // owner unchanged
                    });

                    tracing::info!(id = %id, "obligation approved via dashboard");

                    (
                        StatusCode::OK,
                        Json(
                            serde_json::to_value(ApproveResponse {
                                id,
                                status: "approved".to_string(),
                            })
                            .unwrap(),
                        ),
                    )
                        .into_response()
                }
                Ok(false) => (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({"error": format!("obligation {id} not found")})),
                )
                    .into_response(),
                Err(e) => {
                    tracing::error!(id = %id, error = %e, "approve: update_status failed");
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"error": format!("Failed to approve obligation: {e}")})),
                    )
                        .into_response()
                }
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "approve: failed to open obligation store");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to open obligation store: {e}")})),
            )
                .into_response()
        }
    }
}

// ── GET /api/latency ──────────────────────────────────────────────

/// Per-stage latency percentile entry in the `/api/latency` response.
#[derive(Debug, Serialize)]
pub struct StageLatency {
    pub stage: String,
    pub p50_ms: Option<f64>,
    pub p95_ms: Option<f64>,
    pub window: String,
}

/// Response body for `GET /api/latency`.
#[derive(Debug, Serialize)]
pub struct LatencyResponse {
    pub stages: Vec<StageLatency>,
}

/// GET /api/latency — returns P50 and P95 latency per pipeline stage for the
/// last 24h and 7d windows.
///
/// Stages reported: `receive`, `context_build`, `api_call`, `tool_loop`, `delivery`.
/// Returns 200 with an empty `stages` array when no spans exist yet.
async fn get_latency_handler(
    State(state): State<Arc<HttpState>>,
) -> impl IntoResponse {
    match MessageStore::init(&state.stats_db_path) {
        Ok(store) => {
            const STAGES: &[&str] = &[
                "receive",
                "context_build",
                "api_call",
                "tool_loop",
                "delivery",
            ];
            let mut entries = Vec::new();
            for &stage in STAGES {
                // 24h window
                entries.push(StageLatency {
                    stage: stage.to_string(),
                    p50_ms: store.latency_p50(stage, 24),
                    p95_ms: store.latency_p95(stage, 24),
                    window: "24h".to_string(),
                });
                // 7d window
                entries.push(StageLatency {
                    stage: stage.to_string(),
                    p50_ms: store.latency_p50(stage, 168),
                    p95_ms: store.latency_p95(stage, 168),
                    window: "7d".to_string(),
                });
            }
            (StatusCode::OK, Json(LatencyResponse { stages: entries })).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to open message store for /api/latency");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to open message store: {e}")})),
            )
                .into_response()
        }
    }
}

// ── GET /ws/events ────────────────────────────────────────────────

/// GET /ws/events — upgrade to WebSocket and stream daemon events.
///
/// Each connected client receives a copy of every `DaemonEvent` broadcast
/// on the shared `event_tx` channel. The connection is closed gracefully
/// when the client disconnects or when the broadcast channel is dropped.
async fn ws_events_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<HttpState>>,
) -> impl IntoResponse {
    let rx = state.event_tx.subscribe();
    ws.on_upgrade(move |socket| handle_ws_events(socket, rx))
}

/// Drive a single WebSocket client: forward broadcast events as JSON text frames.
async fn handle_ws_events(
    mut socket: WebSocket,
    mut rx: broadcast::Receiver<DaemonEvent>,
) {
    loop {
        tokio::select! {
            // Forward the next daemon event to the client.
            event = rx.recv() => {
                match event {
                    Ok(ev) => {
                        let json = match serde_json::to_string(&ev) {
                            Ok(j) => j,
                            Err(e) => {
                                tracing::warn!(error = %e, "ws: failed to serialize event");
                                continue;
                            }
                        };
                        if socket.send(WsMessage::Text(json.into())).await.is_err() {
                            // Client disconnected.
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(skipped = n, "ws/events: client lagged, skipped events");
                        // Continue — don't disconnect lagged clients.
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        // Sender dropped — daemon shutting down.
                        break;
                    }
                }
            }
            // Client sent a frame — consume it to detect close.
            msg = socket.recv() => {
                match msg {
                    None | Some(Err(_)) => break, // disconnected or error
                    Some(Ok(WsMessage::Close(_))) => break,
                    Some(Ok(_)) => {} // ping/pong/text from client — ignore
                }
            }
        }
    }

    tracing::debug!("ws/events: client disconnected");
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
    cold_start_store: Option<Arc<std::sync::Mutex<ColdStartStore>>>,
    event_tx: broadcast::Sender<DaemonEvent>,
    cc_session_manager: Option<CcSessionManager>,
    contact_store: Option<Arc<ContactStore>>,
    diary: Option<Arc<std::sync::Mutex<DiaryWriter>>>,
    dispatcher: Option<TeamAgentDispatcher>,
    project_registry: HashMap<String, PathBuf>,
    config_path: Option<PathBuf>,
    memory_base_path: Option<PathBuf>,
    activity_buffer: ActivityRingBuffer,
    tool_dispatch: Option<Arc<ToolDispatch>>,
    pg_contact_store: Option<crate::pg_contact_store::PgContactStore>,
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
        cold_start_store,
        event_tx,
        cc_session_manager,
        contact_store,
        diary,
        dispatcher,
        project_registry,
        config_path,
        memory_base_path,
        activity_buffer,
        tool_dispatch,
        pg_contact_store,
    });
    let app = build_router(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    tracing::info!(port, "HTTP server listening");

    // Use into_make_service_with_connect_info so handlers can extract peer
    // addresses (needed by tool_call_handler's localhost-only guard).
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;
    Ok(())
}

// ── CC Sessions API ──────────────────────────────────────────────────

/// GET /api/cc-sessions — list all CC sessions managed by CcSessionManager.
///
/// Returns a JSON array of `CcSessionSummary` objects. Returns an empty array
/// when the manager is not configured.
async fn get_cc_sessions_handler(State(state): State<Arc<HttpState>>) -> impl IntoResponse {
    let Some(ref mgr) = state.cc_session_manager else {
        return (
            StatusCode::OK,
            Json(serde_json::json!({ "sessions": [], "configured": false })),
        );
    };

    let sessions = mgr.list().await;
    (
        StatusCode::OK,
        Json(serde_json::json!({ "sessions": sessions, "configured": true })),
    )
}

// ── Contact Discovery API ────────────────────────────────────────────

/// Query parameters for GET /api/contacts/relationships.
#[derive(Debug, Deserialize, Default)]
pub struct RelationshipsQuery {
    /// Minimum co-occurrence count to include an edge (default 3).
    pub min_count: Option<i64>,
}

/// GET /api/contacts/discovered — auto-discover contacts from message history.
async fn discovered_contacts_handler(
    State(state): State<Arc<HttpState>>,
) -> impl IntoResponse {
    match MessageStore::init(&state.stats_db_path) {
        Ok(store) => match store.discover_contacts() {
            Ok(response) => (
                StatusCode::OK,
                Json(serde_json::to_value(response).unwrap_or_default()),
            ).into_response(),
            Err(e) => {
                let msg = format!("{e}");
                tracing::warn!(error = %msg, "discover_contacts failed");
                (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": msg }))).into_response()
            }
        },
        Err(e) => {
            let msg = format!("{e}");
            tracing::error!(error = %msg, "failed to open message store");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": msg }))).into_response()
        }
    }
}

/// GET /api/contacts/relationships — co-occurrence relationship edges.
async fn relationships_handler(
    State(state): State<Arc<HttpState>>,
    Query(query): Query<RelationshipsQuery>,
) -> impl IntoResponse {
    let min_count = query.min_count.unwrap_or(3);
    match MessageStore::init(&state.stats_db_path) {
        Ok(store) => match store.discover_relationships(min_count) {
            Ok(response) => (
                StatusCode::OK,
                Json(serde_json::to_value(response).unwrap_or_default()),
            ).into_response(),
            Err(e) => {
                let msg = format!("{e}");
                tracing::warn!(error = %msg, "discover_relationships failed");
                (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": msg }))).into_response()
            }
        },
        Err(e) => {
            let msg = format!("{e}");
            tracing::error!(error = %msg, "failed to open message store");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": msg }))).into_response()
        }
    }
}

// ── Contact CRUD API ────────────────────────────────────────────────

/// Query params for GET /api/contacts.
#[derive(Debug, Deserialize, Default)]
pub struct ContactsQuery {
    /// Filter by relationship_type (e.g. `?relationship=work`).
    pub relationship: Option<String>,
    /// Full-text search on name and notes (e.g. `?q=leo`).
    pub q: Option<String>,
}

/// Request body for POST /api/contacts.
#[derive(Debug, Deserialize)]
pub struct CreateContactRequest {
    pub name: String,
    #[serde(default)]
    pub channel_ids: serde_json::Value,
    #[serde(default = "default_relationship")]
    pub relationship_type: String,
    pub notes: Option<String>,
}

fn default_relationship() -> String {
    "social".to_string()
}

/// Request body for PUT /api/contacts/{id}.
#[derive(Debug, Deserialize)]
pub struct UpdateContactRequest {
    pub name: Option<String>,
    pub channel_ids: Option<serde_json::Value>,
    pub relationship_type: Option<String>,
    pub notes: Option<String>,
}

/// GET /api/contacts — list or search contacts.
async fn list_contacts_handler(
    State(state): State<Arc<HttpState>>,
    Query(query): Query<ContactsQuery>,
) -> impl IntoResponse {
    let Some(ref store) = state.contact_store else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({ "error": "contact store not configured" }))).into_response();
    };

    let result = if let Some(ref q) = query.q {
        store.search(q)
    } else {
        store.list(query.relationship.as_deref())
    };

    match result {
        Ok(contacts) => (StatusCode::OK, Json(serde_json::to_value(contacts).unwrap_or_default())).into_response(),
        Err(e) => {
            let msg = format!("{e}");
            tracing::warn!(error = %msg, "contacts list failed");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": msg }))).into_response()
        }
    }
}

/// POST /api/contacts — create a new contact.
async fn create_contact_handler(
    State(state): State<Arc<HttpState>>,
    Json(req): Json<CreateContactRequest>,
) -> impl IntoResponse {
    let Some(ref store) = state.contact_store else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({ "error": "contact store not configured" }))).into_response();
    };

    match store.create(&req.name, req.channel_ids.clone(), &req.relationship_type, req.notes.as_deref()) {
        Ok(contact) => {
            // Dual-write to Postgres (fire-and-forget).
            if let Some(pg_store) = state.pg_contact_store.clone() {
                let name = req.name.clone();
                let channel_ids = req.channel_ids.clone();
                let rt = req.relationship_type.clone();
                let notes = req.notes.clone();
                tokio::spawn(async move {
                    if let Err(e) = pg_store
                        .create(&name, &channel_ids, &rt, notes.as_deref())
                        .await
                    {
                        tracing::warn!(error = %e, "pg dual-write: contact create failed");
                    }
                });
            }
            (StatusCode::CREATED, Json(serde_json::to_value(contact).unwrap_or_default())).into_response()
        }
        Err(e) => {
            let msg = format!("{e}");
            tracing::warn!(error = %msg, "contact create failed");
            (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": msg }))).into_response()
        }
    }
}

/// GET /api/contacts/{id} — fetch a single contact.
async fn get_contact_handler(
    State(state): State<Arc<HttpState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Some(ref store) = state.contact_store else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({ "error": "contact store not configured" }))).into_response();
    };

    match store.get(&id) {
        Ok(Some(contact)) => (StatusCode::OK, Json(serde_json::to_value(contact).unwrap_or_default())).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "not found" }))).into_response(),
        Err(e) => {
            let msg = format!("{e}");
            tracing::warn!(error = %msg, "contact get failed");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": msg }))).into_response()
        }
    }
}

/// PUT /api/contacts/{id} — update an existing contact.
async fn update_contact_handler(
    State(state): State<Arc<HttpState>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateContactRequest>,
) -> impl IntoResponse {
    let Some(ref store) = state.contact_store else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({ "error": "contact store not configured" }))).into_response();
    };

    match store.update(&id, req.name.as_deref(), req.channel_ids.clone(), req.relationship_type.as_deref(), req.notes.as_deref()) {
        Ok(contact) => {
            // Dual-write to Postgres (fire-and-forget).
            if let Some(pg_store) = state.pg_contact_store.clone() {
                let id = id.clone();
                let name = req.name.clone();
                let channel_ids = req.channel_ids.clone();
                let rt = req.relationship_type.clone();
                let notes = req.notes.clone();
                tokio::spawn(async move {
                    if let Err(e) = pg_store
                        .update(&id, name.as_deref(), channel_ids.as_ref(), rt.as_deref(), notes.as_deref())
                        .await
                    {
                        tracing::warn!(error = %e, "pg dual-write: contact update failed");
                    }
                });
            }
            (StatusCode::OK, Json(serde_json::to_value(contact).unwrap_or_default())).into_response()
        }
        Err(e) => {
            let msg = format!("{e}");
            let status = if msg.contains("not found") { StatusCode::NOT_FOUND } else { StatusCode::BAD_REQUEST };
            (status, Json(serde_json::json!({ "error": msg }))).into_response()
        }
    }
}

/// DELETE /api/contacts/{id} — delete a contact.
async fn delete_contact_handler(
    State(state): State<Arc<HttpState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Some(ref store) = state.contact_store else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    match store.delete(&id) {
        Ok(true) => {
            // Dual-write to Postgres (fire-and-forget).
            if let Some(pg_store) = state.pg_contact_store.clone() {
                let id = id.clone();
                tokio::spawn(async move {
                    if let Err(e) = pg_store.delete(&id).await {
                        tracing::warn!(error = %e, "pg dual-write: contact delete failed");
                    }
                });
            }
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            tracing::warn!(error = %e, "contact delete failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

// ── Diary ────────────────────────────────────────────────────────────

/// Query parameters for GET /api/diary.
#[derive(Debug, Deserialize)]
pub struct DiaryQuery {
    /// Which day to read (YYYY-MM-DD). Defaults to today.
    pub date: Option<String>,
    /// Maximum entries to return. Defaults to 50.
    pub limit: Option<usize>,
}

/// A single diary entry returned by GET /api/diary.
#[derive(Debug, Serialize)]
pub struct DiaryEntryItem {
    pub time: String,
    pub trigger_type: String,
    pub trigger_source: String,
    pub channel_source: String,
    pub slug: String,
    pub tools_called: Vec<String>,
    pub result_summary: String,
    pub response_latency_ms: u64,
    pub tokens_in: u64,
    pub tokens_out: u64,
}

/// Response body for GET /api/diary.
#[derive(Debug, Serialize)]
pub struct DiaryGetResponse {
    pub date: String,
    pub entries: Vec<DiaryEntryItem>,
    pub total: usize,
}

/// Parse a raw diary markdown file into structured entries.
///
/// The fixed schema emitted by `format_entry` makes this straightforward:
/// each `## HH:MM — type (source) · slug` section contains labelled lines.
fn parse_diary_file(content: &str) -> Vec<DiaryEntryItem> {
    let mut entries = Vec::new();

    // Split on "## " to get sections (first element may be empty).
    let sections: Vec<&str> = content.split("\n## ").collect();

    for section in &sections {
        let trimmed = section.trim_start_matches("## ").trim();
        if trimmed.is_empty() {
            continue;
        }
        let lines: Vec<&str> = trimmed.lines().collect();
        let heading = match lines.first() {
            Some(h) => *h,
            None => continue,
        };

        // Parse heading: "HH:MM — trigger_type (trigger_source) · slug"
        let (time_part, after_dash) = heading
            .split_once(" — ")
            .map(|(t, a)| (t.trim().to_string(), a))
            .unwrap_or_else(|| (heading.trim().to_string(), ""));
        // trigger_source is in parentheses inside "type (source) · slug"
        let before_dot = after_dash.split(" \u{00B7} ").next().unwrap_or(after_dash);
        let slug = after_dash.split(" \u{00B7} ").last().unwrap_or("").trim().to_string();

        let (trigger_type, trigger_source) = before_dot
            .split_once(" (")
            .map(|(t, s)| {
                (
                    t.trim().to_string(),
                    s.trim_end_matches(')').trim().to_string(),
                )
            })
            .unwrap_or_else(|| (before_dot.trim().to_string(), String::new()));

        // Extract labelled fields.
        let get_field = |label: &str| -> String {
            lines
                .iter()
                .find(|l| l.starts_with(label))
                .map(|l| l[label.len()..].trim().to_string())
                .unwrap_or_default()
        };

        let channel_source = get_field("**Channel:** ");
        let tools_raw = get_field("**Tools called:** ");
        let tools_called: Vec<String> = if tools_raw.is_empty() || tools_raw == "none" {
            vec![]
        } else {
            tools_raw.split(", ").map(|s| s.trim().to_string()).collect()
        };
        let result_summary = get_field("**Result:** ");

        let latency_raw = get_field("**Latency:** ");
        let response_latency_ms: u64 = latency_raw
            .trim_end_matches("ms")
            .trim()
            .parse()
            .unwrap_or(0);

        let cost_raw = get_field("**Cost:** ");
        // Format: "N in + M out tokens"
        let mut tokens_in: u64 = 0;
        let mut tokens_out: u64 = 0;
        if let Some(in_part) = cost_raw.split(" in + ").next() {
            tokens_in = in_part.trim().parse().unwrap_or(0);
        }
        if let Some(out_part) = cost_raw.split(" in + ").nth(1) {
            tokens_out = out_part
                .split_whitespace()
                .next()
                .unwrap_or("0")
                .parse()
                .unwrap_or(0);
        }

        entries.push(DiaryEntryItem {
            time: time_part,
            trigger_type,
            trigger_source,
            channel_source,
            slug,
            tools_called,
            result_summary,
            response_latency_ms,
            tokens_in,
            tokens_out,
        });
    }

    entries
}

/// GET /api/diary — return diary entries for a given day.
async fn get_diary_handler(
    State(state): State<Arc<HttpState>>,
    Query(query): Query<DiaryQuery>,
) -> impl IntoResponse {
    let Some(ref diary_arc) = state.diary else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    let date_str = query.date.unwrap_or_else(|| {
        chrono::Local::now().format("%Y-%m-%d").to_string()
    });

    let limit = query.limit.unwrap_or(50);

    let file_path = {
        let diary = diary_arc.lock().unwrap();
        let date = match chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "error": "invalid date format, expected YYYY-MM-DD" })),
                )
                    .into_response();
            }
        };
        diary.daily_file_path(date)
    };

    let content = match std::fs::read_to_string(&file_path) {
        Ok(c) => c,
        Err(_) => {
            // File does not exist — return empty list, not an error.
            return Json(DiaryGetResponse {
                date: date_str,
                entries: vec![],
                total: 0,
            })
            .into_response();
        }
    };

    let mut entries = parse_diary_file(&content);
    // Reverse so newest entries come first, then apply limit.
    entries.reverse();
    entries.truncate(limit);

    let total = entries.len();
    Json(DiaryGetResponse {
        date: date_str,
        entries,
        total,
    })
    .into_response()
}

// ── Sessions API ─────────────────────────────────────────────────────

/// Response shape for GET /api/sessions.
#[derive(Debug, Serialize)]
pub struct SessionsResponse {
    pub sessions: Vec<SessionItem>,
}

/// A single session in the GET /api/sessions response.
#[derive(Debug, Serialize)]
pub struct SessionItem {
    pub id: String,
    pub project: Option<String>,
    pub status: String,
    pub agent_name: String,
    pub started_at: Option<String>,
    pub duration_display: String,
    pub branch: Option<String>,
    pub spec: Option<String>,
    pub progress: Option<serde_json::Value>,
}

/// GET /api/sessions — list all TeamAgent sessions.
///
/// Returns `{ "sessions": [...] }` with the shape expected by the dashboard
/// `SessionsGetResponse` type. When `team_agents` is not configured, returns
/// an empty sessions array (not an error).
async fn get_sessions_handler(State(state): State<Arc<HttpState>>) -> impl IntoResponse {
    let Some(ref dispatcher) = state.dispatcher else {
        return Json(SessionsResponse { sessions: vec![] }).into_response();
    };

    let summaries = dispatcher.list_agents().await;
    let sessions = summaries
        .into_iter()
        .map(|s| SessionItem {
            id: s.id,
            project: s.project,
            status: s.status,
            agent_name: s.agent_name,
            started_at: s.started_at.map(|dt| dt.to_rfc3339()),
            duration_display: s.duration_display,
            branch: s.branch,
            spec: s.spec,
            progress: None,
        })
        .collect();

    Json(SessionsResponse { sessions }).into_response()
}

// ── Tool Call Endpoint ────────────────────────────────────────────────

/// POST /api/tool-call — execute a Nova tool and return the result.
///
/// This endpoint is the MCP bridge used by `nova-tools-mcp.py`.  When the
/// Python Agent SDK calls a tool, the MCP server forwards the call here and
/// the Rust daemon dispatches it through the existing
/// `tools::execute_tool_send_with_backend` path.
///
/// Security: only requests from 127.0.0.1 / ::1 are accepted.  All other
/// origins receive 403 Forbidden.  The peer address is injected by axum via
/// `into_make_service_with_connect_info`; in tests use `MockConnectInfo` with
/// `SocketAddr::from(([127, 0, 0, 1], 0))`.
async fn tool_call_handler(
    ConnectInfo(peer): ConnectInfo<std::net::SocketAddr>,
    State(state): State<Arc<HttpState>>,
    Json(body): Json<ToolCallRequest>,
) -> impl IntoResponse {
    // Localhost-only guard: reject anything not from 127.0.0.1 / ::1.
    let peer_ip = peer.ip();
    let is_local = peer_ip == std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
        || peer_ip == std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST);
    if !is_local {
        tracing::warn!(peer = %peer, "tool-call rejected: not from localhost");
        return (
            StatusCode::FORBIDDEN,
            Json(ToolCallResponse {
                result: None,
                error: Some("tool-call endpoint is restricted to localhost".into()),
            }),
        )
            .into_response();
    }

    let Some(ref td) = state.tool_dispatch else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ToolCallResponse {
                result: None,
                error: Some("tool dispatch not configured".into()),
            }),
        )
            .into_response();
    };

    let svc_regs = crate::tools::ServiceRegistries {
        stripe: td.stripe_registry.as_ref(),
        vercel: td.vercel_registry.as_ref(),
        sentry: td.sentry_registry.as_ref(),
        resend: td.resend_registry.as_ref(),
        ha: td.ha_registry.as_ref(),
        upstash: td.upstash_registry.as_ref(),
        ado: td.ado_registry.as_ref(),
        cloudflare: td.cloudflare_registry.as_ref(),
        doppler: td.doppler_registry.as_ref(),
        teams: td.teams_client.as_deref(),
    };

    match crate::tools::execute_tool_send_with_backend(
        &body.tool_name,
        &body.input,
        &td.memory,
        td.jira_registry.as_ref(),
        None, // nexus_backend — not available via HTTP path
        &td.project_registry,
        &td.channels,
        td.calendar_credentials.as_deref(),
        &td.calendar_id,
        &svc_regs,
    )
    .await
    {
        Ok(crate::tools::ToolResult::Immediate(text)) => (
            StatusCode::OK,
            Json(ToolCallResponse {
                result: Some(text),
                error: None,
            }),
        )
            .into_response(),
        Ok(crate::tools::ToolResult::PendingAction { description, .. }) => {
            // PendingAction requires Telegram confirmation — not supported via HTTP path.
            // Return the description as the result so the agent knows what's pending.
            (
                StatusCode::OK,
                Json(ToolCallResponse {
                    result: Some(format!("[pending confirmation] {description}")),
                    error: None,
                }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::warn!(tool = %body.tool_name, error = %e, "tool-call execution failed");
            (
                StatusCode::OK, // Return 200 with error in body so the MCP server can surface it
                Json(ToolCallResponse {
                    result: None,
                    error: Some(e.to_string()),
                }),
            )
                .into_response()
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::extract::connect_info::MockConnectInfo;
    use axum::http::Request;
    use tower::ServiceExt;

    fn setup() -> (Arc<HttpState>, mpsc::UnboundedReceiver<Trigger>, tempfile::TempDir) {
        let (tx, rx) = mpsc::unbounded_channel();
        let health = Arc::new(HealthState::new());
        let tmp = tempfile::TempDir::new().unwrap();
        let db_path = tmp.path().join("messages.db");
        // Initialize the message store so the DB file exists for /stats
        let _store = MessageStore::init(&db_path).unwrap();
        let (event_tx, _event_rx) = broadcast::channel(64);
        let state = Arc::new(HttpState {
            trigger_tx: tx,
            health,
            stats_db_path: db_path.clone(),
            teams_message_buffer: None,
            teams_client: None,
            jira_webhook_state: None,
            weekly_budget_usd: 50.0,
            teams_client_state: None,
            cold_start_store: None,
            event_tx,
            cc_session_manager: None,
            contact_store: None,
            diary: None,
            dispatcher: None,
            project_registry: HashMap::new(),
            config_path: None,
            memory_base_path: None,
            activity_buffer: ActivityRingBuffer::new(),
            tool_dispatch: None,
            pg_contact_store: None,
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

    // ── Memory endpoint tests [4.1] ──────────────────────────────────

    fn setup_with_memory() -> (Arc<HttpState>, mpsc::UnboundedReceiver<Trigger>, tempfile::TempDir) {
        let (tx, rx) = mpsc::unbounded_channel();
        let health = Arc::new(HealthState::new());
        let tmp = tempfile::TempDir::new().unwrap();
        let db_path = tmp.path().join("messages.db");
        let _store = MessageStore::init(&db_path).unwrap();

        // Initialise a memory directory inside the temp dir.
        let mem = crate::memory::Memory::new(tmp.path());
        mem.init().unwrap();

        let (event_tx, _event_rx) = broadcast::channel(64);
        let state = Arc::new(HttpState {
            trigger_tx: tx,
            health,
            stats_db_path: db_path,
            teams_message_buffer: None,
            teams_client: None,
            jira_webhook_state: None,
            weekly_budget_usd: 50.0,
            teams_client_state: None,
            cold_start_store: None,
            event_tx,
            cc_session_manager: None,
            contact_store: None,
            diary: None,
            dispatcher: None,
            project_registry: HashMap::new(),
            config_path: None,
            memory_base_path: Some(tmp.path().join("memory")),
            activity_buffer: ActivityRingBuffer::new(),
            tool_dispatch: None,
            pg_contact_store: None,
        });
        (state, rx, tmp)
    }

    // [4.1a] GET /api/memory returns 200 with { topics: [...] }
    #[tokio::test]
    async fn memory_list_returns_topics() {
        let (state, _rx, _tmp) = setup_with_memory();
        let app = build_router(state);

        let request = Request::builder()
            .uri("/api/memory")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let resp: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(resp["topics"].is_array(), "expected 'topics' array in response");
    }

    // [4.1b] PUT /api/memory with valid body returns 200
    #[tokio::test]
    async fn memory_put_returns_written() {
        let (state, _rx, _tmp) = setup_with_memory();
        let app = build_router(state);

        let payload = serde_json::json!({
            "topic": "test-topic",
            "content": "# Test\n\nSome content here."
        });

        let request = Request::builder()
            .method("PUT")
            .uri("/api/memory")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&payload).unwrap()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let resp: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(resp["topic"], "test-topic");
        assert!(resp["written"].as_u64().unwrap_or(0) > 0);
    }

    // [4.1c] POST /api/solve returns 200 with session_id
    #[tokio::test]
    async fn solve_returns_session_id() {
        let (state, _rx, _tmp) = setup();
        let app = build_router(state);

        let payload = serde_json::json!({
            "project": "nv",
            "error": "build failed",
            "context": "Cargo.toml:12"
        });

        let request = Request::builder()
            .method("POST")
            .uri("/api/solve")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&payload).unwrap()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let resp: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let session_id = resp["session_id"].as_str().unwrap_or("");
        assert!(!session_id.is_empty(), "expected non-empty session_id");
        // Validate it's a valid UUID
        assert!(
            uuid::Uuid::parse_str(session_id).is_ok(),
            "session_id should be a valid UUID, got: {session_id}"
        );
    }

    // ── Tool-call endpoint tests ──────────────────────────────────────

    fn setup_with_tool_dispatch(tmp: &tempfile::TempDir) -> Arc<ToolDispatch> {
        let mem = crate::memory::Memory::new(tmp.path());
        mem.init().unwrap();
        // Pre-write a memory topic for the test to read.
        mem.write("greetings", "# Greetings\n\nHello from memory.").unwrap();

        Arc::new(ToolDispatch {
            memory: mem,
            jira_registry: None,
            channels: HashMap::new(),
            project_registry: HashMap::new(),
            calendar_credentials: None,
            calendar_id: "primary".to_string(),
            stripe_registry: None,
            vercel_registry: None,
            sentry_registry: None,
            resend_registry: None,
            ha_registry: None,
            upstash_registry: None,
            ado_registry: None,
            cloudflare_registry: None,
            doppler_registry: None,
            teams_client: None,
        })
    }

    /// Build the router with `MockConnectInfo` set to 127.0.0.1 so that
    /// the tool-call handler passes the localhost guard in tests.
    fn build_local_router(state: Arc<HttpState>) -> impl tower::Service<
        axum::http::Request<axum::body::Body>,
        Response = axum::response::Response,
        Error = std::convert::Infallible,
        Future: Send,
    > {
        build_router(state)
            .layer(MockConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 0))))
    }

    // [1.2] POST /api/tool-call with read_memory returns memory content.
    #[tokio::test]
    async fn tool_call_read_memory_returns_content() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let health = Arc::new(HealthState::new());
        let tmp = tempfile::TempDir::new().unwrap();
        let db_path = tmp.path().join("messages.db");
        let _store = MessageStore::init(&db_path).unwrap();
        let (event_tx, _event_rx) = broadcast::channel(64);
        let tool_dispatch = setup_with_tool_dispatch(&tmp);

        let state = Arc::new(HttpState {
            trigger_tx: tx,
            health,
            stats_db_path: db_path,
            teams_message_buffer: None,
            teams_client: None,
            jira_webhook_state: None,
            weekly_budget_usd: 50.0,
            teams_client_state: None,
            cold_start_store: None,
            event_tx,
            cc_session_manager: None,
            contact_store: None,
            diary: None,
            dispatcher: None,
            project_registry: HashMap::new(),
            config_path: None,
            memory_base_path: None,
            activity_buffer: ActivityRingBuffer::new(),
            tool_dispatch: Some(tool_dispatch),
        });

        let app = build_local_router(state);

        let payload = serde_json::json!({
            "tool_name": "read_memory",
            "input": { "topic": "greetings" }
        });

        let request = Request::builder()
            .method("POST")
            .uri("/api/tool-call")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&payload).unwrap()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let resp: ToolCallResponse = serde_json::from_slice(&body).unwrap();
        assert!(resp.error.is_none(), "expected no error, got: {:?}", resp.error);
        let result = resp.result.expect("expected a result");
        assert!(
            result.contains("Hello from memory"),
            "expected memory content in result, got: {result}"
        );
    }

    // [1.2b] POST /api/tool-call with unknown tool returns error (not 5xx).
    #[tokio::test]
    async fn tool_call_unknown_tool_returns_error_body() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let health = Arc::new(HealthState::new());
        let tmp = tempfile::TempDir::new().unwrap();
        let db_path = tmp.path().join("messages.db");
        let _store = MessageStore::init(&db_path).unwrap();
        let (event_tx, _event_rx) = broadcast::channel(64);
        let tool_dispatch = setup_with_tool_dispatch(&tmp);

        let state = Arc::new(HttpState {
            trigger_tx: tx,
            health,
            stats_db_path: db_path,
            teams_message_buffer: None,
            teams_client: None,
            jira_webhook_state: None,
            weekly_budget_usd: 50.0,
            teams_client_state: None,
            cold_start_store: None,
            event_tx,
            cc_session_manager: None,
            contact_store: None,
            diary: None,
            dispatcher: None,
            project_registry: HashMap::new(),
            config_path: None,
            memory_base_path: None,
            activity_buffer: ActivityRingBuffer::new(),
            tool_dispatch: Some(tool_dispatch),
        });

        let app = build_local_router(state);

        let payload = serde_json::json!({
            "tool_name": "nonexistent_tool_xyz",
            "input": {}
        });

        let request = Request::builder()
            .method("POST")
            .uri("/api/tool-call")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&payload).unwrap()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        // Returns 200 with error in body (not a 5xx) so the MCP server can
        // surface the error to Claude.
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let resp: ToolCallResponse = serde_json::from_slice(&body).unwrap();
        assert!(resp.error.is_some(), "expected an error for unknown tool");
        assert!(resp.result.is_none());
    }

    // [1.2c] POST /api/tool-call returns 503 when tool_dispatch is not configured.
    #[tokio::test]
    async fn tool_call_returns_503_when_not_configured() {
        let (state, _rx, _tmp) = setup(); // tool_dispatch: None
        let app = build_local_router(state);

        let payload = serde_json::json!({
            "tool_name": "read_memory",
            "input": { "topic": "anything" }
        });

        let request = Request::builder()
            .method("POST")
            .uri("/api/tool-call")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&payload).unwrap()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
