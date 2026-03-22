use std::process::Stdio;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

// ── Error Types ─────────────────────────────────────────────────────

#[derive(thiserror::Error, Debug)]
pub enum ApiError {
    #[error("CLI execution failed: {message}")]
    CliError { message: String },
    #[error("Authentication failed: {0}")]
    AuthError(String),
    #[error("Deserialization error: {0}")]
    Deserialize(String),
    #[error("Process error: {0}")]
    Process(#[from] std::io::Error),
}

// ── API Request/Response Types ──────────────────────────────────────

/// A message in the Anthropic Messages API conversation format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: MessageContent,
}

/// Message content can be a simple string or a list of content blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

impl Message {
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: MessageContent::Text(text.into()),
        }
    }

    pub fn assistant_blocks(blocks: Vec<ContentBlock>) -> Self {
        Self {
            role: "assistant".into(),
            content: MessageContent::Blocks(blocks),
        }
    }

    pub fn tool_results(results: Vec<ToolResultBlock>) -> Self {
        let blocks = results
            .into_iter()
            .map(|r| ContentBlock::ToolResult {
                tool_use_id: r.tool_use_id,
                content: r.content,
                is_error: r.is_error,
            })
            .collect();
        Self {
            role: "user".into(),
            content: MessageContent::Blocks(blocks),
        }
    }

    /// Estimate content length in characters (for history truncation).
    pub fn content_len(&self) -> usize {
        match &self.content {
            MessageContent::Text(s) => s.len(),
            MessageContent::Blocks(blocks) => blocks
                .iter()
                .map(|b| match b {
                    ContentBlock::Text { text } => text.len(),
                    ContentBlock::ToolUse { name, input, .. } => {
                        name.len() + input.to_string().len()
                    }
                    ContentBlock::ToolResult { content, .. } => content.len(),
                })
                .sum(),
        }
    }
}

/// Response from the Claude CLI (mapped to match the API format the agent loop expects).
#[derive(Debug, Clone, Deserialize)]
pub struct ApiResponse {
    #[allow(dead_code)]
    pub id: String,
    pub content: Vec<ContentBlock>,
    pub stop_reason: StopReason,
    #[allow(dead_code)]
    pub usage: Usage,
}

/// A content block in an API response (or used for constructing tool results).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        is_error: bool,
    },
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub enum StopReason {
    #[serde(rename = "end_turn")]
    EndTurn,
    #[serde(rename = "tool_use")]
    ToolUse,
    #[serde(rename = "max_tokens")]
    MaxTokens,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

/// Tool definition in the Anthropic API format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

// ── CLI JSON Response Types ─────────────────────────────────────────

/// The JSON response format from `claude -p --output-format json`.
#[derive(Debug, Deserialize)]
struct CliJsonResponse {
    #[serde(default)]
    result: String,
    #[serde(default)]
    is_error: bool,
    #[serde(default)]
    #[allow(dead_code)]
    stop_reason: String,
    #[serde(default)]
    session_id: String,
    #[serde(default)]
    usage: CliUsage,
}

#[derive(Debug, Default, Deserialize)]
struct CliUsage {
    #[serde(default)]
    input_tokens: u32,
    #[serde(default)]
    output_tokens: u32,
}

// ── Claude CLI Client ───────────────────────────────────────────────

pub struct ClaudeClient {
    model: String,
    #[allow(dead_code)]
    max_tokens: u32,
}

impl ClaudeClient {
    /// Create a new client. The `_api_key` parameter is ignored — the CLI uses
    /// its own OAuth session. Kept for backward-compatible constructor signature.
    pub fn new(_api_key: String, model: String, max_tokens: u32) -> Self {
        Self { model, max_tokens }
    }

    /// Send a messages request via the Claude CLI subprocess.
    ///
    /// The CLI handles authentication (OAuth) internally, so no API key is needed.
    /// Each call is a single-turn `claude -p` invocation with the full conversation
    /// formatted as the prompt. Tool definitions are embedded in the system prompt
    /// so Claude can request tool calls via structured JSON output.
    pub async fn send_messages(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<ApiResponse> {
        // Build the full prompt from system + conversation history
        let prompt = build_prompt(system, messages, tools);

        // Spawn claude CLI in a sandboxed HOME with only auth credentials.
        // This prevents loading hooks, CLAUDE.md, agents, MCP servers, and plugins
        // from the host config (~14s → ~8s startup savings per invocation).
        let real_home = std::env::var("REAL_HOME")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_else(|_| "/home/nyaptor".into());
        let sandbox_home = format!("{real_home}/.nv/claude-sandbox");

        let mut child = Command::new("claude")
            .args([
                "--dangerously-skip-permissions",
                "-p",
                "--output-format",
                "json",
                "--model",
                &self.model,
                "--no-session-persistence",
                "--tools",
                "",
            ])
            .env("HOME", &sandbox_home)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| ApiError::CliError {
                message: format!("Failed to spawn claude CLI: {e}"),
            })?;

        // Write prompt to stdin
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(prompt.as_bytes()).await?;
            drop(stdin); // Close stdin to signal EOF
        }

        // Wait for completion
        let output = child.wait_with_output().await?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !stderr.is_empty() {
            tracing::debug!(stderr = %stderr, "claude CLI stderr");
        }

        // Parse CLI JSON response
        let cli_response: CliJsonResponse = serde_json::from_str(&stdout).map_err(|e| {
            tracing::error!(stdout = %stdout, error = %e, "failed to parse CLI response");
            ApiError::Deserialize(format!("CLI JSON parse error: {e}"))
        })?;

        // Check for auth errors
        if cli_response.is_error
            && cli_response
                .result
                .to_lowercase()
                .contains("not logged in")
        {
            return Err(ApiError::AuthError(cli_response.result).into());
        }

        if cli_response.is_error {
            return Err(ApiError::CliError {
                message: cli_response.result,
            }
            .into());
        }

        tracing::debug!(
            input_tokens = cli_response.usage.input_tokens,
            output_tokens = cli_response.usage.output_tokens,
            session_id = %cli_response.session_id,
            "Claude CLI response received"
        );

        // Parse the result text for tool use requests
        let (content, stop_reason) = parse_tool_calls(&cli_response.result);

        Ok(ApiResponse {
            id: cli_response.session_id,
            content,
            stop_reason,
            usage: Usage {
                input_tokens: cli_response.usage.input_tokens,
                output_tokens: cli_response.usage.output_tokens,
            },
        })
    }
}

// ── Prompt Builder ──────────────────────────────────────────────────

/// Build a single-turn prompt that includes the system prompt, tool definitions,
/// and the full conversation history.
fn build_prompt(system: &str, messages: &[Message], tools: &[ToolDefinition]) -> String {
    let mut prompt = String::new();

    // System prompt
    prompt.push_str(system);
    prompt.push_str("\n\n");

    // Tool definitions (embedded in prompt so Claude knows what's available)
    if !tools.is_empty() {
        prompt.push_str("## Available Tools\n\n");
        prompt.push_str(
            "When you need to use a tool, respond with ONLY a JSON block in this exact format:\n",
        );
        prompt.push_str("```tool_call\n");
        prompt.push_str("{\"tool\": \"tool_name\", \"input\": {\"param\": \"value\"}}\n");
        prompt.push_str("```\n\n");
        prompt.push_str("Available tools:\n\n");
        for tool in tools {
            prompt.push_str(&format!("### {}\n", tool.name));
            prompt.push_str(&format!("{}\n", tool.description));
            prompt.push_str(&format!(
                "Parameters: {}\n\n",
                serde_json::to_string_pretty(&tool.input_schema).unwrap_or_default()
            ));
        }
        prompt.push_str("If you don't need a tool, respond normally with text.\n\n");
    }

    // Conversation history
    prompt.push_str("## Conversation\n\n");
    for msg in messages {
        let role_label = if msg.role == "user" { "User" } else { "Assistant" };
        prompt.push_str(&format!("{role_label}: "));
        match &msg.content {
            MessageContent::Text(text) => {
                prompt.push_str(text);
            }
            MessageContent::Blocks(blocks) => {
                for block in blocks {
                    match block {
                        ContentBlock::Text { text } => {
                            prompt.push_str(text);
                        }
                        ContentBlock::ToolUse { name, input, .. } => {
                            prompt.push_str(&format!(
                                "[Called tool: {name} with {}]",
                                serde_json::to_string(input).unwrap_or_default()
                            ));
                        }
                        ContentBlock::ToolResult {
                            content, is_error, ..
                        } => {
                            if *is_error {
                                prompt.push_str(&format!("[Tool error: {content}]"));
                            } else {
                                prompt.push_str(&format!("[Tool result: {content}]"));
                            }
                        }
                    }
                }
            }
        }
        prompt.push_str("\n\n");
    }

    prompt
}

// ── Tool Call Parser ────────────────────────────────────────────────

/// Parse Claude's response text for tool call requests.
///
/// If the response contains a ```tool_call JSON block, extract it as a
/// ContentBlock::ToolUse. Otherwise, return as plain text.
fn parse_tool_calls(result: &str) -> (Vec<ContentBlock>, StopReason) {
    // Look for tool call blocks
    if let Some(start) = result.find("```tool_call") {
        let after_marker = &result[start + "```tool_call".len()..];
        if let Some(end) = after_marker.find("```") {
            let json_str = after_marker[..end].trim();
            if let Ok(call) = serde_json::from_str::<ToolCall>(json_str) {
                let mut content = Vec::new();

                // Include any text before the tool call
                let text_before = result[..start].trim();
                if !text_before.is_empty() {
                    content.push(ContentBlock::Text {
                        text: text_before.to_string(),
                    });
                }

                content.push(ContentBlock::ToolUse {
                    id: format!("cli-{}", uuid::Uuid::new_v4()),
                    name: call.tool,
                    input: call.input,
                });

                return (content, StopReason::ToolUse);
            }
        }
    }

    // No tool call — plain text response
    let content = if result.is_empty() {
        vec![]
    } else {
        vec![ContentBlock::Text {
            text: result.to_string(),
        }]
    };

    (content, StopReason::EndTurn)
}

#[derive(Debug, Deserialize)]
struct ToolCall {
    tool: String,
    input: serde_json::Value,
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_user_creates_text_content() {
        let msg = Message::user("hello");
        assert_eq!(msg.role, "user");
        match &msg.content {
            MessageContent::Text(t) => assert_eq!(t, "hello"),
            _ => panic!("expected text content"),
        }
    }

    #[test]
    fn message_content_len_text() {
        let msg = Message::user("hello world");
        assert_eq!(msg.content_len(), 11);
    }

    #[test]
    fn message_content_len_blocks() {
        let msg = Message::assistant_blocks(vec![
            ContentBlock::Text {
                text: "abc".into(),
            },
            ContentBlock::ToolUse {
                id: "1".into(),
                name: "test".into(),
                input: serde_json::json!({"key": "value"}),
            },
        ]);
        assert!(msg.content_len() > 3);
    }

    #[test]
    fn tool_result_message_format() {
        let msg = Message::tool_results(vec![ToolResultBlock {
            tool_use_id: "tu-1".into(),
            content: "result data".into(),
            is_error: false,
        }]);
        assert_eq!(msg.role, "user");
        match &msg.content {
            MessageContent::Blocks(blocks) => {
                assert_eq!(blocks.len(), 1);
                match &blocks[0] {
                    ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                    } => {
                        assert_eq!(tool_use_id, "tu-1");
                        assert_eq!(content, "result data");
                        assert!(!is_error);
                    }
                    _ => panic!("expected tool result block"),
                }
            }
            _ => panic!("expected blocks content"),
        }
    }

    #[test]
    fn content_block_text_serialization() {
        let block = ContentBlock::Text {
            text: "hello".into(),
        };
        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json["type"], "text");
        assert_eq!(json["text"], "hello");
    }

    #[test]
    fn content_block_tool_use_serialization() {
        let block = ContentBlock::ToolUse {
            id: "tu-1".into(),
            name: "read_memory".into(),
            input: serde_json::json!({"topic": "tasks"}),
        };
        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json["type"], "tool_use");
        assert_eq!(json["name"], "read_memory");
        assert_eq!(json["input"]["topic"], "tasks");
    }

    #[test]
    fn content_block_tool_result_serialization() {
        let block = ContentBlock::ToolResult {
            tool_use_id: "tu-1".into(),
            content: "file contents here".into(),
            is_error: false,
        };
        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json["type"], "tool_result");
        assert_eq!(json["tool_use_id"], "tu-1");
        assert_eq!(json["content"], "file contents here");
        assert!(json.get("is_error").is_none());
    }

    #[test]
    fn content_block_tool_result_with_error() {
        let block = ContentBlock::ToolResult {
            tool_use_id: "tu-2".into(),
            content: "Error: not found".into(),
            is_error: true,
        };
        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json["is_error"], true);
    }

    #[test]
    fn tool_definition_serialization() {
        let tool = ToolDefinition {
            name: "read_memory".into(),
            description: "Read a memory topic".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "topic": {"type": "string", "description": "The topic to read"}
                },
                "required": ["topic"]
            }),
        };
        let json = serde_json::to_value(&tool).unwrap();
        assert_eq!(json["name"], "read_memory");
        assert_eq!(json["input_schema"]["type"], "object");
    }

    #[test]
    fn parse_tool_calls_plain_text() {
        let (content, reason) = parse_tool_calls("Hello, how can I help?");
        assert_eq!(reason, StopReason::EndTurn);
        assert_eq!(content.len(), 1);
        match &content[0] {
            ContentBlock::Text { text } => assert_eq!(text, "Hello, how can I help?"),
            _ => panic!("expected text"),
        }
    }

    #[test]
    fn parse_tool_calls_with_tool_call() {
        let result = "Let me check that.\n```tool_call\n{\"tool\": \"read_memory\", \"input\": {\"topic\": \"tasks\"}}\n```";
        let (content, reason) = parse_tool_calls(result);
        assert_eq!(reason, StopReason::ToolUse);
        assert_eq!(content.len(), 2);
        match &content[0] {
            ContentBlock::Text { text } => assert_eq!(text, "Let me check that."),
            _ => panic!("expected text"),
        }
        match &content[1] {
            ContentBlock::ToolUse { name, input, .. } => {
                assert_eq!(name, "read_memory");
                assert_eq!(input["topic"], "tasks");
            }
            _ => panic!("expected tool use"),
        }
    }

    #[test]
    fn parse_tool_calls_empty_result() {
        let (content, reason) = parse_tool_calls("");
        assert_eq!(reason, StopReason::EndTurn);
        assert!(content.is_empty());
    }

    #[test]
    fn build_prompt_includes_system_and_messages() {
        let system = "You are NV.";
        let messages = vec![Message::user("hello")];
        let tools = vec![];
        let prompt = build_prompt(system, &messages, &tools);
        assert!(prompt.contains("You are NV."));
        assert!(prompt.contains("User: hello"));
    }

    #[test]
    fn build_prompt_includes_tools() {
        let system = "You are NV.";
        let messages = vec![Message::user("hello")];
        let tools = vec![ToolDefinition {
            name: "read_memory".into(),
            description: "Read a topic".into(),
            input_schema: serde_json::json!({"type": "object"}),
        }];
        let prompt = build_prompt(system, &messages, &tools);
        assert!(prompt.contains("### read_memory"));
        assert!(prompt.contains("tool_call"));
    }

    #[test]
    fn cli_response_deserialization() {
        let json = serde_json::json!({
            "type": "result",
            "subtype": "success",
            "is_error": false,
            "result": "Hello there!",
            "stop_reason": "end_turn",
            "session_id": "abc-123",
            "usage": {"input_tokens": 100, "output_tokens": 10}
        });
        let resp: CliJsonResponse = serde_json::from_value(json).unwrap();
        assert_eq!(resp.result, "Hello there!");
        assert!(!resp.is_error);
        assert_eq!(resp.session_id, "abc-123");
    }
}

/// Helper type for constructing tool result blocks before wrapping
/// into a `ContentBlock::ToolResult`.
#[derive(Debug, Clone)]
pub struct ToolResultBlock {
    pub tool_use_id: String,
    pub content: String,
    pub is_error: bool,
}
