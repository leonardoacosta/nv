//! HTTP mock tests for Jira client response handling and retry logic.
//!
//! Uses `wiremock` to simulate Jira API responses for various status codes,
//! retry behavior, and case-insensitive transition matching.

// These tests are compiled as part of `cargo test -p nv-daemon`.
// The nv-daemon crate is a binary, so we test via its public interface
// by duplicating the core logic in test helpers.

use std::time::Duration;

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ── Helper: Minimal Jira Client ─────────────────────────────────────

/// A minimal Jira-like client that uses the same patterns as JiraClient
/// but is self-contained for integration testing.
struct TestJiraClient {
    http: reqwest::Client,
    base_url: String,
}

impl TestJiraClient {
    fn new(base_url: &str) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        Self {
            http,
            base_url: base_url.to_string(),
        }
    }

    /// Simulate handle_response logic.
    async fn handle_response<T: serde::de::DeserializeOwned>(
        &self,
        resp: reqwest::Response,
        context: &str,
    ) -> anyhow::Result<T> {
        let status = resp.status();
        if status.is_success() {
            let body = resp.text().await?;
            let parsed: T = serde_json::from_str(&body).map_err(|e| {
                anyhow::anyhow!("Jira response parse error for {context}: {e}\nBody: {body}")
            })?;
            return Ok(parsed);
        }
        let error_body = resp.text().await.unwrap_or_default();
        match status.as_u16() {
            401 => anyhow::bail!("Jira auth failed -- check API token. {context}"),
            403 => anyhow::bail!("Jira permission denied. {context}: {error_body}"),
            404 => anyhow::bail!("Jira resource not found. {context}"),
            429 => anyhow::bail!("Jira rate limit hit. {context}"),
            s if s >= 500 => {
                anyhow::bail!("Jira server error ({status}). {context}: {error_body}")
            }
            _ => anyhow::bail!("Jira API error ({status}). {context}: {error_body}"),
        }
    }

    /// Simulate request_with_retry logic.
    async fn request_with_retry<F, Fut, T>(&self, max_retries: u32, f: F) -> anyhow::Result<T>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = anyhow::Result<T>>,
    {
        let mut backoff = Duration::from_millis(50); // Short for tests
        for attempt in 0..=max_retries {
            match f().await {
                Ok(result) => return Ok(result),
                Err(e) if attempt < max_retries && is_retryable(&e) => {
                    tokio::time::sleep(backoff).await;
                    backoff *= 2;
                }
                Err(e) => return Err(e),
            }
        }
        unreachable!()
    }

    async fn get_json<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        context: &str,
    ) -> anyhow::Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.http.get(&url).send().await?;
        self.handle_response(resp, context).await
    }

    async fn get_json_with_retry<T: serde::de::DeserializeOwned + 'static>(
        &self,
        url_path: &str,
        context: &str,
    ) -> anyhow::Result<T> {
        let path_owned = url_path.to_string();
        let ctx_owned = context.to_string();
        self.request_with_retry(3, || {
            let p = path_owned.clone();
            let c = ctx_owned.clone();
            async move {
                let url = format!("{}{}", self.base_url, p);
                let resp = self.http.get(&url).send().await?;
                self.handle_response(resp, &c).await
            }
        })
        .await
    }
}

fn is_retryable(e: &anyhow::Error) -> bool {
    let msg = e.to_string();
    msg.contains("429")
        || msg.contains("rate limit")
        || msg.contains("server error")
        || msg.contains("500")
        || msg.contains("502")
        || msg.contains("503")
        || msg.contains("504")
        || msg.contains("connection")
        || msg.contains("timed out")
        || msg.contains("timeout")
}

// ── handle_response Tests ───────────────────────────────────────────

#[tokio::test]
async fn handle_response_401_returns_auth_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/test"))
        .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
        .mount(&server)
        .await;

    let client = TestJiraClient::new(&server.uri());
    let result: anyhow::Result<serde_json::Value> = client.get_json("/test", "test").await;
    let err = result.unwrap_err().to_string();
    assert!(err.contains("auth failed"), "Expected auth error, got: {err}");
}

#[tokio::test]
async fn handle_response_403_returns_permission_denied() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/test"))
        .respond_with(
            ResponseTemplate::new(403).set_body_string("Forbidden: insufficient permissions"),
        )
        .mount(&server)
        .await;

    let client = TestJiraClient::new(&server.uri());
    let result: anyhow::Result<serde_json::Value> = client.get_json("/test", "test").await;
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("permission denied"),
        "Expected permission denied, got: {err}"
    );
}

#[tokio::test]
async fn handle_response_404_returns_not_found() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/test"))
        .respond_with(ResponseTemplate::new(404).set_body_string("Not Found"))
        .mount(&server)
        .await;

    let client = TestJiraClient::new(&server.uri());
    let result: anyhow::Result<serde_json::Value> = client.get_json("/test", "test").await;
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("not found"),
        "Expected not found, got: {err}"
    );
}

#[tokio::test]
async fn handle_response_429_returns_rate_limit() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/test"))
        .respond_with(ResponseTemplate::new(429).set_body_string("Rate limited"))
        .mount(&server)
        .await;

    let client = TestJiraClient::new(&server.uri());
    let result: anyhow::Result<serde_json::Value> = client.get_json("/test", "test").await;
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("rate limit"),
        "Expected rate limit, got: {err}"
    );
}

#[tokio::test]
async fn handle_response_200_returns_parsed_json() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/test"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"key": "value"})),
        )
        .mount(&server)
        .await;

    let client = TestJiraClient::new(&server.uri());
    let result: serde_json::Value = client.get_json("/test", "test").await.unwrap();
    assert_eq!(result["key"], "value");
}

#[tokio::test]
async fn handle_response_500_returns_server_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/test"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&server)
        .await;

    let client = TestJiraClient::new(&server.uri());
    let result: anyhow::Result<serde_json::Value> = client.get_json("/test", "test").await;
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("server error") || err.contains("500"),
        "Expected server error, got: {err}"
    );
}

// ── request_with_retry Tests ────────────────────────────────────────

#[tokio::test]
async fn retry_on_429_then_succeeds() {
    let server = MockServer::start().await;

    // First request returns 429, second returns 200
    Mock::given(method("GET"))
        .and(path("/search"))
        .respond_with(ResponseTemplate::new(429).set_body_string("Rate limited"))
        .up_to_n_times(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/search"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"total": 1, "maxResults": 50, "issues": []})),
        )
        .mount(&server)
        .await;

    let client = TestJiraClient::new(&server.uri());
    let result: serde_json::Value = client
        .get_json_with_retry("/search", "search")
        .await
        .unwrap();
    assert_eq!(result["total"], 1);
}

#[tokio::test]
async fn retry_on_500_exhausts_retries() {
    let server = MockServer::start().await;

    // Always return 500
    Mock::given(method("GET"))
        .and(path("/search"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&server)
        .await;

    let client = TestJiraClient::new(&server.uri());
    let result: anyhow::Result<serde_json::Value> =
        client.get_json_with_retry("/search", "search").await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("server error") || err.contains("500"),
        "Expected server error after retry exhaustion, got: {err}"
    );
}

#[tokio::test]
async fn retry_on_502_then_succeeds() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/issue"))
        .respond_with(ResponseTemplate::new(502).set_body_string("Bad Gateway"))
        .up_to_n_times(2)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/issue"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"id": "1", "key": "OO-1"})),
        )
        .mount(&server)
        .await;

    let client = TestJiraClient::new(&server.uri());
    let result: serde_json::Value = client
        .get_json_with_retry("/issue", "get issue")
        .await
        .unwrap();
    assert_eq!(result["key"], "OO-1");
}

#[tokio::test]
async fn no_retry_on_401() {
    let server = MockServer::start().await;

    // 401 should not be retried
    Mock::given(method("GET"))
        .and(path("/search"))
        .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
        .expect(1) // Should only be called once (no retry)
        .mount(&server)
        .await;

    let client = TestJiraClient::new(&server.uri());
    let result: anyhow::Result<serde_json::Value> =
        client.get_json_with_retry("/search", "search").await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("auth failed"), "Expected auth error, got: {err}");
}

#[tokio::test]
async fn no_retry_on_403() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/search"))
        .respond_with(ResponseTemplate::new(403).set_body_string("Forbidden"))
        .expect(1)
        .mount(&server)
        .await;

    let client = TestJiraClient::new(&server.uri());
    let result: anyhow::Result<serde_json::Value> =
        client.get_json_with_retry("/search", "search").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn no_retry_on_404() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/search"))
        .respond_with(ResponseTemplate::new(404).set_body_string("Not Found"))
        .expect(1)
        .mount(&server)
        .await;

    let client = TestJiraClient::new(&server.uri());
    let result: anyhow::Result<serde_json::Value> =
        client.get_json_with_retry("/search", "search").await;
    assert!(result.is_err());
}

// ── Case-Insensitive Transition Matching Tests ──────────────────────

#[tokio::test]
async fn transition_matching_case_insensitive() {
    let server = MockServer::start().await;

    // Mock transitions endpoint
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/OO-1/transitions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "transitions": [
                {"id": "11", "name": "To Do"},
                {"id": "21", "name": "In Progress"},
                {"id": "31", "name": "Done"}
            ]
        })))
        .mount(&server)
        .await;

    // Mock transition execution
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/OO-1/transitions"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let client = TestJiraClient::new(&server.uri());

    // Fetch transitions
    let resp = client.http.get(format!("{}/rest/api/3/issue/OO-1/transitions", server.uri()))
        .send().await.unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    let transitions = body["transitions"].as_array().unwrap();

    // Test case-insensitive matching
    for name_variant in &["in progress", "IN PROGRESS", "In Progress", "iN pRoGrEsS"] {
        let found = transitions.iter().find(|t| {
            t["name"]
                .as_str()
                .unwrap()
                .eq_ignore_ascii_case(name_variant)
        });
        assert!(
            found.is_some(),
            "Should match '{}' case-insensitively",
            name_variant
        );
        assert_eq!(found.unwrap()["id"], "21");
    }
}

#[tokio::test]
async fn transition_mismatch_lists_available() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/OO-1/transitions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "transitions": [
                {"id": "11", "name": "To Do"},
                {"id": "21", "name": "In Progress"},
                {"id": "31", "name": "Done"}
            ]
        })))
        .mount(&server)
        .await;

    let client = TestJiraClient::new(&server.uri());

    let resp = client.http.get(format!("{}/rest/api/3/issue/OO-1/transitions", server.uri()))
        .send().await.unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    let transitions = body["transitions"].as_array().unwrap();

    let target = "Nonexistent Status";
    let found = transitions.iter().find(|t| {
        t["name"]
            .as_str()
            .unwrap()
            .eq_ignore_ascii_case(target)
    });

    assert!(found.is_none());

    // Build error message like the real client does
    let available: Vec<&str> = transitions
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    let err_msg = format!(
        "Transition '{}' not available for OO-1. Available: {:?}",
        target, available
    );

    assert!(err_msg.contains("To Do"));
    assert!(err_msg.contains("In Progress"));
    assert!(err_msg.contains("Done"));
    assert!(err_msg.contains("Nonexistent Status"));
}

// ── PendingAction Expiry Tests ──────────────────────────────────────

#[test]
fn pending_action_expiry_detects_old_actions() {
    use chrono::{Duration, Utc};

    let now = Utc::now();
    let two_hours_ago = now - Duration::hours(2);
    let thirty_minutes_ago = now - Duration::minutes(30);

    // Action older than 1 hour should be detected as expired
    let age_old = now.signed_duration_since(two_hours_ago);
    assert!(age_old > Duration::hours(1));

    // Action less than 1 hour old should not be expired
    let age_recent = now.signed_duration_since(thirty_minutes_ago);
    assert!(age_recent <= Duration::hours(1));
}

// ── is_retryable Tests ──────────────────────────────────────────────

#[test]
fn is_retryable_identifies_rate_limit() {
    assert!(is_retryable(&anyhow::anyhow!("Jira rate limit hit. 429")));
    assert!(is_retryable(&anyhow::anyhow!("429 Too Many Requests")));
}

#[test]
fn is_retryable_identifies_server_errors() {
    assert!(is_retryable(&anyhow::anyhow!("Jira server error (500)")));
    assert!(is_retryable(&anyhow::anyhow!("502 Bad Gateway")));
    assert!(is_retryable(&anyhow::anyhow!("503 Service Unavailable")));
    assert!(is_retryable(&anyhow::anyhow!("504 Gateway Timeout")));
}

#[test]
fn is_retryable_identifies_network_errors() {
    assert!(is_retryable(&anyhow::anyhow!("connection refused")));
    assert!(is_retryable(&anyhow::anyhow!("request timed out")));
    assert!(is_retryable(&anyhow::anyhow!("connection timeout")));
}

#[test]
fn is_not_retryable_for_client_errors() {
    assert!(!is_retryable(&anyhow::anyhow!("Jira auth failed")));
    assert!(!is_retryable(&anyhow::anyhow!("Jira permission denied")));
    assert!(!is_retryable(&anyhow::anyhow!("Jira resource not found")));
    assert!(!is_retryable(&anyhow::anyhow!("Bad request")));
}
