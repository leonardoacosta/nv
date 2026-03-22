use std::time::Duration;

use anyhow::Result;
use base64::Engine;

use super::types::*;

// ── JiraClient ─────────────────────────────────────────────────────

/// HTTP client for the Jira REST API v3.
///
/// Uses Basic auth (email + API token). Read operations execute
/// immediately; write operations are called only after PendingAction
/// confirmation.
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

    /// Return the base URL (for testing / display).
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    // ── Read Operations (immediate) ────────────────────────────────

    /// Search for issues using JQL.
    pub async fn search(&self, jql: &str) -> Result<Vec<JiraIssue>> {
        // Atlassian migrated search to /rest/api/3/search/jql (the old
        // /rest/api/3/search endpoint returns 410 Gone on Jira Cloud).
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
    }

    /// Fetch a single issue by key.
    pub async fn get_issue(&self, issue_key: &str) -> Result<JiraIssue> {
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
    }

    /// Search Jira users by query string (for assignee resolution).
    pub async fn search_users(&self, query: &str) -> Result<Vec<JiraUser>> {
        let url = format!("{}/rest/api/3/user/search", self.base_url);
        let resp = self
            .http
            .get(&url)
            .query(&[("query", query), ("maxResults", "10")])
            .send()
            .await?;
        self.handle_response(resp, &format!("user search '{query}'"))
            .await
    }

    // ── Write Operations (called after confirmation) ───────────────

    /// Create a new Jira issue.
    pub async fn create_issue(&self, params: &JiraCreateParams) -> Result<JiraCreatedIssue> {
        let url = format!("{}/rest/api/3/issue", self.base_url);

        let mut fields = serde_json::json!({
            "project": { "key": params.project },
            "issuetype": { "name": params.issue_type },
            "summary": params.title,
        });

        // Description uses Atlassian Document Format (ADF)
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
    }

    /// Get available transitions for an issue.
    pub async fn get_transitions(&self, issue_key: &str) -> Result<Vec<JiraTransition>> {
        let url = format!(
            "{}/rest/api/3/issue/{}/transitions",
            self.base_url, issue_key
        );
        let resp = self.http.get(&url).send().await?;
        let result: JiraTransitionsResponse = self
            .handle_response(resp, &format!("get transitions for {issue_key}"))
            .await?;
        Ok(result.transitions)
    }

    /// Transition an issue to a new status (by transition name, case-insensitive).
    pub async fn transition_issue(
        &self,
        issue_key: &str,
        transition_name: &str,
    ) -> Result<()> {
        // Step 1: Get available transitions
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

        // Step 3: Execute transition
        let url = format!(
            "{}/rest/api/3/issue/{}/transitions",
            self.base_url, issue_key
        );
        let body = serde_json::json!({
            "transition": { "id": transition.id }
        });
        let resp = self.http.post(&url).json(&body).send().await?;
        self.handle_response_no_body(resp, &format!("transition {issue_key}"))
            .await
    }

    /// Assign an issue to a user by account ID.
    pub async fn assign_issue(
        &self,
        issue_key: &str,
        assignee_account_id: &str,
    ) -> Result<()> {
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
    }

    /// Add a comment to an issue. Comment body is plain text, converted to ADF.
    pub async fn add_comment(
        &self,
        issue_key: &str,
        comment_body: &str,
    ) -> Result<JiraComment> {
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
    }

    // ── Response Handling ──────────────────────────────────────────

    /// Parse a JSON response body, returning a descriptive error for
    /// non-success status codes.
    async fn handle_response<T: serde::de::DeserializeOwned>(
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
            _ => anyhow::bail!("Jira API error ({status}). {context}: {error_body}"),
        }
    }
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
}
