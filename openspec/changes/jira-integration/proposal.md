# jira-integration

## Summary

Implement a Jira REST API v3 client exposing agent tools for issue search, creation, transitions, assignment, and commenting. All write operations go through the PendingAction confirmation flow — NV drafts the action, presents it on Telegram with an inline keyboard, and only executes after user approval. Read operations (search, get) execute immediately.

## Motivation

NV needs to manage Jira issues as part of its operational loop. The digest system (spec-7) will query open issues, the agent loop will create/transition issues in response to user commands, and the context query system (spec-8) will search across Jira for answers. This spec provides the foundational Jira tooling that downstream specs depend on.

The PendingAction confirmation flow is critical — NV should never create, modify, or transition Jira issues without explicit user approval via Telegram.

## Design

### Module Structure

```
crates/nv-daemon/src/
├── main.rs
└── jira/
    ├── mod.rs          # JiraClient struct, auth, config
    ├── types.rs        # Jira API request/response types
    └── tools.rs        # Agent tool functions (jira_search, jira_create, etc.)
```

The Jira module lives in nv-daemon alongside the Telegram module — it's a runtime integration, not a shared type.

### JiraClient Struct

```rust
pub struct JiraClient {
    http: reqwest::Client,
    base_url: String,      // e.g. "https://yourorg.atlassian.net"
    auth_email: String,
    auth_token: String,
}
```

- `http`: Shared reqwest client with default headers (Accept: application/json, Content-Type: application/json)
- `base_url`: Jira Cloud instance URL from config, no trailing slash
- `auth_email` + `auth_token`: Basic auth credentials — base64-encoded `email:token` in Authorization header

### Authentication

Jira Cloud uses Basic auth with email + API token (not password). Every request includes:

```
Authorization: Basic base64({email}:{api_token})
```

```rust
impl JiraClient {
    pub fn new(instance_url: &str, email: &str, api_token: &str) -> Self {
        let mut headers = reqwest::header::HeaderMap::new();
        let auth = base64::engine::general_purpose::STANDARD
            .encode(format!("{email}:{api_token}"));
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
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to build HTTP client");

        Self {
            http,
            base_url: instance_url.trim_end_matches('/').to_string(),
            auth_email: email.to_string(),
            auth_token: api_token.to_string(),
        }
    }
}
```

### Tool: jira_search(jql)

Executes a JQL query and returns parsed issues. Read-only — no confirmation needed.

```rust
pub async fn jira_search(&self, jql: &str) -> anyhow::Result<Vec<JiraIssue>> {
    let url = format!("{}/rest/api/3/search", self.base_url);
    let body = serde_json::json!({
        "jql": jql,
        "maxResults": 50,
        "fields": [
            "summary", "status", "assignee", "priority",
            "issuetype", "project", "labels", "created", "updated",
            "description"
        ]
    });
    let resp = self.http.post(&url).json(&body).send().await?;
    let status = resp.status();
    if !status.is_success() {
        let error_body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Jira search failed ({}): {}", status, error_body);
    }
    let search_result: JiraSearchResult = resp.json().await?;
    Ok(search_result.issues)
}
```

- `maxResults`: 50 default, sufficient for agent context windows
- `fields`: Only fetch fields NV needs — keeps response compact
- Returns `Vec<JiraIssue>` with key, summary, status, assignee, priority, labels

### Tool: jira_get(issue_key)

Fetches a single issue by key. Read-only — no confirmation needed.

```rust
pub async fn jira_get(&self, issue_key: &str) -> anyhow::Result<JiraIssue> {
    let url = format!(
        "{}/rest/api/3/issue/{}",
        self.base_url, issue_key
    );
    let resp = self.http.get(&url)
        .query(&[("fields", "summary,status,assignee,priority,issuetype,project,labels,created,updated,description,comment")])
        .send()
        .await?;
    let status = resp.status();
    if !status.is_success() {
        let error_body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Jira get issue failed ({}): {}", status, error_body);
    }
    let issue: JiraIssue = resp.json().await?;
    Ok(issue)
}
```

- Includes `comment` field for full issue context
- Used by agent when user asks about a specific issue

### Tool: jira_create(project, type, title, description, priority, assignee, labels)

Creates a new Jira issue. **Requires Telegram confirmation.**

```rust
pub async fn jira_create(&self, params: &JiraCreateParams) -> anyhow::Result<JiraCreatedIssue> {
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

    // Priority — requires name-to-ID resolution
    if let Some(priority) = &params.priority {
        fields["priority"] = serde_json::json!({ "name": priority });
    }

    // Assignee — requires account ID (not display name)
    if let Some(assignee_id) = &params.assignee_account_id {
        fields["assignee"] = serde_json::json!({ "accountId": assignee_id });
    }

    if let Some(labels) = &params.labels {
        fields["labels"] = serde_json::json!(labels);
    }

    let body = serde_json::json!({ "fields": fields });

    let resp = self.http.post(&url).json(&body).send().await?;
    let status = resp.status();
    if !status.is_success() {
        let error_body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Jira create issue failed ({}): {}", status, error_body);
    }
    let created: JiraCreatedIssue = resp.json().await?;
    Ok(created)
}
```

**Important details:**
- Description must use Atlassian Document Format (ADF), not plain text — Jira API v3 requires this
- Priority accepts a name string (e.g., "Highest", "High", "Medium", "Low", "Lowest") — Jira resolves to ID internally
- Assignee requires an `accountId`, not a display name — the agent should resolve display names via user search or use cached mappings

### Tool: jira_transition(issue_key, transition_name)

Transitions an issue to a new status. **Requires Telegram confirmation.** Two-step process: first GET available transitions, then POST the matching one.

```rust
pub async fn jira_transition(
    &self,
    issue_key: &str,
    transition_name: &str,
) -> anyhow::Result<()> {
    // Step 1: Get available transitions
    let url = format!(
        "{}/rest/api/3/issue/{}/transitions",
        self.base_url, issue_key
    );
    let resp = self.http.get(&url).send().await?;
    let status = resp.status();
    if !status.is_success() {
        let error_body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Jira get transitions failed ({}): {}", status, error_body);
    }
    let transitions: JiraTransitionsResponse = resp.json().await?;

    // Step 2: Find matching transition (case-insensitive)
    let transition = transitions.transitions.iter()
        .find(|t| t.name.eq_ignore_ascii_case(transition_name))
        .ok_or_else(|| {
            let available: Vec<&str> = transitions.transitions.iter()
                .map(|t| t.name.as_str())
                .collect();
            anyhow::anyhow!(
                "Transition '{}' not available for {}. Available: {:?}",
                transition_name, issue_key, available
            )
        })?;

    // Step 3: Execute transition
    let body = serde_json::json!({
        "transition": { "id": transition.id }
    });
    let resp = self.http.post(&url).json(&body).send().await?;
    let status = resp.status();
    if !status.is_success() {
        let error_body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Jira transition failed ({}): {}", status, error_body);
    }
    Ok(())
}
```

- Case-insensitive matching on transition name (e.g., "In Progress", "in progress", "IN PROGRESS")
- Error message lists available transitions when the requested one doesn't exist — helps the agent self-correct
- Transition IDs are workflow-specific and cannot be hardcoded

### Tool: jira_assign(issue_key, assignee)

Assigns an issue to a user. **Requires Telegram confirmation.**

```rust
pub async fn jira_assign(
    &self,
    issue_key: &str,
    assignee_account_id: &str,
) -> anyhow::Result<()> {
    let url = format!(
        "{}/rest/api/3/issue/{}/assignee",
        self.base_url, issue_key
    );
    let body = serde_json::json!({
        "accountId": assignee_account_id
    });
    let resp = self.http.put(&url).json(&body).send().await?;
    let status = resp.status();
    if !status.is_success() {
        let error_body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Jira assign failed ({}): {}", status, error_body);
    }
    Ok(())
}
```

- Uses the dedicated assignee endpoint (PUT `/rest/api/3/issue/{key}/assignee`), not the generic issue update
- Requires `accountId` — agent must resolve display names to account IDs

### Tool: jira_comment(issue_key, body)

Adds a comment to an issue. **Requires Telegram confirmation.**

```rust
pub async fn jira_comment(
    &self,
    issue_key: &str,
    comment_body: &str,
) -> anyhow::Result<JiraComment> {
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
    let status = resp.status();
    if !status.is_success() {
        let error_body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Jira comment failed ({}): {}", status, error_body);
    }
    let comment: JiraComment = resp.json().await?;
    Ok(comment)
}
```

- Comment body uses ADF format, same as issue description
- Returns the created comment (includes ID for reference)

### Jira API Types

```rust
// --- Request Types ---

#[derive(Debug, Serialize)]
pub struct JiraCreateParams {
    pub project: String,           // Project key, e.g. "OO"
    pub issue_type: String,        // e.g. "Bug", "Task", "Story"
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<String>,  // e.g. "Highest", "High", "Medium", "Low", "Lowest"
    pub assignee_account_id: Option<String>,
    pub labels: Option<Vec<String>>,
}

// --- Response Types ---

#[derive(Debug, Deserialize)]
pub struct JiraSearchResult {
    pub total: u32,
    pub max_results: u32,
    pub issues: Vec<JiraIssue>,
}

#[derive(Debug, Deserialize)]
pub struct JiraIssue {
    pub id: String,
    pub key: String,              // e.g. "OO-123"
    pub fields: JiraIssueFields,
}

#[derive(Debug, Deserialize)]
pub struct JiraIssueFields {
    pub summary: String,
    pub status: JiraStatus,
    pub assignee: Option<JiraUser>,
    pub priority: Option<JiraPriority>,
    pub issuetype: JiraIssueType,
    pub project: JiraProject,
    pub labels: Vec<String>,
    pub created: String,
    pub updated: String,
    pub description: Option<serde_json::Value>,  // ADF document
    pub comment: Option<JiraCommentPage>,
}

#[derive(Debug, Deserialize)]
pub struct JiraStatus {
    pub name: String,             // e.g. "To Do", "In Progress", "Done"
    pub id: String,
}

#[derive(Debug, Deserialize)]
pub struct JiraUser {
    #[serde(rename = "accountId")]
    pub account_id: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
}

#[derive(Debug, Deserialize)]
pub struct JiraPriority {
    pub name: String,             // "Highest", "High", "Medium", "Low", "Lowest"
    pub id: String,
}

#[derive(Debug, Deserialize)]
pub struct JiraIssueType {
    pub name: String,             // "Bug", "Task", "Story", "Epic"
}

#[derive(Debug, Deserialize)]
pub struct JiraProject {
    pub key: String,              // "OO", "TC", etc.
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct JiraCreatedIssue {
    pub id: String,
    pub key: String,
    #[serde(rename = "self")]
    pub self_url: String,
}

#[derive(Debug, Deserialize)]
pub struct JiraTransitionsResponse {
    pub transitions: Vec<JiraTransition>,
}

#[derive(Debug, Deserialize)]
pub struct JiraTransition {
    pub id: String,
    pub name: String,             // "To Do", "In Progress", "Done", etc.
}

#[derive(Debug, Deserialize)]
pub struct JiraComment {
    pub id: String,
    pub body: serde_json::Value,  // ADF document
    pub created: String,
    pub author: JiraUser,
}

#[derive(Debug, Deserialize)]
pub struct JiraCommentPage {
    pub total: u32,
    pub comments: Vec<JiraComment>,
}
```

### PendingAction Confirmation Flow

All write operations (create, transition, assign, comment) follow the same flow:

```
User: "Create a P1 bug on OO: Login fails on Safari"
  │
  ▼
Agent Loop: Claude parses intent, calls jira_create tool
  │
  ▼
Tool returns PendingAction (not executed yet):
  PendingAction {
      id: uuid,
      action_type: "jira_create",
      description: "Create Bug on OO: Login fails on Safari (P1)",
      payload: JiraCreateParams { ... },
      status: Pending,
  }
  │
  ▼
Agent sends Telegram message with draft summary + inline keyboard:
  "📋 Create Jira Issue
   Project: OO | Type: Bug | Priority: Highest
   Title: Login fails on Safari
   Description: (none)
   Assignee: (unassigned)
   Labels: (none)

   [✅ Create]  [✏️ Edit]  [❌ Cancel]"
  │
  ├─ User taps ✅ Create → callback "approve:{uuid}"
  │   → Execute jira_create with stored payload
  │   → Edit Telegram message: "✅ Created OO-456: Login fails on Safari"
  │   → Store in memory: "Created OO-456 — Login fails on Safari (P1 Bug)"
  │
  ├─ User taps ✏️ Edit → callback "edit:{uuid}"
  │   → Agent asks: "What would you like to change?"
  │   → User responds with edits
  │   → Agent updates PendingAction payload, sends new draft
  │
  └─ User taps ❌ Cancel → callback "cancel:{uuid}"
      → Remove PendingAction from pending list
      → Edit Telegram message: "❌ Cancelled: Create Bug on OO"
```

### PendingAction Storage

Pending actions are stored in `~/.nv/state/pending-actions.json` (from spec-5 memory-system). Each action has a TTL — uncofirmed actions expire after 1 hour.

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct JiraPendingAction {
    pub id: String,              // UUID
    pub action_type: JiraActionType,
    pub description: String,     // Human-readable summary for Telegram
    pub payload: serde_json::Value, // Serialized tool params
    pub status: PendingActionStatus,
    pub created_at: DateTime<Utc>,
    pub telegram_message_id: Option<i64>, // For editing the confirmation message
}

#[derive(Debug, Serialize, Deserialize)]
pub enum JiraActionType {
    Create,
    Transition,
    Assign,
    Comment,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PendingActionStatus {
    Pending,
    Approved,
    Cancelled,
    Expired,
}
```

### Error Handling

| Error | HTTP Status | Handling |
|-------|-------------|----------|
| Auth failure | 401 | Log error, surface to Telegram: "Jira auth failed — check API token" |
| Permission denied | 403 | Surface to Telegram: "No permission for {action} on {issue_key}" |
| Issue not found | 404 | Surface to Telegram: "Issue {key} not found" |
| Field validation | 400 | Parse error messages from response body, surface specific field errors |
| Rate limit | 429 | Retry with backoff (Retry-After header), max 3 retries |
| Server error | 5xx | Retry with exponential backoff, max 3 retries, then surface error |
| Network error | — | Retry with exponential backoff, max 3 retries |

```rust
impl JiraClient {
    async fn handle_response<T: DeserializeOwned>(
        &self,
        resp: reqwest::Response,
        context: &str,
    ) -> anyhow::Result<T> {
        let status = resp.status();
        if status.is_success() {
            return Ok(resp.json().await?);
        }
        let error_body = resp.text().await.unwrap_or_default();
        match status.as_u16() {
            401 => anyhow::bail!("Jira auth failed — check API token. {context}"),
            403 => anyhow::bail!("Jira permission denied. {context}: {error_body}"),
            404 => anyhow::bail!("Jira resource not found. {context}"),
            429 => anyhow::bail!("Jira rate limit hit. {context}"),
            _ => anyhow::bail!("Jira API error ({status}). {context}: {error_body}"),
        }
    }
}
```

Rate limit retry logic lives in a wrapper around the HTTP calls:

```rust
async fn request_with_retry<F, Fut, T>(
    &self,
    max_retries: u32,
    f: F,
) -> anyhow::Result<T>
where
    F: Fn() -> Fut,
    Fut: Future<Output = anyhow::Result<T>>,
{
    let mut backoff = Duration::from_secs(1);
    for attempt in 0..=max_retries {
        match f().await {
            Ok(result) => return Ok(result),
            Err(e) if attempt < max_retries => {
                let is_retryable = e.to_string().contains("rate limit")
                    || e.to_string().contains("429")
                    || e.to_string().contains("5");
                if !is_retryable {
                    return Err(e);
                }
                tracing::warn!(
                    "Jira request failed (attempt {}/{}): {e}, retrying in {backoff:?}",
                    attempt + 1, max_retries
                );
                tokio::time::sleep(backoff).await;
                backoff *= 2;
            }
            Err(e) => return Err(e),
        }
    }
    unreachable!()
}
```

### Jira Field Mapping

Priority and transition names vary across Jira instances. The agent handles this by:

1. **Priority**: Passing the name directly (e.g., "Highest") — Jira API v3 accepts priority by name in the `fields.priority.name` field
2. **Transitions**: Fetching available transitions at runtime (GET `/transitions`) and matching by name (case-insensitive)
3. **Assignee**: Requiring `accountId`, not display name. The agent should maintain a lightweight cache of team members (accountId ↔ display name) in memory, populated on first use via `/rest/api/3/user/search?query=`

### User Search (for Assignee Resolution)

```rust
pub async fn search_users(&self, query: &str) -> anyhow::Result<Vec<JiraUser>> {
    let url = format!("{}/rest/api/3/user/search", self.base_url);
    let resp = self.http.get(&url)
        .query(&[("query", query), ("maxResults", "10")])
        .send()
        .await?;
    let status = resp.status();
    if !status.is_success() {
        let error_body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Jira user search failed ({}): {}", status, error_body);
    }
    let users: Vec<JiraUser> = resp.json().await?;
    Ok(users)
}
```

### Config and Secrets

Config from `~/.nv/nv.toml`:

```toml
[jira]
instance_url = "https://yourorg.atlassian.net"
default_project = "OO"
```

Secrets from environment variables:

```
NV_JIRA_EMAIL=user@example.com
NV_JIRA_API_TOKEN=ATATT3xFfGF0...
```

### Agent Tool Registration

The tools are registered in the agent loop's system prompt (spec-4) as Claude tool definitions:

```rust
pub fn jira_tool_definitions() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "name": "jira_search",
            "description": "Search Jira issues using JQL. Returns matching issues with key, summary, status, assignee, priority.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "jql": { "type": "string", "description": "JQL query string" }
                },
                "required": ["jql"]
            }
        }),
        serde_json::json!({
            "name": "jira_get",
            "description": "Get a single Jira issue by key. Returns full issue details including comments.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "issue_key": { "type": "string", "description": "Issue key, e.g. OO-123" }
                },
                "required": ["issue_key"]
            }
        }),
        serde_json::json!({
            "name": "jira_create",
            "description": "Create a new Jira issue. Requires confirmation via Telegram before execution.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "project": { "type": "string", "description": "Project key, e.g. OO" },
                    "issue_type": { "type": "string", "description": "Issue type: Bug, Task, Story, Epic" },
                    "title": { "type": "string", "description": "Issue summary" },
                    "description": { "type": "string", "description": "Issue description (plain text, converted to ADF)" },
                    "priority": { "type": "string", "description": "Priority name: Highest, High, Medium, Low, Lowest" },
                    "assignee": { "type": "string", "description": "Assignee display name (resolved to accountId)" },
                    "labels": { "type": "array", "items": { "type": "string" }, "description": "Issue labels" }
                },
                "required": ["project", "issue_type", "title"]
            }
        }),
        serde_json::json!({
            "name": "jira_transition",
            "description": "Transition a Jira issue to a new status. Requires confirmation via Telegram before execution.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "issue_key": { "type": "string", "description": "Issue key, e.g. OO-123" },
                    "transition_name": { "type": "string", "description": "Target status name, e.g. In Progress, Done" }
                },
                "required": ["issue_key", "transition_name"]
            }
        }),
        serde_json::json!({
            "name": "jira_assign",
            "description": "Assign a Jira issue to a user. Requires confirmation via Telegram before execution.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "issue_key": { "type": "string", "description": "Issue key, e.g. OO-123" },
                    "assignee": { "type": "string", "description": "Assignee display name (resolved to accountId)" }
                },
                "required": ["issue_key", "assignee"]
            }
        }),
        serde_json::json!({
            "name": "jira_comment",
            "description": "Add a comment to a Jira issue. Requires confirmation via Telegram before execution.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "issue_key": { "type": "string", "description": "Issue key, e.g. OO-123" },
                    "body": { "type": "string", "description": "Comment text (plain text, converted to ADF)" }
                },
                "required": ["issue_key", "body"]
            }
        }),
    ]
}
```

### Integration with Agent Loop

The agent loop (spec-4) dispatches tool calls from Claude's response:

```rust
// In agent loop tool dispatch:
match tool_name {
    "jira_search" => {
        let jql = input["jql"].as_str().unwrap();
        let issues = jira_client.jira_search(jql).await?;
        // Return issues as tool result (immediate, no confirmation)
        format_issues_for_claude(&issues)
    }
    "jira_get" => {
        let key = input["issue_key"].as_str().unwrap();
        let issue = jira_client.jira_get(key).await?;
        // Return issue as tool result (immediate, no confirmation)
        format_issue_for_claude(&issue)
    }
    "jira_create" | "jira_transition" | "jira_assign" | "jira_comment" => {
        // Write operation — create PendingAction, send Telegram confirmation
        let action = create_pending_action(tool_name, &input);
        save_pending_action(&action).await?;
        send_confirmation_keyboard(&telegram, &action).await?;
        // Return "Awaiting confirmation" as tool result
        format!("Action queued for confirmation: {}", action.description)
    }
    _ => { /* ... */ }
}
```

When a callback query arrives with `approve:{uuid}`, the agent loop:

1. Loads the PendingAction from state
2. Executes the stored payload against JiraClient
3. Edits the original Telegram message with the result
4. Stores the outcome in memory

### Daemon Integration

```rust
// In main.rs:
let jira_client = if let (Some(jira_config), Some(email), Some(token)) = (
    &config.jira,
    &secrets.jira_email,
    &secrets.jira_api_token,
) {
    Some(JiraClient::new(
        &jira_config.instance_url,
        email,
        token,
    ))
} else {
    tracing::warn!("Jira not configured — jira tools disabled");
    None
};
// Pass jira_client into agent loop
```

## Verification

- `cargo build` succeeds
- `cargo test` — unit tests for types serialization, error handling, field mapping
- `cargo clippy` passes with no warnings
- Manual gate: "Create a P1 bug on OO" via Telegram → draft shown with inline keyboard → confirm → issue exists in Jira
