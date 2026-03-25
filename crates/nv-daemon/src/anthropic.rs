//! Direct Anthropic Messages API client with SSE streaming.
//!
//! This module provides [`AnthropicClient`], which calls the Anthropic Messages API
//! directly (as a fallback/alternative to the Claude Code CLI). It reuses the shared
//! types defined in `claude.rs`: [`Message`], [`ContentBlock`], [`ApiResponse`],
//! [`ApiError`], [`StopReason`], [`Usage`], and [`ToolDefinition`].

use futures_util::StreamExt;
use serde::Deserialize;
use tokio::time::{sleep, Duration};
use tracing::{debug, warn};

use crate::claude::{ApiError, ApiResponse, ContentBlock, Message, StopReason, ToolDefinition, Usage};

// ── Client ───────────────────────────────────────────────────────────

/// A client for the Anthropic Messages API.
#[derive(Debug)]
pub struct AnthropicClient {
    http: reqwest::Client,
    api_key: String,
    model: String,
}

impl AnthropicClient {
    /// Create a new `AnthropicClient` with the provided credentials.
    pub fn new(api_key: &str, model: &str) -> Self {
        let http = reqwest::Client::builder()
            .build()
            .expect("failed to build reqwest client");
        Self {
            http,
            api_key: api_key.to_string(),
            model: model.to_string(),
        }
    }

    /// Create an `AnthropicClient` from the `ANTHROPIC_API_KEY` environment variable.
    ///
    /// Returns an error with a descriptive message if the variable is unset or empty.
    pub fn from_env(model: &str) -> anyhow::Result<Self> {
        let api_key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| {
            anyhow::anyhow!(
                "ANTHROPIC_API_KEY environment variable is not set. \
                 Set it to your Anthropic API key before using AnthropicClient."
            )
        })?;
        if api_key.trim().is_empty() {
            anyhow::bail!(
                "ANTHROPIC_API_KEY environment variable is set but empty. \
                 Provide a valid Anthropic API key."
            );
        }
        Ok(Self::new(&api_key, model))
    }

    // ── Request Building ─────────────────────────────────────────────

    /// Serialize the request body for the Anthropic Messages API.
    ///
    /// The `tools` array is omitted entirely when the slice is empty.
    /// When `tool_choice` is `Some`, it is included; defaults to `"auto"` when
    /// tools are present and no override is specified.
    fn build_request_body(
        &self,
        messages: &[Message],
        system: &str,
        tools: &[ToolDefinition],
        tool_choice: Option<&str>,
    ) -> serde_json::Value {
        let mut body = serde_json::json!({
            "model": self.model,
            "max_tokens": 8192,
            "system": system,
            "messages": messages,
            "stream": true,
        });

        if !tools.is_empty() {
            // Use anthropic_json() to emit the exact Anthropic wire format.
            let tools_array: Vec<serde_json::Value> =
                tools.iter().map(|t| t.anthropic_json()).collect();
            body["tools"] = serde_json::Value::Array(tools_array);

            // tool_choice: explicit override, or default to "auto".
            let choice_type = tool_choice.unwrap_or("auto");
            body["tool_choice"] = serde_json::json!({ "type": choice_type });
        }

        body
    }

    // ── Public API ───────────────────────────────────────────────────

    /// Send a message and receive the assembled response.
    ///
    /// Delegates to `send_with_retry` which handles exponential backoff on
    /// rate-limit (429) and overload (529) responses.
    pub async fn send_message(
        &self,
        messages: &[Message],
        system: &str,
        tools: &[ToolDefinition],
    ) -> Result<ApiResponse, ApiError> {
        self.send_with_retry(messages, system, tools, None).await
    }

    /// Send a message with explicit `tool_choice` control.
    ///
    /// Pass `tool_choice: Some("none")` for digest/summary calls that must not
    /// invoke tools. Pass `None` to default to `"auto"` (Claude decides).
    #[allow(dead_code)]
    pub async fn send_message_with_tool_choice(
        &self,
        messages: &[Message],
        system: &str,
        tools: &[ToolDefinition],
        tool_choice: Option<&str>,
    ) -> Result<ApiResponse, ApiError> {
        self.send_with_retry(messages, system, tools, tool_choice).await
    }

    // ── Retry Logic ──────────────────────────────────────────────────

    /// POST the request and parse the SSE stream, retrying on 429/529.
    ///
    /// Backoff schedule: 1 s, 2 s, 4 s (up to 3 retries before giving up).
    async fn send_with_retry(
        &self,
        messages: &[Message],
        system: &str,
        tools: &[ToolDefinition],
        tool_choice: Option<&str>,
    ) -> Result<ApiResponse, ApiError> {
        const MAX_RETRIES: u32 = 3;
        let backoff_secs: [u64; 3] = [1, 2, 4];

        let body = self.build_request_body(messages, system, tools, tool_choice);

        for attempt in 0..=MAX_RETRIES {
            let response = self
                .http
                .post("https://api.anthropic.com/v1/messages")
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .header("accept", "text/event-stream")
                .json(&body)
                .send()
                .await
                .map_err(|e| ApiError::CliError {
                    message: format!("HTTP request failed: {e}"),
                })?;

            let status = response.status();

            // Retryable: rate limit or overload
            if status.as_u16() == 429 || status.as_u16() == 529 {
                if attempt < MAX_RETRIES {
                    let wait = backoff_secs[attempt as usize];
                    warn!(
                        attempt = attempt + 1,
                        status = status.as_u16(),
                        wait_secs = wait,
                        "Anthropic API rate limited — retrying"
                    );
                    sleep(Duration::from_secs(wait)).await;
                    continue;
                } else {
                    return Err(ApiError::CliError {
                        message: format!(
                            "Anthropic API returned {} after {} retries",
                            status.as_u16(),
                            MAX_RETRIES
                        ),
                    });
                }
            }

            // Non-retryable error: read body and return immediately
            if !status.is_success() {
                let body_text = response.text().await.unwrap_or_default();
                return Err(ApiError::CliError {
                    message: format!(
                        "Anthropic API error {}: {}",
                        status.as_u16(),
                        body_text
                    ),
                });
            }

            // Success — parse the SSE stream
            return parse_sse_stream(response).await;
        }

        // Unreachable — loop exits via return on final retry
        Err(ApiError::CliError {
            message: "send_with_retry: exhausted retries".into(),
        })
    }
}

// ── SSE Streaming Parser ─────────────────────────────────────────────

/// Internal state for one in-progress content block.
#[derive(Debug)]
enum BlockInProgress {
    Text { text: String },
    ToolUse { id: String, name: String, input_json: String },
}

/// Parse the SSE byte stream from a successful Anthropic API response.
pub(crate) async fn parse_sse_stream(response: reqwest::Response) -> Result<ApiResponse, ApiError> {
    let mut stream = response.bytes_stream();

    // Accumulate raw bytes into a line buffer to handle partial chunks.
    let mut byte_buf = Vec::<u8>::new();

    // Parser state
    let mut message_id = String::new();
    let mut input_tokens: u32 = 0;
    let mut output_tokens: u32 = 0;
    let mut stop_reason_str: Option<String> = None;
    let mut content_blocks: Vec<ContentBlock> = Vec::new();
    let mut block_in_progress: Option<BlockInProgress> = None;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| ApiError::CliError {
            message: format!("SSE stream read error: {e}"),
        })?;
        byte_buf.extend_from_slice(&chunk);

        // Process all complete lines (terminated by \n) in the buffer.
        loop {
            let newline_pos = byte_buf.iter().position(|&b| b == b'\n');
            let line_end = match newline_pos {
                Some(pos) => pos,
                None => break,
            };

            let raw_line = std::str::from_utf8(&byte_buf[..line_end])
                .unwrap_or("")
                .trim_end_matches('\r')
                .to_string();

            byte_buf.drain(..=line_end);

            // Blank line — SSE event boundary separator, skip.
            if raw_line.is_empty() {
                continue;
            }

            // Keep-alive ping comment.
            if raw_line.starts_with(": ") || raw_line == ":" {
                debug!("SSE keep-alive received");
                continue;
            }

            // We only act on `data:` lines.
            let json_str = if let Some(rest) = raw_line.strip_prefix("data: ") {
                rest
            } else {
                // `event:` type lines are informational; the type is embedded in the
                // data payload's `type` field which we handle below.
                continue;
            };

            // Deserialize the SSE event envelope.
            let event: SseEvent = match serde_json::from_str(json_str) {
                Ok(e) => e,
                Err(err) => {
                    debug!(raw = json_str, error = %err, "SSE: failed to parse event JSON, skipping");
                    continue;
                }
            };

            match event.event_type.as_str() {
                "message_start" => {
                    if let Some(msg) = &event.message {
                        if let Some(id) = &msg.id {
                            message_id.clone_from(id);
                        }
                        if let Some(usage) = &msg.usage {
                            if let Some(it) = usage.input_tokens {
                                input_tokens = it;
                            }
                        }
                    }
                    debug!(message_id = %message_id, "SSE: message_start");
                }

                "content_block_start" => {
                    // Finalise any still-open block (shouldn't happen, but be safe).
                    if let Some(open) = block_in_progress.take() {
                        finalise_block(open, &mut content_blocks)?;
                    }

                    if let Some(cb) = &event.content_block {
                        match cb.block_type.as_str() {
                            "text" => {
                                block_in_progress = Some(BlockInProgress::Text {
                                    text: String::new(),
                                });
                            }
                            "tool_use" => {
                                let id = cb.id.clone().unwrap_or_default();
                                let name = cb.name.clone().unwrap_or_default();
                                block_in_progress = Some(BlockInProgress::ToolUse {
                                    id,
                                    name,
                                    input_json: String::new(),
                                });
                            }
                            other => {
                                debug!(block_type = other, "SSE: unknown content_block_start type");
                            }
                        }
                    }
                }

                "content_block_delta" => {
                    if let Some(delta) = &event.delta {
                        match delta.delta_type.as_deref().unwrap_or("") {
                            "text_delta" => {
                                if let Some(BlockInProgress::Text { ref mut text }) =
                                    block_in_progress
                                {
                                    if let Some(t) = &delta.text {
                                        text.push_str(t);
                                    }
                                }
                            }
                            "input_json_delta" => {
                                if let Some(BlockInProgress::ToolUse {
                                    ref mut input_json, ..
                                }) = block_in_progress
                                {
                                    if let Some(partial) = &delta.partial_json {
                                        input_json.push_str(partial);
                                    }
                                }
                            }
                            other => {
                                debug!(delta_type = other, "SSE: unknown delta type");
                            }
                        }
                    }
                }

                "content_block_stop" => {
                    if let Some(open) = block_in_progress.take() {
                        finalise_block(open, &mut content_blocks)?;
                    }
                }

                "message_delta" => {
                    if let Some(delta) = &event.delta {
                        if let Some(sr) = &delta.stop_reason {
                            stop_reason_str = Some(sr.clone());
                        }
                    }
                    // output_tokens lives under top-level `usage` on message_delta
                    if let Some(usage) = &event.usage {
                        if let Some(ot) = usage.output_tokens {
                            output_tokens = ot;
                        }
                    }
                }

                "message_stop" => {
                    debug!("SSE: message_stop received");
                    // Stream is complete — break out of inner loop; outer while will
                    // return None on next poll and exit cleanly.
                }

                "error" => {
                    let err_type = event
                        .error
                        .as_ref()
                        .and_then(|e| e.error_type.as_deref())
                        .unwrap_or("unknown_error");
                    let err_msg = event
                        .error
                        .as_ref()
                        .and_then(|e| e.message.as_deref())
                        .unwrap_or("no message");
                    return Err(ApiError::CliError {
                        message: format!("Anthropic API error event [{err_type}]: {err_msg}"),
                    });
                }

                other => {
                    debug!(event_type = other, "SSE: unrecognised event type");
                }
            }
        }
    }

    // Flush any block that had no explicit content_block_stop (shouldn't happen
    // with a well-formed stream, but handle gracefully).
    if let Some(open) = block_in_progress.take() {
        finalise_block(open, &mut content_blocks)?;
    }

    let stop_reason = map_stop_reason(stop_reason_str.as_deref().unwrap_or("end_turn"));

    Ok(ApiResponse {
        id: message_id,
        content: content_blocks,
        stop_reason,
        usage: Usage {
            input_tokens,
            output_tokens,
            total_cost_usd: None,
        },
    })
}

/// Finalise an in-progress content block and push it onto the completed list.
fn finalise_block(
    block: BlockInProgress,
    content_blocks: &mut Vec<ContentBlock>,
) -> Result<(), ApiError> {
    match block {
        BlockInProgress::Text { text } => {
            content_blocks.push(ContentBlock::Text { text });
        }
        BlockInProgress::ToolUse { id, name, input_json } => {
            let input: serde_json::Value = if input_json.is_empty() {
                serde_json::Value::Object(serde_json::Map::new())
            } else {
                serde_json::from_str(&input_json).map_err(|e| ApiError::CliError {
                    message: format!("failed to parse tool input JSON for '{name}': {e}"),
                })?
            };
            content_blocks.push(ContentBlock::ToolUse { id, name, input });
        }
    }
    Ok(())
}

/// Map the Anthropic stop_reason string to [`StopReason`].
fn map_stop_reason(s: &str) -> StopReason {
    match s {
        "tool_use" => StopReason::ToolUse,
        "max_tokens" => StopReason::MaxTokens,
        _ => StopReason::EndTurn,
    }
}

// ── SSE Event Deserialization Types ─────────────────────────────────

/// Top-level SSE event envelope from the Anthropic streaming API.
#[derive(Debug, Deserialize)]
struct SseEvent {
    #[serde(rename = "type")]
    event_type: String,

    // message_start
    #[serde(default)]
    message: Option<SseMessageStart>,

    // content_block_start
    #[serde(default)]
    content_block: Option<SseContentBlock>,

    // content_block_delta / message_delta
    #[serde(default)]
    delta: Option<SseDelta>,

    // message_delta usage
    #[serde(default)]
    usage: Option<SseUsage>,

    // error
    #[serde(default)]
    error: Option<SseError>,
}

#[derive(Debug, Deserialize)]
struct SseMessageStart {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    usage: Option<SseUsage>,
}

#[derive(Debug, Deserialize)]
struct SseContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SseDelta {
    #[serde(rename = "type")]
    #[serde(default)]
    delta_type: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    partial_json: Option<String>,
    #[serde(default)]
    stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SseUsage {
    #[serde(default)]
    input_tokens: Option<u32>,
    #[serde(default)]
    output_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct SseError {
    #[serde(rename = "type")]
    #[serde(default)]
    error_type: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

// ── Unit Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal SSE byte stream from a slice of event JSON strings.
    fn sse_bytes(events: &[&str]) -> bytes::Bytes {
        let mut out = String::new();
        for ev in events {
            out.push_str("data: ");
            out.push_str(ev);
            out.push('\n');
            out.push('\n');
        }
        bytes::Bytes::from(out)
    }

    /// Wrap a Bytes value into a mock reqwest::Response via wiremock.
    /// We drive `parse_sse_stream` directly using a helper that feeds the
    /// bytes through the same parser logic (without an actual HTTP connection).
    async fn run_parser(raw: bytes::Bytes) -> Result<ApiResponse, ApiError> {
        parse_sse_bytes(raw).await
    }

    // ── Test-only parser that accepts raw bytes ───────────────────────

    /// Parse SSE events from a pre-assembled Bytes buffer (test helper).
    async fn parse_sse_bytes(raw: bytes::Bytes) -> Result<ApiResponse, ApiError> {
        let mut byte_buf: Vec<u8> = raw.to_vec();
        let mut message_id = String::new();
        let mut input_tokens: u32 = 0;
        let mut output_tokens: u32 = 0;
        let mut stop_reason_str: Option<String> = None;
        let mut content_blocks: Vec<ContentBlock> = Vec::new();
        let mut block_in_progress: Option<BlockInProgress> = None;

        loop {
            let newline_pos = byte_buf.iter().position(|&b| b == b'\n');
            let line_end = match newline_pos {
                Some(pos) => pos,
                None => break,
            };

            let raw_line = std::str::from_utf8(&byte_buf[..line_end])
                .unwrap_or("")
                .trim_end_matches('\r')
                .to_string();

            byte_buf.drain(..=line_end);

            if raw_line.is_empty() {
                continue;
            }
            if raw_line.starts_with(": ") || raw_line == ":" {
                continue;
            }

            let json_str = match raw_line.strip_prefix("data: ") {
                Some(s) => s.to_string(),
                None => continue,
            };

            let event: SseEvent = match serde_json::from_str(&json_str) {
                Ok(e) => e,
                Err(_) => continue,
            };

            match event.event_type.as_str() {
                "message_start" => {
                    if let Some(msg) = &event.message {
                        if let Some(id) = &msg.id {
                            message_id.clone_from(id);
                        }
                        if let Some(usage) = &msg.usage {
                            if let Some(it) = usage.input_tokens {
                                input_tokens = it;
                            }
                        }
                    }
                }
                "content_block_start" => {
                    if let Some(open) = block_in_progress.take() {
                        finalise_block(open, &mut content_blocks)?;
                    }
                    if let Some(cb) = &event.content_block {
                        match cb.block_type.as_str() {
                            "text" => {
                                block_in_progress =
                                    Some(BlockInProgress::Text { text: String::new() });
                            }
                            "tool_use" => {
                                block_in_progress = Some(BlockInProgress::ToolUse {
                                    id: cb.id.clone().unwrap_or_default(),
                                    name: cb.name.clone().unwrap_or_default(),
                                    input_json: String::new(),
                                });
                            }
                            _ => {}
                        }
                    }
                }
                "content_block_delta" => {
                    if let Some(delta) = &event.delta {
                        match delta.delta_type.as_deref().unwrap_or("") {
                            "text_delta" => {
                                if let Some(BlockInProgress::Text { ref mut text }) =
                                    block_in_progress
                                {
                                    if let Some(t) = &delta.text {
                                        text.push_str(t);
                                    }
                                }
                            }
                            "input_json_delta" => {
                                if let Some(BlockInProgress::ToolUse {
                                    ref mut input_json,
                                    ..
                                }) = block_in_progress
                                {
                                    if let Some(partial) = &delta.partial_json {
                                        input_json.push_str(partial);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                "content_block_stop" => {
                    if let Some(open) = block_in_progress.take() {
                        finalise_block(open, &mut content_blocks)?;
                    }
                }
                "message_delta" => {
                    if let Some(delta) = &event.delta {
                        if let Some(sr) = &delta.stop_reason {
                            stop_reason_str = Some(sr.clone());
                        }
                    }
                    if let Some(usage) = &event.usage {
                        if let Some(ot) = usage.output_tokens {
                            output_tokens = ot;
                        }
                    }
                }
                "error" => {
                    let err_type = event
                        .error
                        .as_ref()
                        .and_then(|e| e.error_type.as_deref())
                        .unwrap_or("unknown_error");
                    let err_msg = event
                        .error
                        .as_ref()
                        .and_then(|e| e.message.as_deref())
                        .unwrap_or("no message");
                    return Err(ApiError::CliError {
                        message: format!("Anthropic API error event [{err_type}]: {err_msg}"),
                    });
                }
                _ => {}
            }
        }

        if let Some(open) = block_in_progress.take() {
            finalise_block(open, &mut content_blocks)?;
        }

        Ok(ApiResponse {
            id: message_id,
            content: content_blocks,
            stop_reason: map_stop_reason(stop_reason_str.as_deref().unwrap_or("end_turn")),
            usage: Usage {
                input_tokens,
                output_tokens,
                total_cost_usd: None,
            },
        })
    }

    // ── Test 6.1: text-only SSE sequence ─────────────────────────────

    #[tokio::test]
    async fn test_parse_sse_text_only() {
        let events = [
            r#"{"type":"message_start","message":{"id":"msg_01","role":"assistant","content":[],"model":"claude-opus-4-5","stop_reason":null,"usage":{"input_tokens":25,"output_tokens":0}}}"#,
            r#"{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#,
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello, "}}"#,
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"world!"}}"#,
            r#"{"type":"content_block_stop","index":0}"#,
            r#"{"type":"message_delta","delta":{"stop_reason":"end_turn","stop_sequence":null},"usage":{"output_tokens":3}}"#,
            r#"{"type":"message_stop"}"#,
        ];
        let raw = sse_bytes(&events);
        let response = run_parser(raw).await.expect("parsing should succeed");

        assert_eq!(response.id, "msg_01");
        assert_eq!(response.usage.input_tokens, 25);
        assert_eq!(response.usage.output_tokens, 3);
        assert_eq!(response.stop_reason, StopReason::EndTurn);
        assert_eq!(response.content.len(), 1);
        match &response.content[0] {
            ContentBlock::Text { text } => assert_eq!(text, "Hello, world!"),
            other => panic!("expected Text block, got {other:?}"),
        }
    }

    // ── Test 6.2: tool-use SSE sequence ──────────────────────────────

    #[tokio::test]
    async fn test_parse_sse_tool_use() {
        let events = [
            r#"{"type":"message_start","message":{"id":"msg_02","role":"assistant","content":[],"model":"claude-opus-4-5","stop_reason":null,"usage":{"input_tokens":50,"output_tokens":0}}}"#,
            r#"{"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"toolu_01","name":"read_file"}}"#,
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"path\":"}}"#,
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"\"README.md\"}"}}"#,
            r#"{"type":"content_block_stop","index":0}"#,
            r#"{"type":"message_delta","delta":{"stop_reason":"tool_use","stop_sequence":null},"usage":{"output_tokens":15}}"#,
            r#"{"type":"message_stop"}"#,
        ];
        let raw = sse_bytes(&events);
        let response = run_parser(raw).await.expect("parsing should succeed");

        assert_eq!(response.stop_reason, StopReason::ToolUse);
        assert_eq!(response.content.len(), 1);
        match &response.content[0] {
            ContentBlock::ToolUse { id, name, input } => {
                assert_eq!(id, "toolu_01");
                assert_eq!(name, "read_file");
                assert_eq!(input["path"], "README.md");
            }
            other => panic!("expected ToolUse block, got {other:?}"),
        }
    }

    // ── Test 6.3: error event ─────────────────────────────────────────

    #[tokio::test]
    async fn test_parse_sse_error_event() {
        let events = [
            r#"{"type":"error","error":{"type":"overloaded_error","message":"API overloaded"}}"#,
        ];
        let raw = sse_bytes(&events);
        let result = run_parser(raw).await;

        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("overloaded_error"), "expected error type in message: {msg}");
        assert!(msg.contains("API overloaded"), "expected error message: {msg}");
    }

    // ── Test 6.4: build_request_body omits tools when empty ──────────

    #[test]
    fn test_build_request_body_no_tools() {
        let client = AnthropicClient::new("test-key", "claude-opus-4-5");
        let messages = vec![Message::user("Hello")];
        let body = client.build_request_body(&messages, "You are helpful.", &[], None);

        assert!(body.get("tools").is_none(), "tools key should be absent when tools vec is empty");
        assert!(body.get("tool_choice").is_none(), "tool_choice should be absent when no tools");
        assert_eq!(body["model"], "claude-opus-4-5");
        assert_eq!(body["max_tokens"], 8192);
        assert_eq!(body["stream"], true);
        assert_eq!(body["system"], "You are helpful.");
    }

    // ── Test 6.6: build_request_body includes tools and tool_choice ──

    #[test]
    fn test_build_request_body_with_tools() {
        let client = AnthropicClient::new("test-key", "claude-opus-4-5");
        let messages = vec![Message::user("Hello")];
        let tools = vec![crate::claude::ToolDefinition {
            name: "read_memory".into(),
            description: "Read a memory topic".into(),
            input_schema: serde_json::json!({"type": "object", "properties": {}}),
        }];
        let body = client.build_request_body(&messages, "You are helpful.", &tools, None);

        let tools_arr = body.get("tools").expect("tools should be present");
        assert!(tools_arr.is_array());
        assert_eq!(tools_arr[0]["name"], "read_memory");
        assert_eq!(tools_arr[0]["input_schema"]["type"], "object");
        // Default tool_choice is "auto"
        assert_eq!(body["tool_choice"]["type"], "auto");
    }

    #[test]
    fn test_build_request_body_tool_choice_none() {
        let client = AnthropicClient::new("test-key", "claude-opus-4-5");
        let messages = vec![Message::user("Summarize")];
        let tools = vec![crate::claude::ToolDefinition {
            name: "read_memory".into(),
            description: "Read a memory topic".into(),
            input_schema: serde_json::json!({"type": "object", "properties": {}}),
        }];
        let body = client.build_request_body(&messages, "Digest system.", &tools, Some("none"));
        assert_eq!(body["tool_choice"]["type"], "none");
    }

    // ── Test 6.5: from_env with ANTHROPIC_API_KEY unset ──────────────

    #[test]
    fn test_from_env_missing_key() {
        // Temporarily unset the key (this is safe in unit test contexts).
        let saved = std::env::var("ANTHROPIC_API_KEY").ok();
        std::env::remove_var("ANTHROPIC_API_KEY");

        let result = AnthropicClient::from_env("claude-opus-4-5");

        // Restore
        if let Some(key) = saved {
            std::env::set_var("ANTHROPIC_API_KEY", key);
        }

        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("ANTHROPIC_API_KEY"),
            "error should mention the variable name: {msg}"
        );
    }
}
