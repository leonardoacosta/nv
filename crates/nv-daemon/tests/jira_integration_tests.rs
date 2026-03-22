//! Integration tests for the Jira client against a real Jira instance.
//!
//! Gated behind `NV_JIRA_INTEGRATION_TEST=1` environment variable.
//! Requires real credentials:
//! - `JIRA_INSTANCE_URL`: e.g. "https://yourorg.atlassian.net"
//! - `JIRA_EMAIL`: Jira user email
//! - `JIRA_API_TOKEN`: Jira API token
//! - `JIRA_TEST_PROJECT`: Project key for test issues (e.g. "OO")

/// Skip the test if integration testing is not enabled.
fn skip_unless_integration() -> bool {
    std::env::var("NV_JIRA_INTEGRATION_TEST").unwrap_or_default() != "1"
}

fn jira_env(key: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| panic!("{key} not set for integration test"))
}

/// Create a JiraClient from environment variables.
///
/// Duplicated here because `JiraClient` is internal to the binary crate.
struct IntegrationJiraClient {
    http: reqwest::Client,
    base_url: String,
}

impl IntegrationJiraClient {
    fn new() -> Self {
        let instance_url = jira_env("JIRA_INSTANCE_URL");
        let email = jira_env("JIRA_EMAIL");
        let api_token = jira_env("JIRA_API_TOKEN");

        let mut headers = reqwest::header::HeaderMap::new();
        let auth = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            format!("{email}:{api_token}"),
        );
        headers.insert(
            reqwest::header::AUTHORIZATION,
            format!("Basic {auth}").parse().unwrap(),
        );
        headers.insert(
            reqwest::header::ACCEPT,
            "application/json".parse().unwrap(),
        );
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap();

        Self {
            http,
            base_url: instance_url.trim_end_matches('/').to_string(),
        }
    }

    async fn search(&self, jql: &str) -> anyhow::Result<serde_json::Value> {
        let url = format!("{}/rest/api/3/search/jql", self.base_url);
        let resp = self
            .http
            .get(&url)
            .query(&[("jql", jql), ("maxResults", "5")])
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Search failed ({status}): {body}");
        }
        Ok(resp.json().await?)
    }

    async fn create_issue(
        &self,
        project: &str,
        issue_type: &str,
        title: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let url = format!("{}/rest/api/3/issue", self.base_url);
        let body = serde_json::json!({
            "fields": {
                "project": { "key": project },
                "issuetype": { "name": issue_type },
                "summary": title,
            }
        });
        let resp = self.http.post(&url).json(&body).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Create failed ({status}): {body}");
        }
        Ok(resp.json().await?)
    }

    async fn get_issue(&self, issue_key: &str) -> anyhow::Result<serde_json::Value> {
        let url = format!("{}/rest/api/3/issue/{issue_key}", self.base_url);
        let resp = self
            .http
            .get(&url)
            .query(&[("fields", "summary,status,comment")])
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Get issue failed ({status}): {body}");
        }
        Ok(resp.json().await?)
    }

    async fn transition_issue(
        &self,
        issue_key: &str,
        transition_name: &str,
    ) -> anyhow::Result<()> {
        // Get available transitions
        let url = format!(
            "{}/rest/api/3/issue/{issue_key}/transitions",
            self.base_url
        );
        let resp = self.http.get(&url).send().await?;
        let transitions: serde_json::Value = resp.json().await?;
        let transition = transitions["transitions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| {
                t["name"]
                    .as_str()
                    .unwrap()
                    .eq_ignore_ascii_case(transition_name)
            })
            .ok_or_else(|| anyhow::anyhow!("Transition '{transition_name}' not found"))?;

        let transition_id = transition["id"].as_str().unwrap();

        // Execute transition
        let url = format!(
            "{}/rest/api/3/issue/{issue_key}/transitions",
            self.base_url
        );
        let body = serde_json::json!({
            "transition": { "id": transition_id }
        });
        let resp = self.http.post(&url).json(&body).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Transition failed ({status}): {body}");
        }
        Ok(())
    }

    async fn add_comment(
        &self,
        issue_key: &str,
        comment_text: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let url = format!(
            "{}/rest/api/3/issue/{issue_key}/comment",
            self.base_url
        );
        let body = serde_json::json!({
            "body": {
                "type": "doc",
                "version": 1,
                "content": [{
                    "type": "paragraph",
                    "content": [{"type": "text", "text": comment_text}]
                }]
            }
        });
        let resp = self.http.post(&url).json(&body).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Add comment failed ({status}): {body}");
        }
        Ok(resp.json().await?)
    }
}

#[tokio::test]
async fn integration_search_issues() {
    if skip_unless_integration() {
        eprintln!("Skipping integration test (NV_JIRA_INTEGRATION_TEST not set)");
        return;
    }

    let client = IntegrationJiraClient::new();
    let project = jira_env("JIRA_TEST_PROJECT");

    let result = client
        .search(&format!("project = {project} ORDER BY created DESC"))
        .await;

    match result {
        Ok(data) => {
            assert!(data.get("total").is_some(), "Response should have 'total'");
            assert!(
                data.get("issues").is_some(),
                "Response should have 'issues'"
            );
            let total = data["total"].as_u64().unwrap_or(0);
            eprintln!("Integration test: found {total} issues in project {project}");
        }
        Err(e) => {
            panic!("Integration search failed: {e}");
        }
    }
}

#[tokio::test]
async fn integration_full_create_flow() {
    if skip_unless_integration() {
        eprintln!("Skipping integration test (NV_JIRA_INTEGRATION_TEST not set)");
        return;
    }

    let client = IntegrationJiraClient::new();
    let project = jira_env("JIRA_TEST_PROJECT");
    let timestamp = chrono::Utc::now().format("%Y%m%d%H%M%S");
    let title = format!("[NV Integration Test] Auto-created {timestamp}");

    // Step 1: Create issue
    let created = client.create_issue(&project, "Task", &title).await.unwrap();
    let issue_key = created["key"].as_str().unwrap();
    eprintln!("Created issue: {issue_key}");
    assert!(
        issue_key.starts_with(&project),
        "Key should start with project prefix"
    );

    // Step 2: Transition to "In Progress" (if available)
    match client.transition_issue(issue_key, "In Progress").await {
        Ok(()) => eprintln!("Transitioned {issue_key} to In Progress"),
        Err(e) => eprintln!("Transition to In Progress failed (may not be available): {e}"),
    }

    // Step 3: Add a comment
    let comment = client
        .add_comment(issue_key, "Integration test comment from NV daemon")
        .await
        .unwrap();
    let comment_id = comment["id"].as_str().unwrap();
    eprintln!("Added comment {comment_id} to {issue_key}");

    // Step 4: Verify via get_issue
    let issue = client.get_issue(issue_key).await.unwrap();
    assert_eq!(issue["key"], issue_key);
    let summary = issue["fields"]["summary"].as_str().unwrap();
    assert!(summary.contains("NV Integration Test"));

    // Check comment exists
    if let Some(comment_page) = issue["fields"]["comment"].as_object() {
        let total = comment_page
            .get("total")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        assert!(total > 0, "Should have at least one comment");
        eprintln!("Verified: {issue_key} has {total} comment(s)");
    }

    eprintln!("Integration test complete: {issue_key}");
}
