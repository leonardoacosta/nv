//! Dashboard REST API endpoints and SPA file serving.
//!
//! Provides:
//!   - `/api/obligations` — list and update obligation records
//!   - `/api/projects`    — list configured project codes
//!   - `/api/sessions`    — recent worker session events
//!   - `/api/memory`      — read/write memory markdown files
//!   - `/api/config`      — read/write config fields
//!   - `/api/server-health` — daemon uptime and channel status
//!   - Static SPA files served from the embedded `dashboard/dist/` directory
//!   - SPA fallback: any non-API, non-asset path returns `index.html`

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, patch, put};
use axum::{Json, Router};
use nv_core::types::{ObligationOwner, ObligationStatus};
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};

use crate::health::HealthState;
use crate::obligation_store::ObligationStore;

// ── Embedded SPA Assets ─────────────────────────────────────────────

/// Embeds `dashboard/dist/` at compile time.
///
/// The path is relative to the crate root (crates/nv-daemon).
/// At runtime the struct serves all embedded files with correct MIME types.
#[derive(RustEmbed)]
#[folder = "../../dashboard/dist/"]
struct DashboardAssets;

// ── Shared dashboard state ───────────────────────────────────────────

/// State passed to all dashboard API handlers.
#[derive(Clone)]
pub struct DashboardState {
    pub health: Arc<HealthState>,
    pub obligation_store: Option<Arc<Mutex<ObligationStore>>>,
    /// `~/.nv` base path — memory files live at `{nv_base}/memory/`.
    pub nv_base: PathBuf,
    /// Serialized JSON of the full config (produced once at startup).
    pub config_json: Arc<serde_json::Value>,
}

// ── Router builder ───────────────────────────────────────────────────

/// Build the dashboard sub-router.
///
/// All `/api/*` routes are registered here.  The SPA catch-all MUST be the
/// last route so that `/api/*` paths are matched first by axum.
pub fn build_dashboard_router(state: DashboardState) -> Router {
    Router::new()
        // P-1 endpoints
        .route("/api/obligations", get(get_obligations))
        .route("/api/obligations/:id", patch(patch_obligation))
        .route("/api/projects", get(get_projects))
        .route("/api/sessions", get(get_sessions))
        // P-2 endpoints
        .route("/api/memory", get(get_memory))
        .route("/api/memory", put(put_memory))
        .route("/api/config", get(get_config))
        .route("/api/config", put(put_config))
        .route("/api/server-health", get(get_server_health))
        // SPA static files (assets with content-hashed names)
        .route("/assets/{*path}", get(spa_asset_handler))
        // SPA root
        .route("/", get(spa_index_handler))
        // SPA fallback for client-side routes
        .fallback(spa_fallback_handler)
        .with_state(state)
}

// ── SPA Handlers ─────────────────────────────────────────────────────

/// Serve the SPA `index.html`.
async fn spa_index_handler() -> impl IntoResponse {
    serve_embedded_file("index.html")
}

/// Serve a file from the embedded `/assets/` directory.
async fn spa_asset_handler(Path(path): Path<String>) -> impl IntoResponse {
    serve_embedded_file(&format!("assets/{path}"))
}

/// Fallback for all non-API routes — serves `index.html` for client-side routing.
async fn spa_fallback_handler() -> impl IntoResponse {
    serve_embedded_file("index.html")
}

/// Resolve and serve an embedded file with the correct `Content-Type`.
fn serve_embedded_file(path: &str) -> Response {
    match DashboardAssets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path)
                .first_or_octet_stream()
                .to_string();

            let mut headers = HeaderMap::new();
            if let Ok(val) = HeaderValue::from_str(&mime) {
                headers.insert(header::CONTENT_TYPE, val);
            }

            // Cache hashed assets aggressively; index.html must not be cached.
            if path == "index.html" {
                headers.insert(
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("no-cache, no-store, must-revalidate"),
                );
            } else {
                headers.insert(
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("public, max-age=31536000, immutable"),
                );
            }

            (StatusCode::OK, headers, Body::from(content.data.into_owned())).into_response()
        }
        None => (StatusCode::NOT_FOUND, "Not found").into_response(),
    }
}

// ── GET /api/obligations ─────────────────────────────────────────────

/// Query parameters for `GET /api/obligations`.
#[derive(Debug, Deserialize, Default)]
pub struct ObligationsQuery {
    /// Filter by status: `open`, `in_progress`, `done`, `dismissed`.
    pub status: Option<String>,
    /// Filter by owner: `nova`, `leo`.
    pub owner: Option<String>,
}

/// `GET /api/obligations` — list obligations with optional filters.
async fn get_obligations(
    State(state): State<DashboardState>,
    Query(query): Query<ObligationsQuery>,
) -> impl IntoResponse {
    let Some(store_arc) = &state.obligation_store else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "obligation store not available"})),
        )
            .into_response();
    };

    let result = {
        let store = store_arc.lock().map_err(|_| "lock poisoned");
        match store {
            Err(msg) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": msg})),
                )
                    .into_response()
            }
            Ok(store) => {
                // Apply optional filters
                if let Some(ref status_str) = query.status {
                    match status_str.parse::<ObligationStatus>() {
                        Ok(status) => store.list_by_status(&status),
                        Err(_) => {
                            return (
                                StatusCode::BAD_REQUEST,
                                Json(serde_json::json!({"error": format!("unknown status: {status_str}")})),
                            )
                                .into_response()
                        }
                    }
                } else if let Some(ref owner_str) = query.owner {
                    match owner_str.parse::<ObligationOwner>() {
                        Ok(owner) => store.list_by_owner(&owner),
                        Err(_) => {
                            return (
                                StatusCode::BAD_REQUEST,
                                Json(serde_json::json!({"error": format!("unknown owner: {owner_str}")})),
                            )
                                .into_response()
                        }
                    }
                } else {
                    store.list_all()
                }
            }
        }
    };

    match result {
        Ok(obligations) => (StatusCode::OK, Json(serde_json::to_value(obligations).unwrap_or_default())).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("query failed: {e}")})),
        )
            .into_response(),
    }
}

// ── PATCH /api/obligations/:id ────────────────────────────────────────

/// Request body for `PATCH /api/obligations/:id`.
#[derive(Debug, Deserialize)]
pub struct PatchObligationRequest {
    pub status: String,
}

/// `PATCH /api/obligations/:id` — update the status of an obligation.
async fn patch_obligation(
    State(state): State<DashboardState>,
    Path(id): Path<String>,
    Json(body): Json<PatchObligationRequest>,
) -> impl IntoResponse {
    let Some(store_arc) = &state.obligation_store else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "obligation store not available"})),
        )
            .into_response();
    };

    let new_status = match body.status.parse::<ObligationStatus>() {
        Ok(s) => s,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("unknown status: {}", body.status)})),
            )
                .into_response()
        }
    };

    let result = {
        let store = store_arc.lock().map_err(|_| "lock poisoned");
        match store {
            Err(msg) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": msg})),
                )
                    .into_response()
            }
            Ok(store) => {
                // Verify the obligation exists
                match store.get_by_id(&id) {
                    Ok(None) => {
                        return (
                            StatusCode::NOT_FOUND,
                            Json(serde_json::json!({"error": format!("obligation {id} not found")})),
                        )
                            .into_response()
                    }
                    Err(e) => {
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(serde_json::json!({"error": format!("lookup failed: {e}")})),
                        )
                            .into_response()
                    }
                    Ok(Some(_)) => {}
                }
                store.update_status(&id, &new_status)
            }
        }
    };

    match result {
        Ok(true) => {
            // Re-fetch the updated obligation to return it
            let store = store_arc.lock().map_err(|_| "lock poisoned");
            match store {
                Err(msg) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": msg})),
                )
                    .into_response(),
                Ok(store) => match store.get_by_id(&id) {
                    Ok(Some(ob)) => (StatusCode::OK, Json(serde_json::to_value(ob).unwrap_or_default())).into_response(),
                    Ok(None) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "obligation not found after update"}))).into_response(),
                    Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": format!("{e}")}))).into_response(),
                },
            }
        }
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": format!("obligation {id} not found")})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("update failed: {e}")})),
        )
            .into_response(),
    }
}

// ── GET /api/projects ─────────────────────────────────────────────────

/// `GET /api/projects` — list configured project codes from config.
async fn get_projects(State(state): State<DashboardState>) -> impl IntoResponse {
    let projects: Vec<serde_json::Value> = state
        .config_json
        .get("projects")
        .and_then(|v| v.as_object())
        .map(|map| {
            map.iter()
                .map(|(code, path)| {
                    serde_json::json!({
                        "code": code,
                        "path": path,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    (StatusCode::OK, Json(serde_json::json!({"projects": projects}))).into_response()
}

// ── GET /api/sessions ─────────────────────────────────────────────────

/// `GET /api/sessions` — return recent worker session events from health state.
///
/// Currently returns channel statuses as a proxy for active integrations.
/// Session-level tracking (per-worker event history) is a future enhancement.
async fn get_sessions(State(state): State<DashboardState>) -> impl IntoResponse {
    let health = state.health.to_health_response().await;

    let sessions: Vec<serde_json::Value> = health
        .channels
        .iter()
        .map(|(name, status)| {
            serde_json::json!({
                "channel": name,
                "status": status,
            })
        })
        .collect();

    (StatusCode::OK, Json(serde_json::json!({
        "sessions": sessions,
        "uptime_secs": health.uptime_secs,
        "triggers_processed": health.triggers_processed,
        "last_digest_at": health.last_digest_at,
    })))
    .into_response()
}

// ── GET /api/memory ───────────────────────────────────────────────────

/// Query parameters for `GET /api/memory`.
#[derive(Debug, Deserialize, Default)]
pub struct MemoryQuery {
    /// Optional topic name (e.g. `conversations`). Returns index listing when absent.
    pub topic: Option<String>,
}

/// `GET /api/memory` — list memory topics or read a specific file.
async fn get_memory(
    State(state): State<DashboardState>,
    Query(query): Query<MemoryQuery>,
) -> impl IntoResponse {
    let memory_path = state.nv_base.join("memory");

    if let Some(topic) = query.topic {
        // Read a specific memory file
        let mem = crate::memory::Memory::from_base_path(memory_path);
        match mem.read(&topic) {
            Ok(content) => (StatusCode::OK, Json(serde_json::json!({
                "topic": topic,
                "content": content,
            })))
            .into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("{e}")})),
            )
                .into_response(),
        }
    } else {
        // List available memory files
        match std::fs::read_dir(&memory_path) {
            Ok(entries) => {
                let topics: Vec<String> = entries
                    .filter_map(|e| e.ok())
                    .filter_map(|e| {
                        let path = e.path();
                        if path.extension().map(|x| x == "md").unwrap_or(false) {
                            path.file_stem()
                                .map(|s| s.to_string_lossy().to_string())
                        } else {
                            None
                        }
                    })
                    .collect();

                (StatusCode::OK, Json(serde_json::json!({"topics": topics}))).into_response()
            }
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("failed to read memory directory: {e}")})),
            )
                .into_response(),
        }
    }
}

// ── PUT /api/memory ───────────────────────────────────────────────────

/// Request body for `PUT /api/memory`.
#[derive(Debug, Deserialize)]
pub struct PutMemoryRequest {
    pub topic: String,
    pub content: String,
}

/// Response body for `PUT /api/memory`.
#[derive(Debug, Serialize)]
pub struct PutMemoryResponse {
    pub topic: String,
    pub written: usize,
}

/// `PUT /api/memory` — write a memory file for a given topic.
async fn put_memory(
    State(state): State<DashboardState>,
    Json(body): Json<PutMemoryRequest>,
) -> impl IntoResponse {
    if body.topic.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "topic must not be empty"})),
        )
            .into_response();
    }

    // Sanitize topic to prevent path traversal
    let safe_topic = body
        .topic
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect::<String>();

    if safe_topic.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "topic contains no valid characters"})),
        )
            .into_response();
    }

    let path = state.nv_base.join("memory").join(format!("{safe_topic}.md"));

    match std::fs::write(&path, &body.content) {
        Ok(()) => (
            StatusCode::OK,
            Json(PutMemoryResponse {
                topic: safe_topic,
                written: body.content.len(),
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("failed to write memory file: {e}")})),
        )
            .into_response(),
    }
}

// ── GET /api/config ───────────────────────────────────────────────────

/// `GET /api/config` — return the current daemon config (secrets redacted).
async fn get_config(State(state): State<DashboardState>) -> impl IntoResponse {
    (StatusCode::OK, Json((*state.config_json).clone())).into_response()
}

// ── PUT /api/config ───────────────────────────────────────────────────

/// `PUT /api/config` — update config fields by rewriting `~/.nv/config.toml`.
///
/// Only top-level scalar fields that exist in the current config are accepted.
/// This prevents injecting unknown keys or overwriting complex sub-objects.
#[derive(Debug, Deserialize)]
pub struct PutConfigRequest {
    /// Flat key-value pairs to merge into the config.
    pub fields: serde_json::Value,
}

async fn put_config(
    State(state): State<DashboardState>,
    Json(body): Json<PutConfigRequest>,
) -> impl IntoResponse {
    let config_path = state.nv_base.join("config.toml");

    // Read current config as raw TOML string
    let raw = match std::fs::read_to_string(&config_path) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("failed to read config: {e}")})),
            )
                .into_response()
        }
    };

    // Parse as TOML value for mutation
    let mut doc: toml::Value = match raw.parse() {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("failed to parse config TOML: {e}")})),
            )
                .into_response()
        }
    };

    // Apply only the provided scalar fields
    let Some(fields_obj) = body.fields.as_object() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "fields must be a JSON object"})),
        )
            .into_response();
    };

    let doc_table = match &mut doc {
        toml::Value::Table(t) => t,
        _ => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "config is not a TOML table"})),
            )
                .into_response()
        }
    };

    let mut applied = Vec::new();
    for (key, val) in fields_obj {
        // Only update top-level keys that already exist as scalar values
        if let Some(existing) = doc_table.get(key) {
            if existing.is_str() || existing.is_integer() || existing.is_bool() || existing.is_float() {
                let toml_val = json_to_toml(val);
                if let Some(tv) = toml_val {
                    doc_table.insert(key.clone(), tv);
                    applied.push(key.as_str());
                }
            }
        }
    }

    // Serialize back to TOML
    let new_raw = match toml::to_string_pretty(&doc) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("failed to serialize config: {e}")})),
            )
                .into_response()
        }
    };

    match std::fs::write(&config_path, &new_raw) {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "applied": applied,
                "note": "restart daemon for changes to take effect",
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("failed to write config: {e}")})),
        )
            .into_response(),
    }
}

/// Convert a serde_json `Value` to a `toml::Value` for scalar types.
fn json_to_toml(val: &serde_json::Value) -> Option<toml::Value> {
    match val {
        serde_json::Value::String(s) => Some(toml::Value::String(s.clone())),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Some(toml::Value::Integer(i))
            } else if let Some(f) = n.as_f64() {
                Some(toml::Value::Float(f))
            } else {
                None
            }
        }
        serde_json::Value::Bool(b) => Some(toml::Value::Boolean(*b)),
        _ => None,
    }
}

// ── GET /api/server-health ────────────────────────────────────────────

/// `GET /api/server-health` — return daemon uptime, channel status, and worker metrics.
async fn get_server_health(State(state): State<DashboardState>) -> impl IntoResponse {
    let health = state.health.to_health_response().await;
    (StatusCode::OK, Json(health)).into_response()
}
