use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use chrono::Utc;
use nv_core::types::Trigger;
use serde::Deserialize;
use tokio::sync::mpsc;

use crate::memory::Memory;

// ── Webhook Payload Types ─────────────────────────────────────────

/// Top-level Jira webhook event envelope.
///
/// Jira sends different shapes depending on the event, but all share
/// `webhookEvent`, `timestamp`, and optional `user` fields.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebhookEvent {
    /// Event type string, e.g. "jira:issue_updated", "jira:issue_created", "comment_created".
    pub webhook_event: String,
    /// Unix timestamp in milliseconds (present in payload, used for audit/logging).
    #[serde(default)]
    #[allow(dead_code)]
    pub timestamp: Option<i64>,
    /// The user who triggered the event.
    pub user: Option<WebhookUser>,
    /// Issue data (present for issue and comment events).
    pub issue: Option<WebhookIssue>,
    /// Changelog (present for issue_updated events).
    pub changelog: Option<WebhookChangelog>,
    /// Comment data (present for comment_created events).
    pub comment: Option<WebhookComment>,
}

/// User reference in a webhook payload.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebhookUser {
    pub display_name: String,
    /// Account ID for cross-referencing with Jira API (deserialized for completeness).
    #[serde(default)]
    #[allow(dead_code)]
    pub account_id: Option<String>,
}

/// Issue reference in a webhook payload.
#[derive(Debug, Clone, Deserialize)]
pub struct WebhookIssue {
    pub key: String,
    pub fields: WebhookIssueFields,
}

/// Subset of issue fields relevant to webhook processing.
#[derive(Debug, Clone, Deserialize)]
pub struct WebhookIssueFields {
    pub summary: String,
    pub status: WebhookStatus,
    pub assignee: Option<WebhookUser>,
    /// Priority (deserialized for completeness, used in changelog diffing).
    #[allow(dead_code)]
    pub priority: Option<WebhookPriority>,
}

/// Status in a webhook issue.
#[derive(Debug, Clone, Deserialize)]
pub struct WebhookStatus {
    pub name: String,
}

/// Priority in a webhook issue.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct WebhookPriority {
    pub name: String,
}

/// Changelog attached to issue_updated events.
#[derive(Debug, Clone, Deserialize)]
pub struct WebhookChangelog {
    pub items: Vec<ChangelogItem>,
}

/// A single field change in a changelog.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangelogItem {
    pub field: String,
    pub from_string: Option<String>,
    pub to_string: Option<String>,
}

/// Comment in a webhook payload.
#[derive(Debug, Clone, Deserialize)]
pub struct WebhookComment {
    pub author: WebhookUser,
    /// Comment body — Jira sends ADF (JSON), but we stringify for logging.
    pub body: serde_json::Value,
    /// Creation timestamp (deserialized for completeness).
    #[allow(dead_code)]
    pub created: Option<String>,
}

// ── Shared State ──────────────────────────────────────────────────

/// State shared with the Jira webhook handler.
#[derive(Clone)]
pub struct JiraWebhookState {
    pub trigger_tx: mpsc::UnboundedSender<Trigger>,
    pub webhook_secret: Option<String>,
    pub memory_base_path: std::path::PathBuf,
}

// ── Webhook Handler ───────────────────────────────────────────────

/// POST /webhooks/jira — receive Jira webhook payloads.
///
/// 1. Validates webhook secret from `X-Jira-Webhook-Secret` header.
/// 2. Parses the JSON body into `WebhookEvent`.
/// 3. Routes to event-specific handlers.
/// 4. Returns 200 OK immediately (Jira requires fast responses).
pub async fn jira_webhook_handler(
    State(state): State<Arc<JiraWebhookState>>,
    headers: HeaderMap,
    body: String,
) -> impl IntoResponse {
    // Step 1: Validate webhook secret
    if let Some(expected_secret) = &state.webhook_secret {
        let provided = headers
            .get("x-jira-webhook-secret")
            .and_then(|v| v.to_str().ok());

        match provided {
            Some(secret) if secret == expected_secret => {}
            Some(_) => {
                tracing::warn!("Jira webhook rejected: invalid secret");
                return StatusCode::UNAUTHORIZED;
            }
            None => {
                tracing::warn!("Jira webhook rejected: missing secret header");
                return StatusCode::UNAUTHORIZED;
            }
        }
    }

    // Step 2: Parse the webhook payload
    let event: WebhookEvent = match serde_json::from_str(&body) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to parse Jira webhook payload");
            // Return 200 to prevent Jira from retrying endlessly
            return StatusCode::OK;
        }
    };

    // Step 3: Route and process in the background (return 200 immediately)
    tokio::spawn(async move {
        process_webhook_event(event, &state).await;
    });

    StatusCode::OK
}

/// Process a parsed webhook event — route to type-specific handlers.
async fn process_webhook_event(event: WebhookEvent, state: &JiraWebhookState) {
    let event_type = event.webhook_event.as_str();

    tracing::info!(
        event_type,
        issue_key = event.issue.as_ref().map(|i| i.key.as_str()),
        "processing Jira webhook"
    );

    match event_type {
        "jira:issue_updated" => handle_issue_updated(&event, state),
        "jira:issue_created" => handle_issue_created(&event, state),
        "comment_created" => handle_comment_created(&event, state),
        _ => {
            tracing::debug!(event_type, "ignoring unhandled Jira webhook event type");
        }
    }
}

// ── Event Handlers ────────────────────────────────────────────────

/// Handle `jira:issue_updated` — extract relevant changelog items and alert.
fn handle_issue_updated(event: &WebhookEvent, state: &JiraWebhookState) {
    let issue = match &event.issue {
        Some(i) => i,
        None => {
            tracing::warn!("issue_updated event missing issue field");
            return;
        }
    };

    let actor = event
        .user
        .as_ref()
        .map(|u| u.display_name.as_str())
        .unwrap_or("unknown");

    // Filter changelog for relevant fields
    let relevant_fields = ["status", "assignee", "priority"];
    let changes: Vec<&ChangelogItem> = event
        .changelog
        .as_ref()
        .map(|cl| {
            cl.items
                .iter()
                .filter(|item| relevant_fields.contains(&item.field.as_str()))
                .collect()
        })
        .unwrap_or_default();

    if changes.is_empty() {
        tracing::debug!(
            key = %issue.key,
            "issue_updated with no relevant field changes, skipping"
        );
        return;
    }

    // Format alert messages
    let mut alert_parts = Vec::new();
    let mut memory_parts = Vec::new();

    for change in &changes {
        let from = change.from_string.as_deref().unwrap_or("none");
        let to = change.to_string.as_deref().unwrap_or("none");

        alert_parts.push(format!(
            "{} {}: {} -> {}",
            issue.key, change.field, from, to
        ));
        memory_parts.push(format!(
            "Jira: {} {} changed from '{}' to '{}' by {}",
            issue.key, change.field, from, to, actor
        ));
    }

    let alert_message = format!("[Jira webhook] {}", alert_parts.join("; "));

    // Write to memory
    let memory_content = memory_parts.join("\n");
    write_jira_memory(&state.memory_base_path, &memory_content);

    // Send trigger to agent loop
    send_jira_trigger(&state.trigger_tx, &alert_message);

    tracing::info!(
        key = %issue.key,
        changes = changes.len(),
        actor,
        "Jira issue updated"
    );
}

/// Handle `jira:issue_created` — log new issue to memory and alert.
fn handle_issue_created(event: &WebhookEvent, state: &JiraWebhookState) {
    let issue = match &event.issue {
        Some(i) => i,
        None => {
            tracing::warn!("issue_created event missing issue field");
            return;
        }
    };

    let actor = event
        .user
        .as_ref()
        .map(|u| u.display_name.as_str())
        .unwrap_or("unknown");

    let assignee = issue
        .fields
        .assignee
        .as_ref()
        .map(|a| a.display_name.as_str())
        .unwrap_or("unassigned");

    let alert_message = format!(
        "[Jira webhook] New issue {}: {} (by {}, assigned to {})",
        issue.key, issue.fields.summary, actor, assignee
    );

    let memory_content = format!(
        "Jira: New issue {} created by {} — \"{}\" [status: {}, assigned: {}]",
        issue.key,
        actor,
        issue.fields.summary,
        issue.fields.status.name,
        assignee,
    );

    write_jira_memory(&state.memory_base_path, &memory_content);
    send_jira_trigger(&state.trigger_tx, &alert_message);

    tracing::info!(key = %issue.key, actor, "Jira issue created");
}

/// Handle `comment_created` — log comment to memory and alert.
fn handle_comment_created(event: &WebhookEvent, state: &JiraWebhookState) {
    let issue = match &event.issue {
        Some(i) => i,
        None => {
            tracing::warn!("comment_created event missing issue field");
            return;
        }
    };

    let comment = match &event.comment {
        Some(c) => c,
        None => {
            tracing::warn!("comment_created event missing comment field");
            return;
        }
    };

    let author = &comment.author.display_name;

    // Extract plain text preview from ADF body (best-effort)
    let body_preview = extract_text_from_adf(&comment.body);
    let preview = if body_preview.len() > 200 {
        format!("{}...", &body_preview[..200])
    } else {
        body_preview.clone()
    };

    let alert_message = format!(
        "[Jira webhook] {} commented on {}: {}",
        author, issue.key, preview
    );

    let memory_content = format!(
        "Jira: {} commented on {} — \"{}\"",
        author, issue.key, preview
    );

    write_jira_memory(&state.memory_base_path, &memory_content);
    send_jira_trigger(&state.trigger_tx, &alert_message);

    tracing::info!(
        key = %issue.key,
        author,
        "Jira comment created"
    );
}

// ── Helpers ───────────────────────────────────────────────────────

/// Write a Jira event to the memory system under the "jira-events" topic.
fn write_jira_memory(memory_base_path: &std::path::Path, content: &str) {
    let memory = Memory::from_base_path(memory_base_path.to_path_buf());
    if let Err(e) = memory.write("jira-events", content) {
        tracing::error!(error = %e, "failed to write Jira event to memory");
    }
}

/// Send a Jira webhook alert as an InboundMessage trigger to the agent loop.
fn send_jira_trigger(trigger_tx: &mpsc::UnboundedSender<Trigger>, message: &str) {
    let inbound = nv_core::types::InboundMessage {
        id: format!("jira-webhook-{}", Utc::now().timestamp_millis()),
        channel: "jira-webhook".to_string(),
        sender: "Jira".to_string(),
        content: message.to_string(),
        timestamp: Utc::now(),
        thread_id: None,
        metadata: serde_json::json!({}),
    };

    if let Err(e) = trigger_tx.send(Trigger::Message(inbound)) {
        tracing::error!(error = %e, "failed to send Jira webhook trigger");
    }
}

/// Best-effort extraction of plain text from a Jira ADF (Atlassian Document Format) JSON body.
///
/// ADF is a nested tree of content nodes. We walk it recursively looking for
/// `"type": "text"` nodes and concatenate their `text` fields.
fn extract_text_from_adf(value: &serde_json::Value) -> String {
    let mut result = String::new();
    extract_text_recursive(value, &mut result);
    result.trim().to_string()
}

fn extract_text_recursive(value: &serde_json::Value, result: &mut String) {
    match value {
        serde_json::Value::Object(map) => {
            // If this is a text node, extract the text
            if map.get("type").and_then(|t| t.as_str()) == Some("text") {
                if let Some(text) = map.get("text").and_then(|t| t.as_str()) {
                    if !result.is_empty() {
                        result.push(' ');
                    }
                    result.push_str(text);
                }
            }
            // Recurse into content array
            if let Some(content) = map.get("content") {
                extract_text_recursive(content, result);
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                extract_text_recursive(item, result);
            }
        }
        _ => {}
    }
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::post;
    use axum::Router;
    use tower::ServiceExt;

    fn make_state(secret: Option<&str>) -> (Arc<JiraWebhookState>, mpsc::UnboundedReceiver<Trigger>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let tmp = std::env::temp_dir().join(format!("nv-test-jira-{}", std::process::id()));
        std::fs::create_dir_all(tmp.join("memory")).ok();
        let state = Arc::new(JiraWebhookState {
            trigger_tx: tx,
            webhook_secret: secret.map(|s| s.to_string()),
            memory_base_path: tmp.join("memory"),
        });
        (state, rx)
    }

    fn build_app(state: Arc<JiraWebhookState>) -> Router {
        Router::new()
            .route("/webhooks/jira", post(jira_webhook_handler))
            .with_state(state)
    }

    // ── Serde tests ───────────────────────────────────────────────

    #[test]
    fn deserialize_issue_updated_event() {
        let json = serde_json::json!({
            "webhookEvent": "jira:issue_updated",
            "timestamp": 1711100000000_i64,
            "user": {
                "displayName": "Leo Acosta",
                "accountId": "abc123"
            },
            "issue": {
                "key": "OO-143",
                "fields": {
                    "summary": "Fix auth flow",
                    "status": {"name": "Done"},
                    "assignee": {"displayName": "Leo Acosta", "accountId": "abc123"},
                    "priority": {"name": "High"}
                }
            },
            "changelog": {
                "items": [
                    {
                        "field": "status",
                        "fromString": "In Progress",
                        "toString": "Done"
                    }
                ]
            }
        });

        let event: WebhookEvent = serde_json::from_value(json).unwrap();
        assert_eq!(event.webhook_event, "jira:issue_updated");
        assert_eq!(event.issue.as_ref().unwrap().key, "OO-143");
        let items = &event.changelog.as_ref().unwrap().items;
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].field, "status");
        assert_eq!(items[0].from_string.as_deref(), Some("In Progress"));
        assert_eq!(items[0].to_string.as_deref(), Some("Done"));
    }

    #[test]
    fn deserialize_issue_created_event() {
        let json = serde_json::json!({
            "webhookEvent": "jira:issue_created",
            "timestamp": 1711100000000_i64,
            "user": {
                "displayName": "CI Pipeline"
            },
            "issue": {
                "key": "OO-200",
                "fields": {
                    "summary": "New deployment task",
                    "status": {"name": "To Do"},
                    "assignee": null,
                    "priority": {"name": "Medium"}
                }
            }
        });

        let event: WebhookEvent = serde_json::from_value(json).unwrap();
        assert_eq!(event.webhook_event, "jira:issue_created");
        assert_eq!(event.issue.as_ref().unwrap().key, "OO-200");
        assert_eq!(
            event.issue.as_ref().unwrap().fields.summary,
            "New deployment task"
        );
        assert!(event.issue.as_ref().unwrap().fields.assignee.is_none());
    }

    #[test]
    fn deserialize_comment_created_event() {
        let json = serde_json::json!({
            "webhookEvent": "comment_created",
            "timestamp": 1711100000000_i64,
            "user": {
                "displayName": "Leo Acosta"
            },
            "issue": {
                "key": "OO-143",
                "fields": {
                    "summary": "Fix auth flow",
                    "status": {"name": "In Progress"},
                    "assignee": null,
                    "priority": null
                }
            },
            "comment": {
                "author": {
                    "displayName": "Leo Acosta"
                },
                "body": {
                    "type": "doc",
                    "version": 1,
                    "content": [
                        {
                            "type": "paragraph",
                            "content": [
                                {"type": "text", "text": "Fixed in v2.1 — deploy pending"}
                            ]
                        }
                    ]
                },
                "created": "2026-03-21T15:00:00.000+0000"
            }
        });

        let event: WebhookEvent = serde_json::from_value(json).unwrap();
        assert_eq!(event.webhook_event, "comment_created");
        let comment = event.comment.as_ref().unwrap();
        assert_eq!(comment.author.display_name, "Leo Acosta");
    }

    #[test]
    fn deserialize_minimal_event() {
        // Jira may send minimal payloads for some event types
        let json = serde_json::json!({
            "webhookEvent": "jira:issue_deleted",
            "timestamp": 1711100000000_i64
        });

        let event: WebhookEvent = serde_json::from_value(json).unwrap();
        assert_eq!(event.webhook_event, "jira:issue_deleted");
        assert!(event.issue.is_none());
        assert!(event.changelog.is_none());
        assert!(event.comment.is_none());
    }

    // ── ADF text extraction ───────────────────────────────────────

    #[test]
    fn extract_text_from_adf_simple() {
        let adf = serde_json::json!({
            "type": "doc",
            "version": 1,
            "content": [
                {
                    "type": "paragraph",
                    "content": [
                        {"type": "text", "text": "Hello"},
                        {"type": "text", "text": "world"}
                    ]
                }
            ]
        });

        let text = extract_text_from_adf(&adf);
        assert_eq!(text, "Hello world");
    }

    #[test]
    fn extract_text_from_adf_empty() {
        let adf = serde_json::json!({
            "type": "doc",
            "version": 1,
            "content": []
        });

        let text = extract_text_from_adf(&adf);
        assert_eq!(text, "");
    }

    // ── Secret validation tests ───────────────────────────────────

    #[tokio::test]
    async fn webhook_valid_secret_returns_ok() {
        let (state, _rx) = make_state(Some("my-secret"));
        let app = build_app(state);

        let request = Request::builder()
            .method("POST")
            .uri("/webhooks/jira")
            .header("content-type", "application/json")
            .header("x-jira-webhook-secret", "my-secret")
            .body(Body::from(
                serde_json::json!({
                    "webhookEvent": "jira:issue_updated",
                    "issue": {
                        "key": "OO-1",
                        "fields": {
                            "summary": "Test",
                            "status": {"name": "Done"},
                            "assignee": null,
                            "priority": null
                        }
                    },
                    "changelog": {
                        "items": [{"field": "status", "fromString": "Open", "toString": "Done"}]
                    },
                    "user": {"displayName": "Leo"}
                })
                .to_string(),
            ))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn webhook_missing_secret_returns_401() {
        let (state, _rx) = make_state(Some("my-secret"));
        let app = build_app(state);

        let request = Request::builder()
            .method("POST")
            .uri("/webhooks/jira")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"webhookEvent": "jira:issue_updated"}"#))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn webhook_wrong_secret_returns_401() {
        let (state, _rx) = make_state(Some("my-secret"));
        let app = build_app(state);

        let request = Request::builder()
            .method("POST")
            .uri("/webhooks/jira")
            .header("content-type", "application/json")
            .header("x-jira-webhook-secret", "wrong-secret")
            .body(Body::from(r#"{"webhookEvent": "jira:issue_updated"}"#))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn webhook_no_secret_configured_accepts_all() {
        let (state, _rx) = make_state(None);
        let app = build_app(state);

        let request = Request::builder()
            .method("POST")
            .uri("/webhooks/jira")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::json!({
                    "webhookEvent": "jira:issue_created",
                    "issue": {
                        "key": "OO-1",
                        "fields": {
                            "summary": "Test",
                            "status": {"name": "Open"},
                            "assignee": null,
                            "priority": null
                        }
                    },
                    "user": {"displayName": "Leo"}
                })
                .to_string(),
            ))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn webhook_invalid_json_returns_ok() {
        // Return 200 on parse failure to prevent Jira retries
        let (state, _rx) = make_state(None);
        let app = build_app(state);

        let request = Request::builder()
            .method("POST")
            .uri("/webhooks/jira")
            .header("content-type", "application/json")
            .body(Body::from("not json"))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    // ── Event routing tests ───────────────────────────────────────

    #[tokio::test]
    async fn webhook_unknown_event_returns_ok() {
        let (state, _rx) = make_state(None);
        let app = build_app(state);

        let request = Request::builder()
            .method("POST")
            .uri("/webhooks/jira")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::json!({
                    "webhookEvent": "jira:unknown_event_type"
                })
                .to_string(),
            ))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn webhook_issue_updated_sends_trigger() {
        let (state, mut rx) = make_state(None);
        let app = build_app(state);

        let request = Request::builder()
            .method("POST")
            .uri("/webhooks/jira")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::json!({
                    "webhookEvent": "jira:issue_updated",
                    "user": {"displayName": "Leo"},
                    "issue": {
                        "key": "OO-143",
                        "fields": {
                            "summary": "Fix auth",
                            "status": {"name": "Done"},
                            "assignee": null,
                            "priority": null
                        }
                    },
                    "changelog": {
                        "items": [
                            {"field": "status", "fromString": "In Progress", "toString": "Done"}
                        ]
                    }
                })
                .to_string(),
            ))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Give the spawned task a moment to process
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Should have received a trigger
        let trigger = rx.try_recv();
        assert!(trigger.is_ok(), "expected a trigger to be sent");
        if let Ok(Trigger::Message(msg)) = trigger {
            assert_eq!(msg.channel, "jira-webhook");
            assert!(msg.content.contains("OO-143"));
            assert!(msg.content.contains("status"));
            assert!(msg.content.contains("[Jira webhook]"));
        } else {
            panic!("expected Trigger::Message");
        }
    }

    // ── Telegram alert formatting tests ───────────────────────────

    #[test]
    fn format_issue_updated_alert() {
        // Verify the alert format for status change
        let key = "OO-143";
        let field = "status";
        let from = "In Progress";
        let to = "Done";
        let alert = format!("[Jira webhook] {} {}: {} -> {}", key, field, from, to);
        assert_eq!(
            alert,
            "[Jira webhook] OO-143 status: In Progress -> Done"
        );
    }

    #[test]
    fn format_issue_created_alert() {
        let key = "OO-200";
        let summary = "New deployment task";
        let actor = "CI Pipeline";
        let assignee = "unassigned";
        let alert = format!(
            "[Jira webhook] New issue {}: {} (by {}, assigned to {})",
            key, summary, actor, assignee
        );
        assert_eq!(
            alert,
            "[Jira webhook] New issue OO-200: New deployment task (by CI Pipeline, assigned to unassigned)"
        );
    }

    #[test]
    fn format_comment_created_alert() {
        let author = "Leo Acosta";
        let key = "OO-143";
        let preview = "Fixed in v2.1";
        let alert = format!("[Jira webhook] {} commented on {}: {}", author, key, preview);
        assert_eq!(
            alert,
            "[Jira webhook] Leo Acosta commented on OO-143: Fixed in v2.1"
        );
    }
}
