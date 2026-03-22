use std::time::Duration;

use anyhow::Result;
use serde::{Deserialize, Serialize};

// ── Error Types ─────────────────────────────────────────────────────

#[derive(thiserror::Error, Debug)]
pub enum ApiError {
    #[error("HTTP {status}: {body}")]
    HttpError {
        status: reqwest::StatusCode,
        body: String,
    },
    #[error("Rate limited, retry after {retry_after_secs}s")]
    RateLimited { retry_after_secs: u64 },
    #[error("Authentication failed (HTTP 401): {body}")]
    AuthError { body: String },
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("Deserialization error: {0}")]
    Deserialize(String),
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

/// Response from the Anthropic Messages API.
#[derive(Debug, Clone, Deserialize)]
pub struct ApiResponse {
    #[allow(dead_code)]
    pub id: String,
    pub content: Vec<ContentBlock>,
    pub stop_reason: StopReason,
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

// ── Claude API Client ───────────────────────────────────────────────

pub struct ClaudeClient {
    http: reqwest::Client,
    api_key: String,
    model: String,
    max_tokens: u32,
}

impl ClaudeClient {
    pub fn new(api_key: String, model: String, max_tokens: u32) -> Self {
        Self {
            http: reqwest::Client::new(),
            api_key,
            model,
            max_tokens,
        }
    }

    /// Send a messages request to the Anthropic API.
    ///
    /// Handles retries for rate limits (429) and server errors (5xx).
    pub async fn send_messages(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<ApiResponse> {
        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": self.max_tokens,
            "system": system,
            "messages": messages,
            "tools": tools,
        });

        let mut last_err: Option<anyhow::Error> = None;

        // Retry loop: up to 3 attempts for retryable errors
        for attempt in 0..3u32 {
            if attempt > 0 {
                let delay = Duration::from_secs(1 << (attempt - 1)); // 1s, 2s
                tracing::warn!(attempt, delay_secs = delay.as_secs(), "retrying Claude API call");
                tokio::time::sleep(delay).await;
            }

            let result = self
                .http
                .post("https://api.anthropic.com/v1/messages")
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .json(&body)
                .send()
                .await;

            let response = match result {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(attempt, error = %e, "network error calling Claude API");
                    last_err = Some(ApiError::Network(e).into());
                    continue;
                }
            };

            let status = response.status();

            // 401 — auth error, do not retry
            if status == reqwest::StatusCode::UNAUTHORIZED {
                let body_text = response.text().await.unwrap_or_default();
                return Err(ApiError::AuthError { body: body_text }.into());
            }

            // 429 — rate limited, retry once after delay
            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                let retry_after = response
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(5);
                tracing::warn!(retry_after_secs = retry_after, "rate limited by Claude API");
                tokio::time::sleep(Duration::from_secs(retry_after)).await;
                last_err = Some(ApiError::RateLimited { retry_after_secs: retry_after }.into());
                continue;
            }

            // 5xx — server error, retry with backoff
            if status.is_server_error() {
                let body_text = response.text().await.unwrap_or_default();
                tracing::warn!(attempt, status = %status, "server error from Claude API");
                last_err = Some(
                    ApiError::HttpError {
                        status,
                        body: body_text,
                    }
                    .into(),
                );
                continue;
            }

            // Other non-success status codes
            if !status.is_success() {
                let body_text = response.text().await.unwrap_or_default();
                return Err(ApiError::HttpError {
                    status,
                    body: body_text,
                }
                .into());
            }

            // Success — parse JSON
            let response_text = response.text().await.map_err(ApiError::Network)?;
            match serde_json::from_str::<ApiResponse>(&response_text) {
                Ok(api_response) => {
                    tracing::debug!(
                        input_tokens = api_response.usage.input_tokens,
                        output_tokens = api_response.usage.output_tokens,
                        stop_reason = ?api_response.usage,
                        "Claude API response received"
                    );
                    return Ok(api_response);
                }
                Err(e) => {
                    tracing::error!(
                        body = %response_text,
                        error = %e,
                        "failed to deserialize Claude API response"
                    );
                    return Err(ApiError::Deserialize(e.to_string()).into());
                }
            }
        }

        Err(last_err.unwrap_or_else(|| anyhow::anyhow!("Claude API call failed after 3 attempts")))
    }
}

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
        // is_error: false should be skipped
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
    fn api_response_deserialization() {
        let json = serde_json::json!({
            "id": "msg_123",
            "content": [
                {"type": "text", "text": "Hello there"}
            ],
            "stop_reason": "end_turn",
            "usage": {"input_tokens": 100, "output_tokens": 50}
        });
        let resp: ApiResponse = serde_json::from_value(json).unwrap();
        assert_eq!(resp.id, "msg_123");
        assert_eq!(resp.content.len(), 1);
        assert_eq!(resp.stop_reason, StopReason::EndTurn);
        assert_eq!(resp.usage.input_tokens, 100);
        assert_eq!(resp.usage.output_tokens, 50);
    }

    #[test]
    fn api_response_tool_use_deserialization() {
        let json = serde_json::json!({
            "id": "msg_456",
            "content": [
                {"type": "text", "text": "Let me check that for you."},
                {"type": "tool_use", "id": "tu-1", "name": "read_memory", "input": {"topic": "tasks"}}
            ],
            "stop_reason": "tool_use",
            "usage": {"input_tokens": 200, "output_tokens": 80}
        });
        let resp: ApiResponse = serde_json::from_value(json).unwrap();
        assert_eq!(resp.stop_reason, StopReason::ToolUse);
        assert_eq!(resp.content.len(), 2);
        match &resp.content[1] {
            ContentBlock::ToolUse { id, name, input } => {
                assert_eq!(id, "tu-1");
                assert_eq!(name, "read_memory");
                assert_eq!(input["topic"], "tasks");
            }
            _ => panic!("expected tool_use block"),
        }
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
    fn stop_reason_max_tokens() {
        let json = serde_json::json!({
            "id": "msg_789",
            "content": [{"type": "text", "text": "partial..."}],
            "stop_reason": "max_tokens",
            "usage": {"input_tokens": 100, "output_tokens": 4096}
        });
        let resp: ApiResponse = serde_json::from_value(json).unwrap();
        assert_eq!(resp.stop_reason, StopReason::MaxTokens);
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
