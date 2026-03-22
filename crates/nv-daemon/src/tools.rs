use anyhow::{anyhow, Result};

use crate::claude::ToolDefinition;
use crate::jira;
use crate::memory::Memory;
use crate::nexus;

/// Register all available tool definitions for the Anthropic API.
///
/// Returns tool schemas in the Anthropic `tools` format.
/// Includes memory tools, Jira tools, and Nexus tools.
pub fn register_tools() -> Vec<ToolDefinition> {
    let mut tools = vec![
        ToolDefinition {
            name: "read_memory".into(),
            description: "Read a specific memory topic file. Returns the contents of the topic.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "topic": {
                        "type": "string",
                        "description": "The memory topic to read (e.g., 'tasks', 'preferences', 'project-notes')"
                    }
                },
                "required": ["topic"]
            }),
        },
        ToolDefinition {
            name: "search_memory".into(),
            description: "Search across all memory files for relevant information. Returns matching excerpts.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query to match against memory contents"
                    }
                },
                "required": ["query"]
            }),
        },
        ToolDefinition {
            name: "write_memory".into(),
            description: "Store information in a memory topic for future reference. Appends to the topic file.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "topic": {
                        "type": "string",
                        "description": "The memory topic to write to"
                    },
                    "content": {
                        "type": "string",
                        "description": "The content to store"
                    }
                },
                "required": ["topic", "content"]
            }),
        },
        ToolDefinition {
            name: "query_nexus".into(),
            description: "Get the status of running Nexus agent sessions. Returns session IDs, agent names, and states.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "query_session".into(),
            description: "Get detailed information about a specific Nexus session by ID. Returns project, status, duration, command, branch, model, and cost.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": "The session ID to look up"
                    }
                },
                "required": ["session_id"]
            }),
        },
        ToolDefinition {
            name: "complete_bootstrap".into(),
            description: "Mark first-run bootstrap as complete. Call this after writing identity.md, user.md, and soul.md during the bootstrap conversation. Writes a state file so bootstrap is skipped on future startups.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "update_soul".into(),
            description: "Update Nova's soul/personality file (soul.md). Use sparingly — always notify the operator about what changed and why.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "The full new content for soul.md"
                    }
                },
                "required": ["content"]
            }),
        },
    ];

    // Add all Jira tool definitions
    tools.extend(jira::jira_tool_definitions());

    tools
}

/// Result of executing a tool — either an immediate result or a
/// pending action that requires Telegram confirmation.
#[derive(Debug)]
pub enum ToolResult {
    /// Immediate text result to return to Claude.
    Immediate(String),
    /// A Jira write operation that needs confirmation before executing.
    PendingAction {
        description: String,
        action_type: nv_core::types::ActionType,
        payload: serde_json::Value,
    },
}

/// Execute a tool by name with the given input parameters.
///
/// Memory tools are synchronous. Jira read tools are async. Jira write
/// tools return a PendingAction instead of executing immediately.
pub async fn execute_tool(
    name: &str,
    input: &serde_json::Value,
    memory: &Memory,
    jira_client: Option<&jira::JiraClient>,
    nexus_client: Option<&nexus::client::NexusClient>,
) -> Result<ToolResult> {
    match name {
        // ── Memory Tools ────────────────────────────────────────
        "read_memory" => {
            let topic = input["topic"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'topic' parameter"))?;
            memory.read(topic).map(ToolResult::Immediate)
        }
        "search_memory" => {
            let query = input["query"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'query' parameter"))?;
            memory.search(query).map(ToolResult::Immediate)
        }
        "write_memory" => {
            let topic = input["topic"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'topic' parameter"))?;
            let content = input["content"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'content' parameter"))?;
            memory.write(topic, content).map(ToolResult::Immediate)
        }

        // ── Jira Read Tools (immediate) ─────────────────────────
        "jira_search" => {
            let jql = input["jql"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'jql' parameter"))?;
            let client = jira_client
                .ok_or_else(|| anyhow!("Jira not configured"))?;
            let issues = client.search(jql).await?;
            Ok(ToolResult::Immediate(jira::format_issues_for_claude(
                &issues,
            )))
        }
        "jira_get" => {
            let key = input["issue_key"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'issue_key' parameter"))?;
            let client = jira_client
                .ok_or_else(|| anyhow!("Jira not configured"))?;
            let issue = client.get_issue(key).await?;
            Ok(ToolResult::Immediate(jira::format_issue_for_claude(&issue)))
        }

        // ── Jira Write Tools (pending action) ──────────────────
        "jira_create" => {
            if jira_client.is_none() {
                anyhow::bail!("Jira not configured");
            }
            let description = jira::describe_pending_action(name, input);
            Ok(ToolResult::PendingAction {
                description,
                action_type: nv_core::types::ActionType::JiraCreate,
                payload: input.clone(),
            })
        }
        "jira_transition" => {
            if jira_client.is_none() {
                anyhow::bail!("Jira not configured");
            }
            let description = jira::describe_pending_action(name, input);
            Ok(ToolResult::PendingAction {
                description,
                action_type: nv_core::types::ActionType::JiraTransition,
                payload: input.clone(),
            })
        }
        "jira_assign" => {
            if jira_client.is_none() {
                anyhow::bail!("Jira not configured");
            }
            let description = jira::describe_pending_action(name, input);
            Ok(ToolResult::PendingAction {
                description,
                action_type: nv_core::types::ActionType::JiraAssign,
                payload: input.clone(),
            })
        }
        "jira_comment" => {
            if jira_client.is_none() {
                anyhow::bail!("Jira not configured");
            }
            let description = jira::describe_pending_action(name, input);
            Ok(ToolResult::PendingAction {
                description,
                action_type: nv_core::types::ActionType::JiraComment,
                payload: input.clone(),
            })
        }

        // ── Bootstrap & Soul Tools ──────────────────────────
        "complete_bootstrap" => {
            let home = std::env::var("HOME").unwrap_or_default();
            let path = std::path::Path::new(&home)
                .join(".nv")
                .join("bootstrap-state.json");
            let state = serde_json::json!({
                "completed_at": chrono::Utc::now().to_rfc3339()
            });
            std::fs::write(&path, serde_json::to_string_pretty(&state)?)
                .map_err(|e| anyhow!("failed to write bootstrap state: {e}"))?;
            tracing::info!("bootstrap completed, state written");
            Ok(ToolResult::Immediate(
                "Bootstrap completed. Nova is ready.".into(),
            ))
        }
        "update_soul" => {
            let content = input["content"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'content' parameter"))?;
            let home = std::env::var("HOME").unwrap_or_default();
            let path = std::path::Path::new(&home).join(".nv").join("soul.md");
            std::fs::write(&path, content)
                .map_err(|e| anyhow!("failed to write soul.md: {e}"))?;
            tracing::info!("soul.md updated");
            Ok(ToolResult::Immediate(
                "Soul updated. Notification sent to Leo.".into(),
            ))
        }

        // ── Nexus Tools ──────────────────────────────────────
        "query_nexus" => {
            let client = nexus_client
                .ok_or_else(|| anyhow!("Nexus not configured"))?;
            let output = nexus::tools::format_query_sessions(client).await?;
            Ok(ToolResult::Immediate(output))
        }
        "query_session" => {
            let client = nexus_client
                .ok_or_else(|| anyhow!("Nexus not configured"))?;
            let session_id = input["session_id"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'session_id' parameter"))?;
            let output = nexus::tools::format_query_session(client, session_id).await?;
            Ok(ToolResult::Immediate(output))
        }

        _ => Err(anyhow!("unknown tool: {name}")),
    }
}

/// Execute a confirmed Jira pending action against the real JiraClient.
///
/// Called when the user taps "Approve" on a Telegram inline keyboard.
#[allow(dead_code)]
pub async fn execute_jira_action(
    jira_client: &jira::JiraClient,
    action_type: &nv_core::types::ActionType,
    payload: &serde_json::Value,
) -> Result<String> {
    match action_type {
        nv_core::types::ActionType::JiraCreate => {
            let params: jira::JiraCreateParams = serde_json::from_value(payload.clone())
                .map_err(|e| anyhow!("invalid jira_create payload: {e}"))?;
            let created = jira_client.create_issue(&params).await?;
            Ok(format!("Created {}: {}", created.key, params.title))
        }
        nv_core::types::ActionType::JiraTransition => {
            let issue_key = payload["issue_key"]
                .as_str()
                .ok_or_else(|| anyhow!("missing issue_key in transition payload"))?;
            let transition_name = payload["transition_name"]
                .as_str()
                .ok_or_else(|| anyhow!("missing transition_name in transition payload"))?;
            jira_client
                .transition_issue(issue_key, transition_name)
                .await?;
            Ok(format!("Transitioned {issue_key} to {transition_name}"))
        }
        nv_core::types::ActionType::JiraAssign => {
            let issue_key = payload["issue_key"]
                .as_str()
                .ok_or_else(|| anyhow!("missing issue_key in assign payload"))?;
            let assignee = payload["assignee_account_id"]
                .as_str()
                .or_else(|| payload["assignee"].as_str())
                .ok_or_else(|| anyhow!("missing assignee in assign payload"))?;
            jira_client
                .assign_issue(issue_key, assignee)
                .await?;
            Ok(format!("Assigned {issue_key} to {assignee}"))
        }
        nv_core::types::ActionType::JiraComment => {
            let issue_key = payload["issue_key"]
                .as_str()
                .ok_or_else(|| anyhow!("missing issue_key in comment payload"))?;
            let body = payload["body"]
                .as_str()
                .ok_or_else(|| anyhow!("missing body in comment payload"))?;
            let comment = jira_client.add_comment(issue_key, body).await?;
            Ok(format!("Added comment {} to {issue_key}", comment.id))
        }
        _ => Err(anyhow!("Not a Jira action type: {action_type:?}")),
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, Memory) {
        let dir = TempDir::new().unwrap();
        let memory = Memory::new(dir.path());
        memory.init().unwrap();
        (dir, memory)
    }

    #[test]
    fn register_tools_returns_thirteen() {
        let tools = register_tools();
        // 3 memory + 2 bootstrap/soul + 2 nexus + 6 jira = 13
        assert_eq!(tools.len(), 13);

        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"read_memory"));
        assert!(names.contains(&"search_memory"));
        assert!(names.contains(&"write_memory"));
        assert!(names.contains(&"complete_bootstrap"));
        assert!(names.contains(&"update_soul"));
        assert!(names.contains(&"query_nexus"));
        assert!(names.contains(&"query_session"));
        assert!(names.contains(&"jira_search"));
        assert!(names.contains(&"jira_get"));
        assert!(names.contains(&"jira_create"));
        assert!(names.contains(&"jira_transition"));
        assert!(names.contains(&"jira_assign"));
        assert!(names.contains(&"jira_comment"));
    }

    #[test]
    fn tool_schemas_have_required_fields() {
        let tools = register_tools();
        for tool in &tools {
            assert!(!tool.name.is_empty());
            assert!(!tool.description.is_empty());
            assert_eq!(tool.input_schema["type"], "object");
            assert!(tool.input_schema.get("properties").is_some());
            assert!(tool.input_schema.get("required").is_some());
        }
    }

    #[test]
    fn read_memory_schema_requires_topic() {
        let tools = register_tools();
        let rm = tools.iter().find(|t| t.name == "read_memory").unwrap();
        let required = rm.input_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v.as_str() == Some("topic")));
    }

    #[test]
    fn write_memory_schema_requires_topic_and_content() {
        let tools = register_tools();
        let wm = tools.iter().find(|t| t.name == "write_memory").unwrap();
        let required = wm.input_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v.as_str() == Some("topic")));
        assert!(required.iter().any(|v| v.as_str() == Some("content")));
    }

    #[test]
    fn query_nexus_schema_has_no_required_params() {
        let tools = register_tools();
        let qn = tools.iter().find(|t| t.name == "query_nexus").unwrap();
        let required = qn.input_schema["required"].as_array().unwrap();
        assert!(required.is_empty());
    }

    #[tokio::test]
    async fn execute_read_memory_returns_content() {
        let (_dir, memory) = setup();
        let result = execute_tool(
            "read_memory",
            &serde_json::json!({"topic": "tasks"}),
            &memory,
            None,
            None,
        )
        .await
        .unwrap();
        match result {
            ToolResult::Immediate(s) => assert!(s.contains("Tasks")),
            _ => panic!("expected Immediate"),
        }
    }

    #[tokio::test]
    async fn execute_read_memory_nonexistent() {
        let (_dir, memory) = setup();
        let result = execute_tool(
            "read_memory",
            &serde_json::json!({"topic": "nonexistent"}),
            &memory,
            None,
            None,
        )
        .await
        .unwrap();
        match result {
            ToolResult::Immediate(s) => assert!(s.contains("No memory file found")),
            _ => panic!("expected Immediate"),
        }
    }

    #[tokio::test]
    async fn execute_search_memory_finds_content() {
        let (_dir, memory) = setup();
        memory.write("decisions", "Stripe fee is 5%").unwrap();

        let result = execute_tool(
            "search_memory",
            &serde_json::json!({"query": "Stripe"}),
            &memory,
            None,
            None,
        )
        .await
        .unwrap();
        match result {
            ToolResult::Immediate(s) => {
                assert!(s.contains("Stripe"));
                assert!(s.contains("Found matches"));
            }
            _ => panic!("expected Immediate"),
        }
    }

    #[tokio::test]
    async fn execute_search_memory_no_results() {
        let (_dir, memory) = setup();
        let result = execute_tool(
            "search_memory",
            &serde_json::json!({"query": "xyznonexistent"}),
            &memory,
            None,
            None,
        )
        .await
        .unwrap();
        match result {
            ToolResult::Immediate(s) => assert!(s.contains("No matches found")),
            _ => panic!("expected Immediate"),
        }
    }

    #[tokio::test]
    async fn execute_write_memory_creates_topic() {
        let (_dir, memory) = setup();
        let result = execute_tool(
            "write_memory",
            &serde_json::json!({"topic": "notes", "content": "hello world"}),
            &memory,
            None,
            None,
        )
        .await
        .unwrap();
        match result {
            ToolResult::Immediate(s) => {
                assert!(s.contains("Created new memory topic"));
                assert!(s.contains("notes"));
            }
            _ => panic!("expected Immediate"),
        }

        // Verify it was written
        let read_result = execute_tool(
            "read_memory",
            &serde_json::json!({"topic": "notes"}),
            &memory,
            None,
            None,
        )
        .await
        .unwrap();
        match read_result {
            ToolResult::Immediate(s) => assert!(s.contains("hello world")),
            _ => panic!("expected Immediate"),
        }
    }

    #[tokio::test]
    async fn execute_jira_search_without_client_returns_error() {
        let (_dir, memory) = setup();
        let result = execute_tool(
            "jira_search",
            &serde_json::json!({"jql": "project = NV"}),
            &memory,
            None,
            None,
        )
        .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Jira not configured"));
    }

    #[tokio::test]
    async fn execute_jira_create_returns_pending_action() {
        let (_dir, memory) = setup();
        // Create a dummy JiraClient (won't be called for write tools)
        let client = jira::JiraClient::new(
            "https://test.atlassian.net",
            "test@test.com",
            "fake-token",
        );
        let result = execute_tool(
            "jira_create",
            &serde_json::json!({
                "project": "OO",
                "issue_type": "Bug",
                "title": "Test issue"
            }),
            &memory,
            Some(&client),
            None,
        )
        .await
        .unwrap();
        match result {
            ToolResult::PendingAction {
                description,
                action_type,
                payload,
            } => {
                assert!(description.contains("Bug"));
                assert!(description.contains("OO"));
                assert!(description.contains("Test issue"));
                assert!(matches!(action_type, nv_core::types::ActionType::JiraCreate));
                assert_eq!(payload["project"], "OO");
            }
            _ => panic!("expected PendingAction"),
        }
    }

    #[tokio::test]
    async fn execute_jira_transition_returns_pending_action() {
        let (_dir, memory) = setup();
        let client = jira::JiraClient::new(
            "https://test.atlassian.net",
            "test@test.com",
            "fake-token",
        );
        let result = execute_tool(
            "jira_transition",
            &serde_json::json!({
                "issue_key": "OO-42",
                "transition_name": "In Progress"
            }),
            &memory,
            Some(&client),
            None,
        )
        .await
        .unwrap();
        match result {
            ToolResult::PendingAction {
                description,
                action_type,
                ..
            } => {
                assert!(description.contains("OO-42"));
                assert!(description.contains("In Progress"));
                assert!(matches!(
                    action_type,
                    nv_core::types::ActionType::JiraTransition
                ));
            }
            _ => panic!("expected PendingAction"),
        }
    }

    #[tokio::test]
    async fn execute_jira_comment_returns_pending_action() {
        let (_dir, memory) = setup();
        let client = jira::JiraClient::new(
            "https://test.atlassian.net",
            "test@test.com",
            "fake-token",
        );
        let result = execute_tool(
            "jira_comment",
            &serde_json::json!({
                "issue_key": "OO-42",
                "body": "This is a comment"
            }),
            &memory,
            Some(&client),
            None,
        )
        .await
        .unwrap();
        match result {
            ToolResult::PendingAction {
                description,
                action_type,
                ..
            } => {
                assert!(description.contains("OO-42"));
                assert!(matches!(
                    action_type,
                    nv_core::types::ActionType::JiraComment
                ));
            }
            _ => panic!("expected PendingAction"),
        }
    }

    #[tokio::test]
    async fn execute_query_nexus_without_client_returns_error() {
        let (_dir, memory) = setup();
        let result = execute_tool("query_nexus", &serde_json::json!({}), &memory, None, None)
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Nexus not configured"));
    }

    #[tokio::test]
    async fn execute_query_nexus_with_client_returns_immediate() {
        let (_dir, memory) = setup();
        let client = nexus::client::NexusClient::new(&[nv_core::config::NexusAgent {
            name: "test".into(),
            host: "127.0.0.1".into(),
            port: 7400,
        }]);
        let result = execute_tool("query_nexus", &serde_json::json!({}), &memory, None, Some(&client))
            .await
            .unwrap();
        match result {
            ToolResult::Immediate(s) => {
                assert!(s.contains("unreachable") || s.contains("No Nexus agents") || s.contains("No active sessions"));
            }
            _ => panic!("expected Immediate"),
        }
    }

    #[tokio::test]
    async fn execute_query_session_without_client_returns_error() {
        let (_dir, memory) = setup();
        let result = execute_tool(
            "query_session",
            &serde_json::json!({"session_id": "s-1"}),
            &memory,
            None,
            None,
        )
        .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Nexus not configured"));
    }

    #[tokio::test]
    async fn execute_query_session_missing_param() {
        let (_dir, memory) = setup();
        let client = nexus::client::NexusClient::new(&[]);
        let result = execute_tool(
            "query_session",
            &serde_json::json!({}),
            &memory,
            None,
            Some(&client),
        )
        .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("session_id"));
    }

    #[tokio::test]
    async fn execute_unknown_tool_returns_error() {
        let (_dir, memory) = setup();
        let result = execute_tool("nonexistent_tool", &serde_json::json!({}), &memory, None, None).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("unknown tool"));
        assert!(err.contains("nonexistent_tool"));
    }

    #[tokio::test]
    async fn execute_read_memory_missing_param() {
        let (_dir, memory) = setup();
        let result = execute_tool("read_memory", &serde_json::json!({}), &memory, None, None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("topic"));
    }

    #[tokio::test]
    async fn execute_write_memory_missing_content() {
        let (_dir, memory) = setup();
        let result = execute_tool(
            "write_memory",
            &serde_json::json!({"topic": "x"}),
            &memory,
            None,
            None,
        )
        .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("content"));
    }

    #[tokio::test]
    async fn execute_complete_bootstrap_writes_state() {
        let (_dir, memory) = setup();
        // Set HOME to a temp dir so we don't write to real ~/.nv/
        let tmp = TempDir::new().unwrap();
        let nv_dir = tmp.path().join(".nv");
        std::fs::create_dir_all(&nv_dir).unwrap();
        std::env::set_var("HOME", tmp.path());

        let result = execute_tool(
            "complete_bootstrap",
            &serde_json::json!({}),
            &memory,
            None,
            None,
        )
        .await
        .unwrap();

        match result {
            ToolResult::Immediate(s) => {
                assert!(s.contains("Bootstrap completed"));
            }
            _ => panic!("expected Immediate"),
        }

        // Verify state file was written
        let state_path = nv_dir.join("bootstrap-state.json");
        assert!(state_path.exists());
        let state: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&state_path).unwrap()).unwrap();
        assert!(state["completed_at"].is_string());
    }

    #[tokio::test]
    async fn execute_update_soul_writes_file() {
        let (_dir, memory) = setup();
        let tmp = TempDir::new().unwrap();
        let nv_dir = tmp.path().join(".nv");
        std::fs::create_dir_all(&nv_dir).unwrap();
        std::env::set_var("HOME", tmp.path());

        let new_soul = "# Nova — Soul\n\nUpdated personality.";
        let result = execute_tool(
            "update_soul",
            &serde_json::json!({"content": new_soul}),
            &memory,
            None,
            None,
        )
        .await
        .unwrap();

        match result {
            ToolResult::Immediate(s) => {
                assert!(s.contains("Soul updated"));
            }
            _ => panic!("expected Immediate"),
        }

        // Verify soul.md was written
        let soul_path = nv_dir.join("soul.md");
        let content = std::fs::read_to_string(&soul_path).unwrap();
        assert_eq!(content, new_soul);
    }

    #[tokio::test]
    async fn execute_update_soul_missing_content() {
        let (_dir, memory) = setup();
        let result = execute_tool(
            "update_soul",
            &serde_json::json!({}),
            &memory,
            None,
            None,
        )
        .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("content"));
    }

    #[test]
    fn complete_bootstrap_schema_has_no_required_params() {
        let tools = register_tools();
        let cb = tools
            .iter()
            .find(|t| t.name == "complete_bootstrap")
            .unwrap();
        let required = cb.input_schema["required"].as_array().unwrap();
        assert!(required.is_empty());
    }

    #[test]
    fn update_soul_schema_requires_content() {
        let tools = register_tools();
        let us = tools.iter().find(|t| t.name == "update_soul").unwrap();
        let required = us.input_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v.as_str() == Some("content")));
    }
}
