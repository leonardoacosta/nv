use crate::claude::ToolDefinition;

use super::types::*;

/// Return agent tool definitions for all Jira tools.
///
/// These are registered in the agent loop and sent to the Claude API
/// as available tools.
pub fn jira_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "jira_search".into(),
            description: "Search Jira issues using JQL. Returns matching issues with key, summary, status, assignee, priority.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "jql": {
                        "type": "string",
                        "description": "JQL query string. Do NOT use LIMIT — result count is controlled automatically (max 50). Example: project = OO AND status != Done ORDER BY created DESC"
                    }
                },
                "required": ["jql"]
            }),
        },
        ToolDefinition {
            name: "jira_get".into(),
            description: "Get a single Jira issue by key. Returns full issue details including comments.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "issue_key": {
                        "type": "string",
                        "description": "Issue key, e.g. OO-123"
                    }
                },
                "required": ["issue_key"]
            }),
        },
        ToolDefinition {
            name: "jira_create".into(),
            description: "Create a new Jira issue. Requires confirmation via Telegram before execution.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project": {
                        "type": "string",
                        "description": "Jira project KEY (uppercase, 2-4 chars). Examples: 'OO', 'TC', 'CT', 'TL', 'MV'. NOT the full project name."
                    },
                    "issue_type": {
                        "type": "string",
                        "description": "Issue type: Bug, Task, Story, Epic"
                    },
                    "title": {
                        "type": "string",
                        "description": "Issue summary"
                    },
                    "description": {
                        "type": "string",
                        "description": "Issue description (plain text, converted to ADF)"
                    },
                    "priority": {
                        "type": "string",
                        "description": "Priority name: Highest, High, Medium, Low, Lowest"
                    },
                    "assignee": {
                        "type": "string",
                        "description": "Assignee display name (resolved to accountId)"
                    },
                    "labels": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Issue labels"
                    }
                },
                "required": ["project", "issue_type", "title"]
            }),
        },
        ToolDefinition {
            name: "jira_transition".into(),
            description: "Transition a Jira issue to a new status. Requires confirmation via Telegram before execution.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "issue_key": {
                        "type": "string",
                        "description": "Issue key, e.g. OO-123"
                    },
                    "transition_name": {
                        "type": "string",
                        "description": "Target status name, e.g. In Progress, Done"
                    }
                },
                "required": ["issue_key", "transition_name"]
            }),
        },
        ToolDefinition {
            name: "jira_assign".into(),
            description: "Assign a Jira issue to a user. Requires confirmation via Telegram before execution.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "issue_key": {
                        "type": "string",
                        "description": "Issue key, e.g. OO-123"
                    },
                    "assignee": {
                        "type": "string",
                        "description": "Assignee display name (resolved to accountId)"
                    }
                },
                "required": ["issue_key", "assignee"]
            }),
        },
        ToolDefinition {
            name: "jira_comment".into(),
            description: "Add a comment to a Jira issue. Requires confirmation via Telegram before execution.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "issue_key": {
                        "type": "string",
                        "description": "Issue key, e.g. OO-123"
                    },
                    "body": {
                        "type": "string",
                        "description": "Comment text (plain text, converted to ADF)"
                    }
                },
                "required": ["issue_key", "body"]
            }),
        },
    ]
}

/// Format a list of Jira issues as concise text for a Claude tool result.
pub fn format_issues_for_claude(issues: &[JiraIssue]) -> String {
    if issues.is_empty() {
        return "No issues found.".to_string();
    }

    let mut lines = Vec::with_capacity(issues.len());

    for issue in issues {
        let assignee = issue
            .fields
            .assignee
            .as_ref()
            .map(|a| a.display_name.as_str())
            .unwrap_or("Unassigned");
        let priority = issue
            .fields
            .priority
            .as_ref()
            .map(|p| p.name.as_str())
            .unwrap_or("None");

        lines.push(format!(
            "📋 **{}** — {}\n   {} · {} · {}",
            issue.key,
            issue.fields.summary,
            issue.fields.status.name,
            assignee,
            priority,
        ));
    }

    lines.join("\n")
}

/// Format a single Jira issue as detailed text for a Claude tool result.
pub fn format_issue_for_claude(issue: &JiraIssue) -> String {
    let assignee = issue
        .fields
        .assignee
        .as_ref()
        .map(|a| a.display_name.as_str())
        .unwrap_or("Unassigned");
    let priority = issue
        .fields
        .priority
        .as_ref()
        .map(|p| p.name.as_str())
        .unwrap_or("None");

    let mut parts = vec![
        format!("Key: {}", issue.key),
        format!("Summary: {}", issue.fields.summary),
        format!("Status: {}", issue.fields.status.name),
        format!("Type: {}", issue.fields.issuetype.name),
        format!(
            "Project: {} ({})",
            issue.fields.project.name, issue.fields.project.key
        ),
        format!("Priority: {priority}"),
        format!("Assignee: {assignee}"),
        format!("Created: {}", issue.fields.created),
        format!("Updated: {}", issue.fields.updated),
    ];

    if !issue.fields.labels.is_empty() {
        parts.push(format!("Labels: {}", issue.fields.labels.join(", ")));
    }

    // Extract plain text from ADF description
    if let Some(desc) = &issue.fields.description {
        if let Some(text) = extract_adf_text(desc) {
            parts.push(format!("Description: {text}"));
        }
    }

    // Format comments
    if let Some(comment_page) = &issue.fields.comment {
        if !comment_page.comments.is_empty() {
            parts.push(format!("\nComments ({}):", comment_page.total));
            for comment in &comment_page.comments {
                let text = extract_adf_text(&comment.body).unwrap_or_default();
                parts.push(format!(
                    "  [{} by {}] {}",
                    comment.created, comment.author.display_name, text
                ));
            }
        }
    }

    parts.join("\n")
}

/// Extract plain text from an Atlassian Document Format (ADF) JSON value.
///
/// Walks the ADF tree and concatenates all text nodes.
fn extract_adf_text(adf: &serde_json::Value) -> Option<String> {
    let mut texts = Vec::new();
    collect_adf_text(adf, &mut texts);
    if texts.is_empty() {
        None
    } else {
        Some(texts.join(" "))
    }
}

fn collect_adf_text(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            if map.get("type").and_then(|t| t.as_str()) == Some("text") {
                if let Some(text) = map.get("text").and_then(|t| t.as_str()) {
                    out.push(text.to_string());
                }
            }
            if let Some(content) = map.get("content") {
                collect_adf_text(content, out);
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                collect_adf_text(item, out);
            }
        }
        _ => {}
    }
}

/// Build a human-readable description of a pending Jira action
/// for display in Telegram confirmation messages.
pub fn describe_pending_action(tool_name: &str, input: &serde_json::Value) -> String {
    match tool_name {
        "jira_create" => {
            let project = input["project"].as_str().unwrap_or("?");
            let issue_type = input["issue_type"].as_str().unwrap_or("?");
            let title = input["title"].as_str().unwrap_or("?");
            let priority = input["priority"].as_str().unwrap_or("Medium");
            format!("Create {issue_type} on {project}: {title} ({priority})")
        }
        "jira_transition" => {
            let key = input["issue_key"].as_str().unwrap_or("?");
            let target = input["transition_name"].as_str().unwrap_or("?");
            format!("Transition {key} to {target}")
        }
        "jira_assign" => {
            let key = input["issue_key"].as_str().unwrap_or("?");
            let assignee = input["assignee"].as_str().unwrap_or("?");
            format!("Assign {key} to {assignee}")
        }
        "jira_comment" => {
            let key = input["issue_key"].as_str().unwrap_or("?");
            let body = input["body"].as_str().unwrap_or("");
            let preview = if body.len() > 50 {
                format!("{}...", &body[..50])
            } else {
                body.to_string()
            };
            format!("Comment on {key}: {preview}")
        }
        _ => format!("Unknown Jira action: {tool_name}"),
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jira_tool_definitions_returns_six_tools() {
        let tools = jira_tool_definitions();
        assert_eq!(tools.len(), 6);

        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"jira_search"));
        assert!(names.contains(&"jira_get"));
        assert!(names.contains(&"jira_create"));
        assert!(names.contains(&"jira_transition"));
        assert!(names.contains(&"jira_assign"));
        assert!(names.contains(&"jira_comment"));
    }

    #[test]
    fn tool_definitions_have_correct_required_fields() {
        let tools = jira_tool_definitions();
        for tool in &tools {
            assert!(!tool.name.is_empty());
            assert!(!tool.description.is_empty());
            assert_eq!(tool.input_schema["type"], "object");
            assert!(tool.input_schema.get("properties").is_some());
            assert!(tool.input_schema.get("required").is_some());
        }

        // Verify specific required fields
        let search = tools.iter().find(|t| t.name == "jira_search").unwrap();
        let required = search.input_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v.as_str() == Some("jql")));

        let create = tools.iter().find(|t| t.name == "jira_create").unwrap();
        let required = create.input_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v.as_str() == Some("project")));
        assert!(required.iter().any(|v| v.as_str() == Some("issue_type")));
        assert!(required.iter().any(|v| v.as_str() == Some("title")));
    }

    #[test]
    fn format_issues_empty() {
        let result = format_issues_for_claude(&[]);
        assert_eq!(result, "No issues found.");
    }

    #[test]
    fn format_issues_single() {
        let issues = vec![JiraIssue {
            id: "10001".into(),
            key: "OO-42".into(),
            fields: JiraIssueFields {
                summary: "Fix login flow".into(),
                status: JiraStatus {
                    name: "In Progress".into(),
                    id: "3".into(),
                },
                assignee: Some(JiraUser {
                    account_id: "abc".into(),
                    display_name: "Leo".into(),
                }),
                priority: Some(JiraPriority {
                    name: "High".into(),
                    id: "2".into(),
                }),
                issuetype: JiraIssueType { name: "Bug".into() },
                project: JiraProject {
                    key: "OO".into(),
                    name: "Otaku Odyssey".into(),
                },
                labels: vec![],
                created: "2026-03-20T10:00:00.000+0000".into(),
                updated: "2026-03-21T14:30:00.000+0000".into(),
                description: None,
                comment: None,
            },
        }];

        let result = format_issues_for_claude(&issues);
        assert!(result.contains("📋"));
        assert!(result.contains("OO-42"));
        assert!(result.contains("Fix login flow"));
        assert!(result.contains("In Progress"));
        assert!(result.contains("Leo"));
        assert!(result.contains("High"));
    }

    #[test]
    fn format_issues_multiple() {
        let issues = vec![
            JiraIssue {
                id: "10001".into(),
                key: "OO-1".into(),
                fields: JiraIssueFields {
                    summary: "First".into(),
                    status: JiraStatus {
                        name: "To Do".into(),
                        id: "1".into(),
                    },
                    assignee: None,
                    priority: None,
                    issuetype: JiraIssueType {
                        name: "Task".into(),
                    },
                    project: JiraProject {
                        key: "OO".into(),
                        name: "OO".into(),
                    },
                    labels: vec![],
                    created: "2026-03-20T00:00:00.000+0000".into(),
                    updated: "2026-03-20T00:00:00.000+0000".into(),
                    description: None,
                    comment: None,
                },
            },
            JiraIssue {
                id: "10002".into(),
                key: "OO-2".into(),
                fields: JiraIssueFields {
                    summary: "Second".into(),
                    status: JiraStatus {
                        name: "Done".into(),
                        id: "4".into(),
                    },
                    assignee: None,
                    priority: None,
                    issuetype: JiraIssueType {
                        name: "Task".into(),
                    },
                    project: JiraProject {
                        key: "OO".into(),
                        name: "OO".into(),
                    },
                    labels: vec![],
                    created: "2026-03-20T00:00:00.000+0000".into(),
                    updated: "2026-03-20T00:00:00.000+0000".into(),
                    description: None,
                    comment: None,
                },
            },
        ];

        let result = format_issues_for_claude(&issues);
        assert!(result.contains("📋"));
        assert!(result.contains("OO-1"));
        assert!(result.contains("OO-2"));
        assert!(result.contains("Unassigned"));
    }

    #[test]
    fn format_issue_detailed() {
        let issue = JiraIssue {
            id: "10001".into(),
            key: "OO-42".into(),
            fields: JiraIssueFields {
                summary: "Fix login flow".into(),
                status: JiraStatus {
                    name: "In Progress".into(),
                    id: "3".into(),
                },
                assignee: Some(JiraUser {
                    account_id: "abc".into(),
                    display_name: "Leo".into(),
                }),
                priority: Some(JiraPriority {
                    name: "High".into(),
                    id: "2".into(),
                }),
                issuetype: JiraIssueType { name: "Bug".into() },
                project: JiraProject {
                    key: "OO".into(),
                    name: "Otaku Odyssey".into(),
                },
                labels: vec!["frontend".into(), "auth".into()],
                created: "2026-03-20T10:00:00.000+0000".into(),
                updated: "2026-03-21T14:30:00.000+0000".into(),
                description: Some(serde_json::json!({
                    "type": "doc",
                    "version": 1,
                    "content": [{
                        "type": "paragraph",
                        "content": [{"type": "text", "text": "Users cannot log in"}]
                    }]
                })),
                comment: Some(JiraCommentPage {
                    total: 1,
                    comments: vec![JiraComment {
                        id: "10100".into(),
                        body: serde_json::json!({
                            "type": "doc",
                            "version": 1,
                            "content": [{
                                "type": "paragraph",
                                "content": [{"type": "text", "text": "Working on fix"}]
                            }]
                        }),
                        created: "2026-03-21T15:00:00.000+0000".into(),
                        author: JiraUser {
                            account_id: "abc".into(),
                            display_name: "Leo".into(),
                        },
                    }],
                }),
            },
        };

        let result = format_issue_for_claude(&issue);
        assert!(result.contains("Key: OO-42"));
        assert!(result.contains("Summary: Fix login flow"));
        assert!(result.contains("Status: In Progress"));
        assert!(result.contains("Type: Bug"));
        assert!(result.contains("Priority: High"));
        assert!(result.contains("Labels: frontend, auth"));
        assert!(result.contains("Description: Users cannot log in"));
        assert!(result.contains("Comments (1):"));
        assert!(result.contains("Working on fix"));
    }

    #[test]
    fn describe_pending_create_action() {
        let input = serde_json::json!({
            "project": "OO",
            "issue_type": "Bug",
            "title": "Login fails on Safari",
            "priority": "Highest"
        });
        let desc = describe_pending_action("jira_create", &input);
        assert_eq!(desc, "Create Bug on OO: Login fails on Safari (Highest)");
    }

    #[test]
    fn describe_pending_transition_action() {
        let input = serde_json::json!({
            "issue_key": "OO-42",
            "transition_name": "In Progress"
        });
        let desc = describe_pending_action("jira_transition", &input);
        assert_eq!(desc, "Transition OO-42 to In Progress");
    }

    #[test]
    fn describe_pending_assign_action() {
        let input = serde_json::json!({
            "issue_key": "OO-42",
            "assignee": "Leo"
        });
        let desc = describe_pending_action("jira_assign", &input);
        assert_eq!(desc, "Assign OO-42 to Leo");
    }

    #[test]
    fn describe_pending_comment_action() {
        let input = serde_json::json!({
            "issue_key": "OO-42",
            "body": "This is a short comment."
        });
        let desc = describe_pending_action("jira_comment", &input);
        assert_eq!(desc, "Comment on OO-42: This is a short comment.");
    }

    #[test]
    fn describe_pending_comment_truncates_long_body() {
        let long_body = "x".repeat(100);
        let input = serde_json::json!({
            "issue_key": "OO-42",
            "body": long_body
        });
        let desc = describe_pending_action("jira_comment", &input);
        assert!(desc.ends_with("..."));
        assert!(desc.len() < 100);
    }

    #[test]
    fn extract_adf_text_simple() {
        let adf = serde_json::json!({
            "type": "doc",
            "version": 1,
            "content": [{
                "type": "paragraph",
                "content": [{"type": "text", "text": "Hello world"}]
            }]
        });
        assert_eq!(extract_adf_text(&adf), Some("Hello world".to_string()));
    }

    #[test]
    fn extract_adf_text_null() {
        let adf = serde_json::Value::Null;
        assert_eq!(extract_adf_text(&adf), None);
    }
}
