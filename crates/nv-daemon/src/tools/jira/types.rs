use serde::{Deserialize, Serialize};

// ── Request Types ──────────────────────────────────────────────────

/// Parameters for creating a new Jira issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraCreateParams {
    /// Project key, e.g. "OO"
    pub project: String,
    /// Issue type: Bug, Task, Story, Epic
    pub issue_type: String,
    /// Issue summary / title
    pub title: String,
    /// Optional description (plain text, converted to ADF on send)
    pub description: Option<String>,
    /// Priority name: Highest, High, Medium, Low, Lowest
    pub priority: Option<String>,
    /// Jira account ID for the assignee
    pub assignee_account_id: Option<String>,
    /// Issue labels
    pub labels: Option<Vec<String>>,
}

/// Parameters for transitioning a Jira issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraTransitionParams {
    pub issue_key: String,
    pub transition_name: String,
}

/// Parameters for assigning a Jira issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraAssignParams {
    pub issue_key: String,
    pub assignee_account_id: String,
}

/// Parameters for adding a comment to a Jira issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraCommentParams {
    pub issue_key: String,
    pub body: String,
}

// ── Response Types ─────────────────────────────────────────────────

/// Top-level search response from `/rest/api/3/search`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JiraSearchResult {
    pub total: u32,
    pub max_results: u32,
    pub issues: Vec<JiraIssue>,
}

/// A single Jira issue.
#[derive(Debug, Clone, Deserialize)]
pub struct JiraIssue {
    pub id: String,
    /// Issue key, e.g. "OO-123"
    pub key: String,
    pub fields: JiraIssueFields,
}

/// Fields on a Jira issue.
#[derive(Debug, Clone, Deserialize)]
pub struct JiraIssueFields {
    pub summary: String,
    pub status: JiraStatus,
    pub assignee: Option<JiraUser>,
    pub priority: Option<JiraPriority>,
    pub issuetype: JiraIssueType,
    pub project: JiraProject,
    #[serde(default)]
    pub labels: Vec<String>,
    pub created: String,
    pub updated: String,
    /// Description in Atlassian Document Format (ADF)
    pub description: Option<serde_json::Value>,
    /// Comments page (only present when explicitly requested)
    pub comment: Option<JiraCommentPage>,
}

/// Issue status.
#[derive(Debug, Clone, Deserialize)]
pub struct JiraStatus {
    pub name: String,
    pub id: String,
}

/// Jira user reference.
#[derive(Debug, Clone, Deserialize)]
pub struct JiraUser {
    #[serde(rename = "accountId")]
    pub account_id: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
}

/// Issue priority.
#[derive(Debug, Clone, Deserialize)]
pub struct JiraPriority {
    pub name: String,
    pub id: String,
}

/// Issue type (Bug, Task, Story, Epic, etc.).
#[derive(Debug, Clone, Deserialize)]
pub struct JiraIssueType {
    pub name: String,
}

/// Project reference.
#[derive(Debug, Clone, Deserialize)]
pub struct JiraProject {
    pub key: String,
    pub name: String,
}

/// Response from POST `/rest/api/3/issue`.
#[derive(Debug, Clone, Deserialize)]
pub struct JiraCreatedIssue {
    pub id: String,
    pub key: String,
    #[serde(rename = "self")]
    pub self_url: String,
}

/// Response from GET `/rest/api/3/issue/{key}/transitions`.
#[derive(Debug, Clone, Deserialize)]
pub struct JiraTransitionsResponse {
    pub transitions: Vec<JiraTransition>,
}

/// A single available transition.
#[derive(Debug, Clone, Deserialize)]
pub struct JiraTransition {
    pub id: String,
    pub name: String,
}

/// A Jira comment.
#[derive(Debug, Clone, Deserialize)]
pub struct JiraComment {
    pub id: String,
    /// Comment body in ADF
    pub body: serde_json::Value,
    pub created: String,
    pub author: JiraUser,
}

/// Paginated comment list.
#[derive(Debug, Clone, Deserialize)]
pub struct JiraCommentPage {
    pub total: u32,
    pub comments: Vec<JiraComment>,
}

// ── Jira Action Types (for PendingAction integration) ──────────────

/// The type of Jira write action pending confirmation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JiraActionType {
    Create,
    Transition,
    Assign,
    Comment,
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_params_serializes_all_fields() {
        let params = JiraCreateParams {
            project: "OO".into(),
            issue_type: "Bug".into(),
            title: "Login fails on Safari".into(),
            description: Some("Users cannot login using Safari 17".into()),
            priority: Some("Highest".into()),
            assignee_account_id: Some("5b10ac8d82e05b22cc7d4ef5".into()),
            labels: Some(vec!["browser".into(), "auth".into()]),
        };

        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["project"], "OO");
        assert_eq!(json["issue_type"], "Bug");
        assert_eq!(json["title"], "Login fails on Safari");
        assert_eq!(json["description"], "Users cannot login using Safari 17");
        assert_eq!(json["priority"], "Highest");
        assert_eq!(json["assignee_account_id"], "5b10ac8d82e05b22cc7d4ef5");
        let labels = json["labels"].as_array().unwrap();
        assert_eq!(labels.len(), 2);
    }

    #[test]
    fn create_params_serializes_required_only() {
        let params = JiraCreateParams {
            project: "TC".into(),
            issue_type: "Task".into(),
            title: "Set up CI pipeline".into(),
            description: None,
            priority: None,
            assignee_account_id: None,
            labels: None,
        };

        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["project"], "TC");
        assert_eq!(json["issue_type"], "Task");
        assert_eq!(json["title"], "Set up CI pipeline");
        assert!(json["description"].is_null());
        assert!(json["priority"].is_null());
    }

    #[test]
    fn jira_issue_deserializes_from_api_response() {
        let json = serde_json::json!({
            "id": "10001",
            "key": "OO-42",
            "fields": {
                "summary": "Fix login flow",
                "status": {"name": "In Progress", "id": "3"},
                "assignee": {
                    "accountId": "5b10ac8d82e05b22cc7d4ef5",
                    "displayName": "Leo Acosta"
                },
                "priority": {"name": "High", "id": "2"},
                "issuetype": {"name": "Bug"},
                "project": {"key": "OO", "name": "Otaku Odyssey"},
                "labels": ["frontend", "auth"],
                "created": "2026-03-20T10:00:00.000+0000",
                "updated": "2026-03-21T14:30:00.000+0000",
                "description": null,
                "comment": null
            }
        });

        let issue: JiraIssue = serde_json::from_value(json).unwrap();
        assert_eq!(issue.key, "OO-42");
        assert_eq!(issue.fields.summary, "Fix login flow");
        assert_eq!(issue.fields.status.name, "In Progress");
        let assignee = issue.fields.assignee.unwrap();
        assert_eq!(assignee.account_id, "5b10ac8d82e05b22cc7d4ef5");
        assert_eq!(assignee.display_name, "Leo Acosta");
        assert_eq!(issue.fields.priority.unwrap().name, "High");
        assert_eq!(issue.fields.labels.len(), 2);
    }

    #[test]
    fn search_result_deserializes_empty_issues() {
        let json = serde_json::json!({
            "total": 0,
            "maxResults": 50,
            "issues": []
        });

        let result: JiraSearchResult = serde_json::from_value(json).unwrap();
        assert_eq!(result.total, 0);
        assert_eq!(result.max_results, 50);
        assert!(result.issues.is_empty());
    }

    #[test]
    fn transitions_response_deserializes() {
        let json = serde_json::json!({
            "transitions": [
                {"id": "11", "name": "To Do"},
                {"id": "21", "name": "In Progress"},
                {"id": "31", "name": "Done"}
            ]
        });

        let resp: JiraTransitionsResponse = serde_json::from_value(json).unwrap();
        assert_eq!(resp.transitions.len(), 3);
        assert_eq!(resp.transitions[0].name, "To Do");
        assert_eq!(resp.transitions[1].name, "In Progress");
        assert_eq!(resp.transitions[2].name, "Done");
    }

    #[test]
    fn jira_user_deserializes_with_serde_rename() {
        let json = serde_json::json!({
            "accountId": "5b10ac8d82e05b22cc7d4ef5",
            "displayName": "Leo Acosta"
        });

        let user: JiraUser = serde_json::from_value(json).unwrap();
        assert_eq!(user.account_id, "5b10ac8d82e05b22cc7d4ef5");
        assert_eq!(user.display_name, "Leo Acosta");
    }

    #[test]
    fn created_issue_deserializes_with_self_rename() {
        let json = serde_json::json!({
            "id": "10042",
            "key": "OO-42",
            "self": "https://yourorg.atlassian.net/rest/api/3/issue/10042"
        });

        let created: JiraCreatedIssue = serde_json::from_value(json).unwrap();
        assert_eq!(created.id, "10042");
        assert_eq!(created.key, "OO-42");
        assert_eq!(
            created.self_url,
            "https://yourorg.atlassian.net/rest/api/3/issue/10042"
        );
    }

    #[test]
    fn jira_comment_deserializes() {
        let json = serde_json::json!({
            "id": "10100",
            "body": {
                "type": "doc",
                "version": 1,
                "content": [{"type": "paragraph", "content": [{"type": "text", "text": "Fixed in v2.1"}]}]
            },
            "created": "2026-03-21T15:00:00.000+0000",
            "author": {
                "accountId": "5b10ac8d82e05b22cc7d4ef5",
                "displayName": "Leo Acosta"
            }
        });

        let comment: JiraComment = serde_json::from_value(json).unwrap();
        assert_eq!(comment.id, "10100");
        assert_eq!(comment.author.display_name, "Leo Acosta");
    }

    #[test]
    fn comment_page_deserializes() {
        let json = serde_json::json!({
            "total": 2,
            "comments": [
                {
                    "id": "10100",
                    "body": {"type": "doc", "version": 1, "content": []},
                    "created": "2026-03-21T15:00:00.000+0000",
                    "author": {"accountId": "abc", "displayName": "Leo"}
                },
                {
                    "id": "10101",
                    "body": {"type": "doc", "version": 1, "content": []},
                    "created": "2026-03-21T16:00:00.000+0000",
                    "author": {"accountId": "def", "displayName": "NV"}
                }
            ]
        });

        let page: JiraCommentPage = serde_json::from_value(json).unwrap();
        assert_eq!(page.total, 2);
        assert_eq!(page.comments.len(), 2);
    }

    #[test]
    fn jira_action_type_round_trip() {
        let action = JiraActionType::Create;
        let json = serde_json::to_string(&action).unwrap();
        let restored: JiraActionType = serde_json::from_str(&json).unwrap();
        matches!(restored, JiraActionType::Create);
    }
}
