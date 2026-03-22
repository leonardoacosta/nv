use std::future::Future;
use std::time::Duration;

use anyhow::Result;
use base64::Engine;

use super::types::*;

// ── Retry Constants ───────────────────────────────────────────────

/// Default maximum number of retries for Jira API requests.
const DEFAULT_MAX_RETRIES: u32 = 3;

// ── JiraClient ─────────────────────────────────────────────────────

/// HTTP client for the Jira REST API v3.
///
/// Uses Basic auth (email + API token). Read operations execute
/// immediately; write operations are called only after PendingAction
/// confirmation. All HTTP calls are wrapped with exponential backoff
/// retry for transient failures (429/5xx/network errors).
pub struct JiraClient {
    http: reqwest::Client,
    base_url: String,
    #[allow(dead_code)]
    auth_email: String,
    #[allow(dead_code)]
    auth_token: String,
}

impl JiraClient {
    /// Create a new Jira client.
    ///
    /// * `instance_url` — e.g. "https://yourorg.atlassian.net"
    /// * `email` — Jira user email
    /// * `api_token` — Jira API token (not password)
    pub fn new(instance_url: &str, email: &str, api_token: &str) -> Self {
        let mut headers = reqwest::header::HeaderMap::new();

        let auth = base64::engine::general_purpose::STANDARD
            .encode(format!("{email}:{api_token}"));
        headers.insert(
            reqwest::header::AUTHORIZATION,
            format!("Basic {auth}").parse().expect("valid auth header"),
        );
        headers.insert(
            reqwest::header::ACCEPT,
            "application/json".parse().expect("valid accept header"),
        );
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            "application/json".parse().expect("valid content-type header"),
        );

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(30))
            .build()
            .expect("failed to build HTTP client");

        Self {
            http,
            base_url: instance_url.trim_end_matches('/').to_string(),
            auth_email: email.to_string(),
            auth_token: api_token.to_string(),
        }
    }

    /// Create a JiraClient with a custom reqwest::Client (for testing with mock servers).
    #[cfg(test)]
    pub fn with_http_client(http: reqwest::Client, base_url: &str) -> Self {
        Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            auth_email: String::new(),
            auth_token: String::new(),
        }
    }

    /// Return the base URL (for testing / display).
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    // ── Retry Wrapper ─────────────────────────────────────────────

    /// Execute a closure with exponential backoff retry on transient errors.
    ///
    /// Retries on:
    /// - HTTP 429 (rate limit)
    /// - HTTP 5xx (server error)
    /// - Network/transport errors (connection failures, timeouts)
    ///
    /// All other errors (401, 403, 404, 400) propagate immediately.
    async fn request_with_retry<F, Fut, T>(
        &self,
        max_retries: u32,
        f: F,
    ) -> Result<T>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        let mut backoff = Duration::from_secs(1);
        for attempt in 0..=max_retries {
            match f().await {
                Ok(result) => return Ok(result),
                Err(e) if attempt < max_retries && is_retryable(&e) => {
                    tracing::warn!(
                        attempt = attempt + 1,
                        max_retries,
                        backoff_ms = backoff.as_millis(),
                        error = %e,
                        "Jira request failed, retrying"
                    );
                    tokio::time::sleep(backoff).await;
                    backoff *= 2;
                }
                Err(e) => return Err(e),
            }
        }
        unreachable!()
    }

    // ── Read Operations (immediate, with retry) ───────────────────

    /// Search for issues using JQL.
    pub async fn search(&self, jql: &str) -> Result<Vec<JiraIssue>> {
        self.request_with_retry(DEFAULT_MAX_RETRIES, || async {
            let url = format!("{}/rest/api/3/search/jql", self.base_url);
            let resp = self
                .http
                .get(&url)
                .query(&[
                    ("jql", jql),
                    ("maxResults", "50"),
                    (
                        "fields",
                        "summary,status,assignee,priority,issuetype,project,labels,created,updated,description",
                    ),
                ])
                .send()
                .await?;
            let result: JiraSearchResult =
                self.handle_response(resp, "search").await?;
            Ok(result.issues)
        })
        .await
    }

    /// Fetch a single issue by key.
    pub async fn get_issue(&self, issue_key: &str) -> Result<JiraIssue> {
        self.request_with_retry(DEFAULT_MAX_RETRIES, || async {
            let url = format!("{}/rest/api/3/issue/{}", self.base_url, issue_key);
            let resp = self
                .http
                .get(&url)
                .query(&[(
                    "fields",
                    "summary,status,assignee,priority,issuetype,project,labels,created,updated,description,comment",
                )])
                .send()
                .await?;
            self.handle_response(resp, &format!("get issue {issue_key}"))
                .await
        })
        .await
    }

    /// Search Jira users by query string (for assignee resolution).
    pub async fn search_users(&self, query: &str) -> Result<Vec<JiraUser>> {
        self.request_with_retry(DEFAULT_MAX_RETRIES, || async {
            let url = format!("{}/rest/api/3/user/search", self.base_url);
            let resp = self
                .http
                .get(&url)
                .query(&[("query", query), ("maxResults", "10")])
                .send()
                .await?;
            self.handle_response(resp, &format!("user search '{query}'"))
                .await
        })
        .await
    }

    // ── Write Operations (called after confirmation, with retry) ──

    /// Create a new Jira issue.
    pub async fn create_issue(&self, params: &JiraCreateParams) -> Result<JiraCreatedIssue> {
        self.request_with_retry(DEFAULT_MAX_RETRIES, || async {
            let url = format!("{}/rest/api/3/issue", self.base_url);

            let mut fields = serde_json::json!({
                "project": { "key": params.project },
                "issuetype": { "name": params.issue_type },
                "summary": params.title,
            });

            if let Some(desc) = &params.description {
                fields["description"] = serde_json::json!({
                    "type": "doc",
                    "version": 1,
                    "content": [{
                        "type": "paragraph",
                        "content": [{
                            "type": "text",
                            "text": desc
                        }]
                    }]
                });
            }

            if let Some(priority) = &params.priority {
                fields["priority"] = serde_json::json!({ "name": priority });
            }

            if let Some(assignee_id) = &params.assignee_account_id {
                fields["assignee"] = serde_json::json!({ "accountId": assignee_id });
            }

            if let Some(labels) = &params.labels {
                fields["labels"] = serde_json::json!(labels);
            }

            let body = serde_json::json!({ "fields": fields });
            let resp = self.http.post(&url).json(&body).send().await?;
            self.handle_response(resp, "create issue").await
        })
        .await
    }

    /// Get available transitions for an issue.
    pub async fn get_transitions(&self, issue_key: &str) -> Result<Vec<JiraTransition>> {
        self.request_with_retry(DEFAULT_MAX_RETRIES, || async {
            let url = format!(
                "{}/rest/api/3/issue/{}/transitions",
                self.base_url, issue_key
            );
            let resp = self.http.get(&url).send().await?;
            let result: JiraTransitionsResponse = self
                .handle_response(resp, &format!("get transitions for {issue_key}"))
                .await?;
            Ok(result.transitions)
        })
        .await
    }

    /// Transition an issue to a new status (by transition name, case-insensitive).
    pub async fn transition_issue(
        &self,
        issue_key: &str,
        transition_name: &str,
    ) -> Result<()> {
        // Step 1: Get available transitions (already retry-wrapped via get_transitions)
        let transitions = self.get_transitions(issue_key).await?;

        // Step 2: Find matching transition (case-insensitive)
        let transition = transitions
            .iter()
            .find(|t| t.name.eq_ignore_ascii_case(transition_name))
            .ok_or_else(|| {
                let available: Vec<&str> =
                    transitions.iter().map(|t| t.name.as_str()).collect();
                anyhow::anyhow!(
                    "Transition '{}' not available for {}. Available: {:?}",
                    transition_name,
                    issue_key,
                    available
                )
            })?;

        let transition_id = transition.id.clone();

        // Step 3: Execute transition (with retry)
        self.request_with_retry(DEFAULT_MAX_RETRIES, || async {
            let url = format!(
                "{}/rest/api/3/issue/{}/transitions",
                self.base_url, issue_key
            );
            let body = serde_json::json!({
                "transition": { "id": &transition_id }
            });
            let resp = self.http.post(&url).json(&body).send().await?;
            self.handle_response_no_body(resp, &format!("transition {issue_key}"))
                .await
        })
        .await
    }

    /// Assign an issue to a user by account ID.
    pub async fn assign_issue(
        &self,
        issue_key: &str,
        assignee_account_id: &str,
    ) -> Result<()> {
        self.request_with_retry(DEFAULT_MAX_RETRIES, || async {
            let url = format!(
                "{}/rest/api/3/issue/{}/assignee",
                self.base_url, issue_key
            );
            let body = serde_json::json!({
                "accountId": assignee_account_id
            });
            let resp = self.http.put(&url).json(&body).send().await?;
            self.handle_response_no_body(resp, &format!("assign {issue_key}"))
                .await
        })
        .await
    }

    /// Add a comment to an issue. Comment body is plain text, converted to ADF.
    pub async fn add_comment(
        &self,
        issue_key: &str,
        comment_body: &str,
    ) -> Result<JiraComment> {
        self.request_with_retry(DEFAULT_MAX_RETRIES, || async {
            let url = format!(
                "{}/rest/api/3/issue/{}/comment",
                self.base_url, issue_key
            );
            let body = serde_json::json!({
                "body": {
                    "type": "doc",
                    "version": 1,
                    "content": [{
                        "type": "paragraph",
                        "content": [{
                            "type": "text",
                            "text": comment_body
                        }]
                    }]
                }
            });
            let resp = self.http.post(&url).json(&body).send().await?;
            self.handle_response(resp, &format!("comment on {issue_key}"))
                .await
        })
        .await
    }

    // ── Response Handling ──────────────────────────────────────────

    /// Parse a JSON response body, returning a descriptive error for
    /// non-success status codes.
    pub(crate) async fn handle_response<T: serde::de::DeserializeOwned>(
        &self,
        resp: reqwest::Response,
        context: &str,
    ) -> Result<T> {
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
            s if s >= 500 => anyhow::bail!("Jira server error ({status}). {context}: {error_body}"),
            _ => anyhow::bail!("Jira API error ({status}). {context}: {error_body}"),
        }
    }

    /// Handle a response that should have no body (204 No Content).
    async fn handle_response_no_body(
        &self,
        resp: reqwest::Response,
        context: &str,
    ) -> Result<()> {
        let status = resp.status();
        if status.is_success() {
            return Ok(());
        }
        let error_body = resp.text().await.unwrap_or_default();
        match status.as_u16() {
            401 => anyhow::bail!("Jira auth failed -- check API token. {context}"),
            403 => anyhow::bail!("Jira permission denied. {context}: {error_body}"),
            404 => anyhow::bail!("Jira resource not found. {context}"),
            429 => anyhow::bail!("Jira rate limit hit. {context}"),
            s if s >= 500 => anyhow::bail!("Jira server error ({status}). {context}: {error_body}"),
            _ => anyhow::bail!("Jira API error ({status}). {context}: {error_body}"),
        }
    }
}

// ── Retryable Error Detection ─────────────────────────────────────

/// Check whether an error is retryable (transient).
///
/// Returns `true` for:
/// - HTTP 429 (rate limit)
/// - HTTP 5xx (server errors)
/// - Network/transport errors (connection failures, timeouts)
pub(crate) fn is_retryable(e: &anyhow::Error) -> bool {
    let msg = e.to_string();
    // Rate limit
    if msg.contains("429") || msg.contains("rate limit") {
        return true;
    }
    // Server errors
    if msg.contains("server error")
        || msg.contains("500")
        || msg.contains("502")
        || msg.contains("503")
        || msg.contains("504")
    {
        return true;
    }
    // Network errors
    if msg.contains("connection")
        || msg.contains("timed out")
        || msg.contains("timeout")
        || msg.contains("dns")
        || msg.contains("reset by peer")
    {
        return true;
    }
    false
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_strips_trailing_slash() {
        let client = JiraClient::new(
            "https://example.atlassian.net/",
            "user@example.com",
            "fake-token",
        );
        assert_eq!(client.base_url(), "https://example.atlassian.net");
    }

    #[test]
    fn client_preserves_clean_url() {
        let client = JiraClient::new(
            "https://example.atlassian.net",
            "user@example.com",
            "fake-token",
        );
        assert_eq!(client.base_url(), "https://example.atlassian.net");
    }

    #[test]
    fn is_retryable_rate_limit() {
        let e = anyhow::anyhow!("Jira rate limit hit. 429");
        assert!(is_retryable(&e));
    }

    #[test]
    fn is_retryable_server_error() {
        assert!(is_retryable(&anyhow::anyhow!("Jira server error (500)")));
        assert!(is_retryable(&anyhow::anyhow!("502 Bad Gateway")));
        assert!(is_retryable(&anyhow::anyhow!("503 Service Unavailable")));
        assert!(is_retryable(&anyhow::anyhow!("504 Gateway Timeout")));
    }

    #[test]
    fn is_retryable_network_error() {
        assert!(is_retryable(&anyhow::anyhow!("connection refused")));
        assert!(is_retryable(&anyhow::anyhow!("request timed out")));
        assert!(is_retryable(&anyhow::anyhow!("dns lookup failed")));
    }

    #[test]
    fn is_not_retryable_client_error() {
        assert!(!is_retryable(&anyhow::anyhow!("Jira auth failed -- check API token. 401")));
        assert!(!is_retryable(&anyhow::anyhow!("Jira permission denied. 403")));
        assert!(!is_retryable(&anyhow::anyhow!("Jira resource not found. 404")));
    }
}
