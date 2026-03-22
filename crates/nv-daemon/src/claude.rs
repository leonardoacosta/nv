use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;

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
    #[serde(default)]
    pub total_cost_usd: Option<f64>,
}

/// Tool definition in the Anthropic API format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

// ── CLI JSON Response Types (cold-start fallback) ───────────────────

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
    #[serde(default)]
    total_cost_usd: Option<f64>,
}

// ── Stream-JSON Types ───────────────────────────────────────────────

/// Input message for stream-json format (sent to stdin).
#[derive(Debug, Serialize)]
struct StreamJsonInput {
    #[serde(rename = "type")]
    msg_type: String,
    content: String,
}

/// Parsed stream-json event from stdout.
#[derive(Debug, Deserialize)]
struct StreamJsonEvent {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(default)]
    subtype: String,
    // Text content fields
    #[serde(default)]
    text: String,
    // Tool use fields
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    input: Option<serde_json::Value>,
    // Result fields
    #[serde(default)]
    result: String,
    #[serde(default)]
    session_id: String,
    #[serde(default)]
    usage: Option<StreamJsonUsage>,
    #[serde(default)]
    is_error: bool,
}

#[derive(Debug, Default, Deserialize)]
struct StreamJsonUsage {
    #[serde(default)]
    input_tokens: u32,
    #[serde(default)]
    output_tokens: u32,
    #[serde(default)]
    total_cost_usd: Option<f64>,
}

// ── Persistent Process ──────────────────────────────────────────────

/// A live Claude CLI subprocess with stream-json I/O.
struct PersistentProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

/// Configuration for spawning the persistent subprocess.
#[derive(Clone)]
struct SpawnConfig {
    model: String,
    real_home: String,
    sandbox_home: String,
}

impl SpawnConfig {
    fn new(model: &str) -> Self {
        let real_home = std::env::var("REAL_HOME")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_else(|_| "/home/nyaptor".into());
        let sandbox_home = format!("{real_home}/.nv/claude-sandbox");
        Self {
            model: model.to_string(),
            real_home,
            sandbox_home,
        }
    }
}

/// Spawn a persistent Claude CLI subprocess with stream-json I/O.
fn spawn_persistent(config: &SpawnConfig) -> Result<PersistentProcess, ApiError> {
    let mut child = Command::new("claude")
        .args([
            "--dangerously-skip-permissions",
            "-p",
            "--input-format",
            "stream-json",
            "--output-format",
            "stream-json",
            "--model",
            &config.model,
            "--no-session-persistence",
            "--tools",
            "Read,Glob,Grep,Bash(git:*)",
            "--strict-mcp-config",
        ])
        .env("HOME", &config.sandbox_home)
        .env(
            "PATH",
            format!(
                "{}/.local/bin:/usr/local/bin:/usr/bin:/bin",
                config.real_home
            ),
        )
        .current_dir(format!("{}/dev", config.real_home))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| ApiError::CliError {
            message: format!("Failed to spawn persistent claude CLI: {e}"),
        })?;

    let stdin = child.stdin.take().ok_or_else(|| ApiError::CliError {
        message: "Failed to capture stdin of persistent subprocess".into(),
    })?;

    let stdout = child.stdout.take().ok_or_else(|| ApiError::CliError {
        message: "Failed to capture stdout of persistent subprocess".into(),
    })?;

    // Spawn a task to drain stderr so it doesn't block the pipe
    if let Some(stderr) = child.stderr.take() {
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::debug!(stderr = %line, "claude persistent stderr");
            }
        });
    }

    Ok(PersistentProcess {
        child,
        stdin,
        stdout: BufReader::new(stdout),
    })
}

// ── Persistent Session ──────────────────────────────────────────────

/// Backoff configuration for auto-restart.
struct BackoffState {
    consecutive_failures: u32,
    next_delay: Duration,
}

impl BackoffState {
    fn new() -> Self {
        Self {
            consecutive_failures: 0,
            next_delay: Duration::from_secs(1),
        }
    }

    fn record_failure(&mut self) {
        self.consecutive_failures += 1;
        // Exponential backoff: 1s, 2s, 4s, 8s, 16s, 30s (capped)
        self.next_delay = Duration::from_secs(
            (1u64 << self.consecutive_failures.min(5)).min(30),
        );
    }

    fn record_success(&mut self) {
        self.consecutive_failures = 0;
        self.next_delay = Duration::from_secs(1);
    }

    fn should_fallback(&self) -> bool {
        self.consecutive_failures >= 5
    }

    fn delay(&self) -> Duration {
        self.next_delay
    }
}

/// Internal state for the persistent session, protected by a Mutex.
struct SessionInner {
    process: Option<PersistentProcess>,
    config: SpawnConfig,
    backoff: BackoffState,
    /// True when persistent mode has been permanently disabled (5 consecutive failures).
    fallback_only: bool,
}

/// A persistent Claude CLI session that keeps the subprocess alive between turns.
///
/// Thread-safe via internal `Mutex`. Falls back to cold-start mode on failure.
pub struct PersistentSession {
    inner: Mutex<SessionInner>,
}

impl PersistentSession {
    /// Create a new persistent session (does not spawn the subprocess yet).
    fn new(config: SpawnConfig) -> Self {
        Self {
            inner: Mutex::new(SessionInner {
                process: None,
                config,
                backoff: BackoffState::new(),
                fallback_only: false,
            }),
        }
    }

    /// Ensure the subprocess is alive, spawning or restarting as needed.
    async fn ensure_alive(inner: &mut SessionInner) -> bool {
        if inner.fallback_only {
            return false;
        }

        // Check if existing process is still alive
        if let Some(proc) = &mut inner.process {
            match proc.child.try_wait() {
                Ok(None) => return true, // Still running
                Ok(Some(status)) => {
                    tracing::warn!(
                        exit_status = ?status,
                        "persistent subprocess exited unexpectedly"
                    );
                    inner.process = None;
                }
                Err(e) => {
                    tracing::warn!(error = %e, "failed to check persistent subprocess status");
                    inner.process = None;
                }
            }
        }

        // Need to spawn (or respawn after death)
        if inner.backoff.consecutive_failures > 0 {
            let delay = inner.backoff.delay();
            tracing::info!(
                delay_ms = delay.as_millis(),
                failures = inner.backoff.consecutive_failures,
                "backing off before respawn"
            );
            tokio::time::sleep(delay).await;
        }

        match spawn_persistent(&inner.config) {
            Ok(proc) => {
                tracing::info!("persistent subprocess spawned");
                inner.process = Some(proc);
                // Don't reset backoff until a successful turn completes
                true
            }
            Err(e) => {
                tracing::error!(error = %e, "failed to spawn persistent subprocess");
                inner.backoff.record_failure();
                if inner.backoff.should_fallback() {
                    tracing::warn!(
                        "persistent mode disabled after {} consecutive failures, switching to cold-start fallback",
                        inner.backoff.consecutive_failures
                    );
                    inner.fallback_only = true;
                }
                false
            }
        }
    }

    /// Send a turn via the persistent subprocess. Returns None if the subprocess
    /// is unavailable (caller should fall back to cold-start).
    async fn send_turn(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Option<Result<ApiResponse>> {
        let mut inner = self.inner.lock().await;

        if !Self::ensure_alive(&mut inner).await {
            return None;
        }

        let proc = inner.process.as_mut()?;

        // Build the user message: last message content for the turn.
        // The persistent subprocess maintains conversation state, so we only send
        // the latest user message (the CLI accumulates context internally).
        //
        // For the stream-json format, we write a JSON object per line to stdin.
        // The system prompt is passed via --system-prompt flag at spawn time,
        // so here we only send the user turn content.
        let user_content = build_stream_input(system, messages, tools);

        let input = StreamJsonInput {
            msg_type: "user".to_string(),
            content: user_content,
        };

        let json_line = match serde_json::to_string(&input) {
            Ok(j) => j,
            Err(e) => {
                tracing::error!(error = %e, "failed to serialize stream-json input");
                return Some(Err(ApiError::Deserialize(format!(
                    "failed to serialize input: {e}"
                ))
                .into()));
            }
        };

        // Write to stdin
        if let Err(e) = proc.stdin.write_all(json_line.as_bytes()).await {
            tracing::error!(error = %e, "failed to write to persistent subprocess stdin");
            inner.process = None;
            inner.backoff.record_failure();
            if inner.backoff.should_fallback() {
                inner.fallback_only = true;
            }
            return None;
        }
        if let Err(e) = proc.stdin.write_all(b"\n").await {
            tracing::error!(error = %e, "failed to write newline to stdin");
            inner.process = None;
            inner.backoff.record_failure();
            if inner.backoff.should_fallback() {
                inner.fallback_only = true;
            }
            return None;
        }
        if let Err(e) = proc.stdin.flush().await {
            tracing::error!(error = %e, "failed to flush stdin");
            inner.process = None;
            inner.backoff.record_failure();
            if inner.backoff.should_fallback() {
                inner.fallback_only = true;
            }
            return None;
        }

        // Read response events from stdout until we get a "result" event
        let result = read_stream_response(&mut proc.stdout).await;

        match &result {
            Ok(_) => {
                inner.backoff.record_success();
            }
            Err(e) => {
                tracing::error!(error = %e, "persistent subprocess response read failed");
                // Kill the process — it may be in a bad state
                inner.process = None;
                inner.backoff.record_failure();
                if inner.backoff.should_fallback() {
                    inner.fallback_only = true;
                }
                return None;
            }
        }

        Some(result)
    }
}

/// Read stream-json events from stdout until a "result" event arrives.
/// Delegates to the generic `read_stream_response_from_lines` implementation.
async fn read_stream_response(
    stdout: &mut BufReader<ChildStdout>,
) -> Result<ApiResponse> {
    read_stream_response_from_lines(stdout).await
}

/// Build the user content for a stream-json turn.
///
/// Since the persistent subprocess manages conversation state via the CLI,
/// we pass the full context (system + history + tools) in each user message.
/// This ensures every turn has the full context even though the subprocess
/// stays alive.
fn build_stream_input(system: &str, messages: &[Message], tools: &[ToolDefinition]) -> String {
    build_prompt(system, messages, tools)
}

// ── Claude CLI Client ───────────────────────────────────────────────

/// Claude CLI client with persistent subprocess and cold-start fallback.
///
/// The client tries to use a persistent subprocess (stream-json mode) for
/// each turn. If the subprocess is dead or unavailable, it falls back to
/// spawning a fresh `claude -p` per turn (cold-start mode).
pub struct ClaudeClient {
    model: String,
    #[allow(dead_code)]
    max_tokens: u32,
    session: Arc<PersistentSession>,
    spawn_config: SpawnConfig,
}

// ClaudeClient needs Clone for the agent loop. The Arc<PersistentSession>
// makes this cheap — all clones share the same session.
impl Clone for ClaudeClient {
    fn clone(&self) -> Self {
        Self {
            model: self.model.clone(),
            max_tokens: self.max_tokens,
            session: Arc::clone(&self.session),
            spawn_config: self.spawn_config.clone(),
        }
    }
}

impl ClaudeClient {
    /// Return the configured model name.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Create a new client. The `_api_key` parameter is ignored — the CLI uses
    /// its own OAuth session. Kept for backward-compatible constructor signature.
    pub fn new(_api_key: String, model: String, max_tokens: u32) -> Self {
        let spawn_config = SpawnConfig::new(&model);
        let session = Arc::new(PersistentSession::new(spawn_config.clone()));
        Self {
            model,
            max_tokens,
            session,
            spawn_config,
        }
    }

    /// Send a messages request via the Claude CLI.
    ///
    /// Tries the persistent subprocess first. Falls back to cold-start
    /// `claude -p` if the persistent session is unavailable.
    pub async fn send_messages(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<ApiResponse> {
        // Try persistent path first
        if let Some(result) = self.session.send_turn(system, messages, tools).await {
            return result;
        }

        // Fallback to cold-start mode
        tracing::warn!("using cold-start fallback for this turn");
        self.send_messages_cold_start(system, messages, tools).await
    }

    /// Cold-start fallback: spawn a fresh `claude -p` subprocess per turn.
    /// This is the original implementation, kept as a reliable fallback.
    async fn send_messages_cold_start(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<ApiResponse> {
        let prompt = build_prompt(system, messages, tools);

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
                "Read,Glob,Grep,Bash(git:*)",
                "--system-prompt",
                system,
                "--strict-mcp-config",
            ])
            .env("HOME", &self.spawn_config.sandbox_home)
            .env(
                "PATH",
                format!(
                    "{}/.local/bin:/usr/local/bin:/usr/bin:/bin",
                    self.spawn_config.real_home
                ),
            )
            .current_dir(format!("{}/dev", self.spawn_config.real_home))
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

        // Parse CLI JSON response with retry on empty/truncated output
        let cli_response: CliJsonResponse = match serde_json::from_str(&stdout) {
            Ok(resp) => resp,
            Err(first_err) => {
                tracing::warn!(
                    stdout_len = stdout.len(),
                    error = %first_err,
                    "CLI JSON parse failed, retrying once after 1s"
                );

                // Retry: spawn a fresh subprocess
                tokio::time::sleep(Duration::from_secs(1)).await;

                let mut retry_child = Command::new("claude")
                    .args([
                        "--dangerously-skip-permissions",
                        "-p",
                        "--output-format",
                        "json",
                        "--model",
                        &self.model,
                        "--no-session-persistence",
                        "--tools",
                        "Read,Glob,Grep,Bash(git:*)",
                        "--system-prompt",
                        system,
                        "--strict-mcp-config",
                    ])
                    .env("HOME", &self.spawn_config.sandbox_home)
                    .env(
                        "PATH",
                        format!(
                            "{}/.local/bin:/usr/local/bin:/usr/bin:/bin",
                            self.spawn_config.real_home
                        ),
                    )
                    .current_dir(format!("{}/dev", self.spawn_config.real_home))
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .map_err(|e| ApiError::CliError {
                        message: format!("Failed to spawn claude CLI (retry): {e}"),
                    })?;

                if let Some(mut stdin) = retry_child.stdin.take() {
                    stdin.write_all(prompt.as_bytes()).await?;
                    drop(stdin);
                }

                let retry_output = retry_child.wait_with_output().await?;
                let retry_stdout =
                    String::from_utf8_lossy(&retry_output.stdout).to_string();

                serde_json::from_str(&retry_stdout).map_err(|retry_err| {
                    tracing::error!(
                        retry_stdout_len = retry_stdout.len(),
                        first_error = %first_err,
                        retry_error = %retry_err,
                        "CLI JSON parse failed on retry — returning error"
                    );
                    ApiError::Deserialize(format!(
                        "CLI JSON parse failed after retry: {retry_err} (first attempt: {first_err})"
                    ))
                })?
            }
        };

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
            "Claude CLI cold-start response received"
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
                total_cost_usd: cli_response.usage.total_cost_usd,
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
        let role_label = if msg.role == "user" {
            "User"
        } else {
            "Assistant"
        };
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

/// Helper type for constructing tool result blocks before wrapping
/// into a `ContentBlock::ToolResult`.
#[derive(Debug, Clone)]
pub struct ToolResultBlock {
    pub tool_use_id: String,
    pub content: String,
    pub is_error: bool,
}

// ── Generic stream response reader ──────────────────────────────────

/// Read stream-json events from any async buffered reader until a "result" event.
/// Accumulates text + tool_use blocks into an `ApiResponse`.
///
/// Used by both the production `read_stream_response` (with `BufReader<ChildStdout>`)
/// and tests (with `tokio::io::DuplexStream`).
async fn read_stream_response_from_lines<R: tokio::io::AsyncBufRead + Unpin>(
    reader: &mut R,
) -> Result<ApiResponse> {
    let mut content_blocks: Vec<ContentBlock> = Vec::new();
    let mut current_text = String::new();
    let mut usage = Usage {
        input_tokens: 0,
        output_tokens: 0,
        total_cost_usd: None,
    };
    let mut stop_reason = StopReason::EndTurn;
    let mut line = String::new();

    let timeout = Duration::from_secs(300);

    loop {
        line.clear();
        let read_result = tokio::time::timeout(timeout, reader.read_line(&mut line)).await;

        match read_result {
            Err(_) => {
                return Err(ApiError::CliError {
                    message: "Timeout waiting for persistent subprocess response".into(),
                }
                .into());
            }
            Ok(Err(e)) => {
                return Err(ApiError::CliError {
                    message: format!("IO error reading from persistent subprocess: {e}"),
                }
                .into());
            }
            Ok(Ok(0)) => {
                return Err(ApiError::CliError {
                    message: "Persistent subprocess closed stdout (process died)".into(),
                }
                .into());
            }
            Ok(Ok(_)) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                let event: StreamJsonEvent = match serde_json::from_str(trimmed) {
                    Ok(e) => e,
                    Err(e) => {
                        tracing::debug!(
                            line = %trimmed,
                            error = %e,
                            "skipping unparseable stream-json line"
                        );
                        continue;
                    }
                };

                match event.event_type.as_str() {
                    "assistant" => match event.subtype.as_str() {
                        "text" => {
                            current_text.push_str(&event.text);
                        }
                        "tool_use" => {
                            if !current_text.is_empty() {
                                content_blocks.push(ContentBlock::Text {
                                    text: std::mem::take(&mut current_text),
                                });
                            }
                            content_blocks.push(ContentBlock::ToolUse {
                                id: event.id,
                                name: event.name,
                                input: event.input.unwrap_or(serde_json::Value::Null),
                            });
                            stop_reason = StopReason::ToolUse;
                        }
                        _ => {}
                    },
                    "result" => {
                        if !current_text.is_empty() {
                            content_blocks.push(ContentBlock::Text {
                                text: std::mem::take(&mut current_text),
                            });
                        }

                        let session_id = event.session_id;
                        if let Some(u) = event.usage {
                            usage = Usage {
                                input_tokens: u.input_tokens,
                                output_tokens: u.output_tokens,
                                total_cost_usd: u.total_cost_usd,
                            };
                        }

                        if event.is_error {
                            if event.result.to_lowercase().contains("not logged in") {
                                return Err(ApiError::AuthError(event.result).into());
                            }
                            return Err(ApiError::CliError {
                                message: event.result,
                            }
                            .into());
                        }

                        if content_blocks.is_empty() && !event.result.is_empty() {
                            let (parsed_content, parsed_reason) =
                                parse_tool_calls(&event.result);
                            content_blocks = parsed_content;
                            stop_reason = parsed_reason;
                        }

                        if stop_reason != StopReason::ToolUse {
                            stop_reason = match event.subtype.as_str() {
                                "max_tokens" => StopReason::MaxTokens,
                                _ => StopReason::EndTurn,
                            };
                        }

                        tracing::debug!(
                            input_tokens = usage.input_tokens,
                            output_tokens = usage.output_tokens,
                            session_id = %session_id,
                            "stream response received"
                        );

                        return Ok(ApiResponse {
                            id: session_id,
                            content: content_blocks,
                            stop_reason,
                            usage,
                        });
                    }
                    _ => {}
                }
            }
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Existing tests (preserved) ──────────────────────────────────

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

    // ── New tests: Stream-JSON event parsing ────────────────────────

    #[test]
    fn stream_json_event_text_deserialization() {
        let json = r#"{"type":"assistant","subtype":"text","text":"Hello there!"}"#;
        let event: StreamJsonEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.event_type, "assistant");
        assert_eq!(event.subtype, "text");
        assert_eq!(event.text, "Hello there!");
    }

    #[test]
    fn stream_json_event_tool_use_deserialization() {
        let json = r#"{"type":"assistant","subtype":"tool_use","id":"tu-123","name":"read_memory","input":{"topic":"tasks"}}"#;
        let event: StreamJsonEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.event_type, "assistant");
        assert_eq!(event.subtype, "tool_use");
        assert_eq!(event.id, "tu-123");
        assert_eq!(event.name, "read_memory");
        assert_eq!(event.input.unwrap()["topic"], "tasks");
    }

    #[test]
    fn stream_json_event_result_deserialization() {
        let json = r#"{"type":"result","subtype":"success","result":"done","session_id":"sess-1","usage":{"input_tokens":50,"output_tokens":20},"is_error":false}"#;
        let event: StreamJsonEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.event_type, "result");
        assert_eq!(event.subtype, "success");
        assert_eq!(event.result, "done");
        assert_eq!(event.session_id, "sess-1");
        assert!(!event.is_error);
        let usage = event.usage.unwrap();
        assert_eq!(usage.input_tokens, 50);
        assert_eq!(usage.output_tokens, 20);
    }

    #[test]
    fn stream_json_event_result_error() {
        let json = r#"{"type":"result","subtype":"error","result":"not logged in","session_id":"","usage":{"input_tokens":0,"output_tokens":0},"is_error":true}"#;
        let event: StreamJsonEvent = serde_json::from_str(json).unwrap();
        assert!(event.is_error);
        assert!(event.result.contains("not logged in"));
    }

    #[test]
    fn stream_json_event_unknown_fields_ignored() {
        let json = r#"{"type":"assistant","subtype":"text","text":"hi","extra_field":"ignored"}"#;
        let event: StreamJsonEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.text, "hi");
    }

    // ── New tests: Stream-JSON input serialization ──────────────────

    #[test]
    fn stream_json_input_serialization() {
        let input = StreamJsonInput {
            msg_type: "user".to_string(),
            content: "hello world".to_string(),
        };
        let json = serde_json::to_string(&input).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["type"], "user");
        assert_eq!(parsed["content"], "hello world");
    }

    #[test]
    fn stream_json_input_roundtrip() {
        let input = StreamJsonInput {
            msg_type: "user".to_string(),
            content: "test with \"quotes\" and\nnewlines".to_string(),
        };
        let json = serde_json::to_string(&input).unwrap();
        // Ensure it's a single line (no unescaped newlines)
        assert!(!json.contains('\n') || json.contains("\\n"));
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["type"], "user");
    }

    // ── New tests: Stream response reader ───────────────────────────

    #[tokio::test]
    async fn read_stream_response_text_only() {
        use tokio::io::AsyncWriteExt;

        let (reader, mut writer) = tokio::io::duplex(8192);
        let mut buf_reader = BufReader::new(reader);

        // Simulate stream events
        tokio::spawn(async move {
            let events = vec![
                r#"{"type":"assistant","subtype":"text","text":"Hello "}"#,
                r#"{"type":"assistant","subtype":"text","text":"world!"}"#,
                r#"{"type":"result","subtype":"success","result":"","session_id":"s1","usage":{"input_tokens":10,"output_tokens":5},"is_error":false}"#,
            ];
            for event in events {
                writer.write_all(event.as_bytes()).await.unwrap();
                writer.write_all(b"\n").await.unwrap();
            }
        });

        let response = read_stream_response_from_lines(&mut buf_reader).await.unwrap();
        assert_eq!(response.id, "s1");
        assert_eq!(response.content.len(), 1);
        match &response.content[0] {
            ContentBlock::Text { text } => assert_eq!(text, "Hello world!"),
            _ => panic!("expected text block"),
        }
        assert_eq!(response.stop_reason, StopReason::EndTurn);
        assert_eq!(response.usage.input_tokens, 10);
        assert_eq!(response.usage.output_tokens, 5);
    }

    #[tokio::test]
    async fn read_stream_response_with_tool_use() {
        use tokio::io::AsyncWriteExt;

        let (reader, mut writer) = tokio::io::duplex(8192);
        let mut buf_reader = BufReader::new(reader);

        tokio::spawn(async move {
            let events = vec![
                r#"{"type":"assistant","subtype":"text","text":"Let me check."}"#,
                r#"{"type":"assistant","subtype":"tool_use","id":"tu-1","name":"read_memory","input":{"topic":"tasks"}}"#,
                r#"{"type":"result","subtype":"success","result":"","session_id":"s2","usage":{"input_tokens":20,"output_tokens":10},"is_error":false}"#,
            ];
            for event in events {
                writer.write_all(event.as_bytes()).await.unwrap();
                writer.write_all(b"\n").await.unwrap();
            }
        });

        let response = read_stream_response_from_lines(&mut buf_reader).await.unwrap();
        assert_eq!(response.content.len(), 2);
        match &response.content[0] {
            ContentBlock::Text { text } => assert_eq!(text, "Let me check."),
            _ => panic!("expected text block"),
        }
        match &response.content[1] {
            ContentBlock::ToolUse { id, name, input } => {
                assert_eq!(id, "tu-1");
                assert_eq!(name, "read_memory");
                assert_eq!(input["topic"], "tasks");
            }
            _ => panic!("expected tool_use block"),
        }
        assert_eq!(response.stop_reason, StopReason::ToolUse);
    }

    #[tokio::test]
    async fn read_stream_response_tool_use_only() {
        use tokio::io::AsyncWriteExt;

        let (reader, mut writer) = tokio::io::duplex(8192);
        let mut buf_reader = BufReader::new(reader);

        tokio::spawn(async move {
            let events = vec![
                r#"{"type":"assistant","subtype":"tool_use","id":"tu-2","name":"search_memory","input":{"query":"test"}}"#,
                r#"{"type":"result","subtype":"success","result":"","session_id":"s3","usage":{"input_tokens":15,"output_tokens":8},"is_error":false}"#,
            ];
            for event in events {
                writer.write_all(event.as_bytes()).await.unwrap();
                writer.write_all(b"\n").await.unwrap();
            }
        });

        let response = read_stream_response_from_lines(&mut buf_reader).await.unwrap();
        assert_eq!(response.content.len(), 1);
        match &response.content[0] {
            ContentBlock::ToolUse { name, .. } => assert_eq!(name, "search_memory"),
            _ => panic!("expected tool_use block"),
        }
        assert_eq!(response.stop_reason, StopReason::ToolUse);
    }

    #[tokio::test]
    async fn read_stream_response_error_result() {
        use tokio::io::AsyncWriteExt;

        let (reader, mut writer) = tokio::io::duplex(8192);
        let mut buf_reader = BufReader::new(reader);

        tokio::spawn(async move {
            let event = r#"{"type":"result","subtype":"error","result":"something went wrong","session_id":"","usage":{"input_tokens":0,"output_tokens":0},"is_error":true}"#;
            writer.write_all(event.as_bytes()).await.unwrap();
            writer.write_all(b"\n").await.unwrap();
        });

        let result = read_stream_response_from_lines(&mut buf_reader).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("something went wrong"));
    }

    #[tokio::test]
    async fn read_stream_response_auth_error() {
        use tokio::io::AsyncWriteExt;

        let (reader, mut writer) = tokio::io::duplex(8192);
        let mut buf_reader = BufReader::new(reader);

        tokio::spawn(async move {
            let event = r#"{"type":"result","subtype":"error","result":"Not logged in to Claude","session_id":"","usage":{"input_tokens":0,"output_tokens":0},"is_error":true}"#;
            writer.write_all(event.as_bytes()).await.unwrap();
            writer.write_all(b"\n").await.unwrap();
        });

        let result = read_stream_response_from_lines(&mut buf_reader).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Not logged in"));
    }

    #[tokio::test]
    async fn read_stream_response_result_with_text_content() {
        use tokio::io::AsyncWriteExt;

        let (reader, mut writer) = tokio::io::duplex(8192);
        let mut buf_reader = BufReader::new(reader);

        tokio::spawn(async move {
            // Some CLI versions put the response in the result field instead of assistant events
            let event = r#"{"type":"result","subtype":"success","result":"The answer is 42.","session_id":"s4","usage":{"input_tokens":30,"output_tokens":12},"is_error":false}"#;
            writer.write_all(event.as_bytes()).await.unwrap();
            writer.write_all(b"\n").await.unwrap();
        });

        let response = read_stream_response_from_lines(&mut buf_reader).await.unwrap();
        assert_eq!(response.content.len(), 1);
        match &response.content[0] {
            ContentBlock::Text { text } => assert_eq!(text, "The answer is 42."),
            _ => panic!("expected text block"),
        }
    }

    #[tokio::test]
    async fn read_stream_response_skips_bad_lines() {
        use tokio::io::AsyncWriteExt;

        let (reader, mut writer) = tokio::io::duplex(8192);
        let mut buf_reader = BufReader::new(reader);

        tokio::spawn(async move {
            let lines = vec![
                "not json at all",
                r#"{"type":"assistant","subtype":"text","text":"ok"}"#,
                "", // empty line
                r#"{"type":"result","subtype":"success","result":"","session_id":"s5","usage":{"input_tokens":1,"output_tokens":1},"is_error":false}"#,
            ];
            for line in lines {
                writer.write_all(line.as_bytes()).await.unwrap();
                writer.write_all(b"\n").await.unwrap();
            }
        });

        let response = read_stream_response_from_lines(&mut buf_reader).await.unwrap();
        assert_eq!(response.content.len(), 1);
        match &response.content[0] {
            ContentBlock::Text { text } => assert_eq!(text, "ok"),
            _ => panic!("expected text block"),
        }
    }

    // ── New tests: Backoff state ────────────────────────────────────

    #[test]
    fn backoff_initial_state() {
        let b = BackoffState::new();
        assert_eq!(b.consecutive_failures, 0);
        assert_eq!(b.delay(), Duration::from_secs(1));
        assert!(!b.should_fallback());
    }

    #[test]
    fn backoff_exponential_progression() {
        let mut b = BackoffState::new();

        b.record_failure(); // 1st failure
        assert_eq!(b.consecutive_failures, 1);
        assert_eq!(b.delay(), Duration::from_secs(2));
        assert!(!b.should_fallback());

        b.record_failure(); // 2nd
        assert_eq!(b.delay(), Duration::from_secs(4));

        b.record_failure(); // 3rd
        assert_eq!(b.delay(), Duration::from_secs(8));

        b.record_failure(); // 4th
        assert_eq!(b.delay(), Duration::from_secs(16));
        assert!(!b.should_fallback());

        b.record_failure(); // 5th — should trigger fallback
        assert!(b.should_fallback());
        assert_eq!(b.delay(), Duration::from_secs(30)); // capped at 30s
    }

    #[test]
    fn backoff_resets_on_success() {
        let mut b = BackoffState::new();
        b.record_failure();
        b.record_failure();
        b.record_failure();
        assert_eq!(b.consecutive_failures, 3);

        b.record_success();
        assert_eq!(b.consecutive_failures, 0);
        assert_eq!(b.delay(), Duration::from_secs(1));
        assert!(!b.should_fallback());
    }

    #[test]
    fn backoff_cap_at_30s() {
        let mut b = BackoffState::new();
        for _ in 0..10 {
            b.record_failure();
        }
        // Should be capped at 30s regardless of how many failures
        assert!(b.delay() <= Duration::from_secs(30));
    }

    // ── New tests: SpawnConfig ──────────────────────────────────────

    #[test]
    fn spawn_config_resolves_paths() {
        let config = SpawnConfig::new("claude-sonnet-4-20250514");
        assert_eq!(config.model, "claude-sonnet-4-20250514");
        assert!(config.sandbox_home.contains("claude-sandbox"));
        assert!(!config.real_home.is_empty());
    }

    // ── New tests: Stream input builder ─────────────────────────────

    #[test]
    fn build_stream_input_produces_prompt() {
        let system = "You are NV.";
        let messages = vec![Message::user("hello")];
        let tools = vec![];
        let input = build_stream_input(system, &messages, &tools);
        assert!(input.contains("You are NV."));
        assert!(input.contains("User: hello"));
    }

    // ── New tests: ClaudeClient constructor ─────────────────────────

    #[test]
    fn claude_client_new_and_clone() {
        let client = ClaudeClient::new(
            "ignored".into(),
            "claude-sonnet-4-20250514".into(),
            4096,
        );
        assert_eq!(client.model, "claude-sonnet-4-20250514");
        assert_eq!(client.max_tokens, 4096);

        // Clone should share the same session Arc
        let cloned = client.clone();
        assert_eq!(cloned.model, client.model);
        assert!(Arc::ptr_eq(&client.session, &cloned.session));
    }
}
