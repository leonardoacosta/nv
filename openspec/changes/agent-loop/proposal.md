# agent-loop

## Summary

Event-driven agent loop that receives triggers from all channel listeners via `mpsc::channel<Trigger>`, builds Claude API requests with system prompt + memory context + tool definitions, executes a tool-use loop against the Anthropic Messages API, and routes responses back to the appropriate channels.

## Motivation

The agent loop is NV's brain. Without it, channel listeners have nowhere to send messages and no intelligence processes them. This spec connects the Telegram listener (spec-3) to Claude's reasoning engine, establishing the core request/response cycle that every subsequent feature (memory, Jira, digest, Nexus) plugs into.

## Design

### mpsc Channel Architecture

All trigger sources feed a single `mpsc::unbounded_channel<Trigger>()`. The agent loop owns the `Receiver`; every listener and scheduler holds a cloned `Sender`.

```rust
// In daemon startup (main.rs)
let (trigger_tx, trigger_rx) = tokio::sync::mpsc::unbounded_channel::<Trigger>();

// Each listener gets a clone
let telegram_tx = trigger_tx.clone();
let cron_tx = trigger_tx.clone();
// Future: nexus_tx, discord_tx, etc.

// Agent loop owns the receiver
let agent = AgentLoop::new(config, trigger_rx, channels);
agent.run().await;
```

Unbounded channel is chosen over bounded because:
- Trigger producers (listeners) must never block — they are real-time I/O tasks
- The agent loop is the sole consumer and drains the queue on each wake
- Backpressure is handled at the Claude API level (rate limits), not the queue level
- In practice, trigger volume is low (human-speed messaging)

### Batch Drain Logic

When the agent loop wakes on `recv()`, it collects all queued triggers before making a single Claude call. This batches messages that arrived while the agent was processing a previous request.

```rust
impl AgentLoop {
    async fn drain_triggers(&mut self) -> Vec<Trigger> {
        // Block until at least one trigger arrives
        let first = self.trigger_rx.recv().await;
        let Some(first) = first else {
            return vec![]; // Channel closed, shutdown
        };

        let mut batch = vec![first];

        // Non-blocking drain of any additional queued triggers
        loop {
            match self.trigger_rx.try_recv() {
                Ok(trigger) => batch.push(trigger),
                Err(_) => break,
            }
        }

        tracing::info!(count = batch.len(), "drained trigger batch");
        batch
    }
}
```

The batch is formatted into a single user message that presents all triggers to Claude, allowing it to reason about priority and relationships between concurrent events.

### AgentLoop Struct

```rust
pub struct AgentLoop {
    config: AgentConfig,
    client: ClaudeClient,
    trigger_rx: mpsc::UnboundedReceiver<Trigger>,
    channels: ChannelRegistry,
    conversation_history: Vec<Message>,
    system_prompt: String,
    tool_definitions: Vec<ToolDefinition>,
}
```

`ChannelRegistry` is a `HashMap<String, Arc<dyn Channel>>` that maps channel names to their implementations, used for routing outbound messages.

### Claude API Client

The `ClaudeClient` wraps reqwest and handles Anthropic Messages API communication.

```rust
pub struct ClaudeClient {
    http: reqwest::Client,
    api_key: String,
    model: String,
    max_tokens: u32,
}

impl ClaudeClient {
    pub async fn send_messages(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<ApiResponse> {
        let body = json!({
            "model": self.model,
            "max_tokens": self.max_tokens,
            "system": system,
            "messages": messages,
            "tools": tools,
        });

        let response = self.http
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        // Handle HTTP-level errors
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ApiError::HttpError { status, body }.into());
        }

        response.json::<ApiResponse>().await
            .map_err(Into::into)
    }
}
```

**API types** (serde Deserialize):

```rust
#[derive(Deserialize)]
pub struct ApiResponse {
    pub id: String,
    pub content: Vec<ContentBlock>,
    pub stop_reason: StopReason,
    pub usage: Usage,
}

#[derive(Deserialize)]
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
}

#[derive(Deserialize)]
pub enum StopReason {
    #[serde(rename = "end_turn")]
    EndTurn,
    #[serde(rename = "tool_use")]
    ToolUse,
    #[serde(rename = "max_tokens")]
    MaxTokens,
}

#[derive(Deserialize)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}
```

### Messages Format / Request Building

Each agent loop iteration constructs a messages array following the Anthropic API format:

```
[system prompt]        ← defined once, loaded at startup
messages[]:
  [0] user:  memory context summary (injected)
  [1] user:  trigger batch (formatted)
  ...        conversation history (previous turns in session)
  [N] user:  current trigger batch
```

**Trigger batch formatting** — multiple triggers become a single structured user message:

```rust
fn format_trigger_batch(triggers: &[Trigger]) -> String {
    let mut parts = Vec::new();
    for trigger in triggers {
        match trigger {
            Trigger::Message(msg) => {
                parts.push(format!(
                    "[{}] {} from @{}: {}",
                    msg.channel, msg.timestamp.format("%H:%M"), msg.sender, msg.content
                ));
            }
            Trigger::Cron(event) => {
                parts.push(format!("[cron] {event:?} triggered"));
            }
            Trigger::NexusEvent(event) => {
                parts.push(format!("[nexus] {event:?}"));
            }
            Trigger::CliCommand(req) => {
                parts.push(format!("[cli] {}", req.command));
            }
        }
    }
    parts.join("\n")
}
```

### System Prompt

The system prompt is loaded from `~/.nv/system-prompt.md` (fallback to compiled-in default). It defines:

```markdown
You are NV, a task-focused agent harness for Leo. You are NOT a chatbot — you are an
operations assistant that monitors systems, manages Jira, and provides cross-system context.

## Identity
- Name: NV
- Operator: Leo (solo user, power user)
- Primary channel: Telegram

## Autonomy Rules
- READ operations: Execute immediately (memory, Jira search, Nexus query)
- WRITE operations: ALWAYS draft first, present for confirmation via pending action
- Never create/modify/transition Jira issues without explicit confirmation
- Memory writes (storing context) are autonomous — no confirmation needed

## Available Tools
You have access to these tools. Use them proactively when relevant:
- read_memory(topic): Read a specific memory file
- search_memory(query): Search across all memory files
- write_memory(topic, content): Store information for future reference
- query_jira(jql): Search Jira issues
- query_nexus(): Get running session status

## Response Format
Respond conversationally but concisely. When you need to take an action:
1. Describe what you want to do
2. If it's a write operation, say "I'll draft this for your confirmation"
3. Use the appropriate tool

## Context
You receive triggers from multiple sources (Telegram messages, cron events, Nexus events,
CLI commands). Process them in priority order. Multiple triggers may arrive at once — batch
your reasoning.
```

### Tool Definitions

Tools are defined as JSON schema objects per the Anthropic tool use specification:

```rust
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}
```

Initial tool set (spec 4 — stubs, implementations come from spec 5 and 6):

| Tool | Parameters | Description |
|------|-----------|-------------|
| `read_memory` | `topic: string` | Read a specific memory topic file |
| `search_memory` | `query: string` | Grep across all memory files |
| `write_memory` | `topic: string, content: string` | Append content to a memory topic |
| `query_jira` | `jql: string` | Search Jira issues with JQL |
| `query_nexus` | `(none)` | Get running Nexus sessions |

Tool definitions are registered at startup. Specs 5 and 6 add their implementations; spec 4 provides stub implementations that return placeholder responses so the loop is testable end-to-end.

### Tool Use Loop

When Claude returns `stop_reason: "tool_use"`, the agent loop must execute the requested tool(s) and return results before Claude can continue.

```rust
async fn run_tool_loop(
    &mut self,
    initial_response: ApiResponse,
) -> Result<Vec<ContentBlock>> {
    let mut response = initial_response;
    let mut all_content = Vec::new();

    loop {
        // Collect text blocks from this response
        let mut tool_uses = Vec::new();
        for block in &response.content {
            match block {
                ContentBlock::Text { .. } => all_content.push(block.clone()),
                ContentBlock::ToolUse { id, name, input } => {
                    tool_uses.push((id.clone(), name.clone(), input.clone()));
                }
            }
        }

        // If no tool uses, we're done
        if tool_uses.is_empty() || response.stop_reason != StopReason::ToolUse {
            all_content.extend(response.content);
            break;
        }

        // Add assistant response to conversation history
        self.conversation_history.push(Message {
            role: "assistant".into(),
            content: response.content.clone(),
        });

        // Execute each tool and collect results
        let mut tool_results = Vec::new();
        for (id, name, input) in &tool_uses {
            let result = self.execute_tool(name, input).await;
            tool_results.push(ToolResult {
                tool_use_id: id.clone(),
                content: match result {
                    Ok(output) => output,
                    Err(e) => format!("Error: {e}"),
                },
                is_error: result.is_err(),
            });
        }

        // Send tool results back to Claude
        self.conversation_history.push(Message {
            role: "user".into(),
            content: tool_results_to_content(tool_results),
        });

        // Continue the conversation
        response = self.client.send_messages(
            &self.system_prompt,
            &self.conversation_history,
            &self.tool_definitions,
        ).await?;
    }

    Ok(all_content)
}
```

**Tool execution dispatch:**

```rust
async fn execute_tool(
    &self,
    name: &str,
    input: &serde_json::Value,
) -> Result<String> {
    match name {
        "read_memory" => {
            let topic = input["topic"].as_str()
                .ok_or_else(|| anyhow!("missing topic parameter"))?;
            self.memory.read(topic).await
        }
        "search_memory" => {
            let query = input["query"].as_str()
                .ok_or_else(|| anyhow!("missing query parameter"))?;
            self.memory.search(query).await
        }
        "write_memory" => {
            let topic = input["topic"].as_str()
                .ok_or_else(|| anyhow!("missing topic parameter"))?;
            let content = input["content"].as_str()
                .ok_or_else(|| anyhow!("missing content parameter"))?;
            self.memory.write(topic, content).await
        }
        "query_jira" => {
            let jql = input["jql"].as_str()
                .ok_or_else(|| anyhow!("missing jql parameter"))?;
            self.jira.search(jql).await
        }
        "query_nexus" => {
            self.nexus.get_sessions().await
        }
        _ => Err(anyhow!("unknown tool: {name}")),
    }
}
```

### Response Routing

After the tool loop completes, the final text content is parsed and routed:

```rust
async fn route_response(
    &self,
    content: &[ContentBlock],
    source_triggers: &[Trigger],
) -> Result<()> {
    let text = content.iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");

    if text.is_empty() {
        return Ok(());
    }

    // Determine the reply channel from the source trigger
    // Default to Telegram for cron/nexus/cli triggers
    let reply_channel = source_triggers.first()
        .and_then(|t| match t {
            Trigger::Message(msg) => Some(msg.channel.as_str()),
            _ => None,
        })
        .unwrap_or("telegram");

    // Send the response via the appropriate channel
    if let Some(channel) = self.channels.get(reply_channel) {
        let reply_to = source_triggers.first().and_then(|t| match t {
            Trigger::Message(msg) => Some(msg.id.clone()),
            _ => None,
        });

        channel.send_message(OutboundMessage {
            channel: reply_channel.to_string(),
            content: text,
            reply_to,
            keyboard: None,
        }).await?;
    }

    Ok(())
}
```

**PendingAction routing** — when Claude drafts a write action (Jira create, transition, etc.), the response includes a structured action block. The agent loop detects this, writes to `pending-actions.json`, and sends a Telegram confirmation keyboard:

```rust
// Detected from Claude's response structure or a dedicated tool call
if let Some(action) = parse_pending_action(&text) {
    let action_id = uuid::Uuid::new_v4().to_string();
    let pending = PendingAction {
        id: action_id.clone(),
        description: action.description,
        jira_payload: action.payload,
        status: PendingStatus::AwaitingConfirmation,
    };

    // Persist to state
    self.state.save_pending_action(&pending).await?;

    // Send confirmation keyboard to Telegram
    let keyboard = InlineKeyboard::new(vec![
        vec![
            InlineButton::new("Confirm", format!("action:confirm:{action_id}")),
            InlineButton::new("Edit", format!("action:edit:{action_id}")),
            InlineButton::new("Cancel", format!("action:cancel:{action_id}")),
        ],
    ]);

    self.channels.get("telegram").unwrap()
        .send_message(OutboundMessage {
            channel: "telegram".into(),
            content: format!("Draft action:\n{}\n\nConfirm?", pending.description),
            reply_to: None,
            keyboard: Some(keyboard),
        }).await?;
}
```

### Context Window Management

The conversation history is bounded to prevent exceeding Claude's context window.

**Strategy:**
1. System prompt: fixed (~800 tokens, always included)
2. Memory context: injected per-call, budget of ~2000 tokens
3. Conversation history: sliding window of last N turns, newest kept
4. Trigger batch: current triggers (variable, typically small)

**Truncation implementation:**

```rust
const MAX_HISTORY_TURNS: usize = 20;
const MAX_HISTORY_CHARS: usize = 50_000; // ~12k tokens rough estimate

fn truncate_history(history: &mut Vec<Message>) {
    // Keep at most N turns
    if history.len() > MAX_HISTORY_TURNS {
        let drain_count = history.len() - MAX_HISTORY_TURNS;
        history.drain(..drain_count);
    }

    // If still too large, drop oldest turns until under budget
    let mut total_chars: usize = history.iter()
        .map(|m| m.content_len())
        .sum();

    while total_chars > MAX_HISTORY_CHARS && history.len() > 2 {
        if let Some(removed) = history.first() {
            total_chars -= removed.content_len();
        }
        history.remove(0);
    }
}
```

The conversation history resets on each "session" — a session is a burst of activity. After 10 minutes of inactivity (no triggers), the history is cleared and a fresh context is built on the next trigger. This prevents stale context from accumulating.

### Error Handling

| Error | Handling |
|-------|----------|
| HTTP 429 (rate limit) | Parse `retry-after` header, sleep, retry once. If still 429, log and skip this cycle. Notify on Telegram if repeated. |
| HTTP 5xx (API error) | Retry up to 3 times with exponential backoff (1s, 2s, 4s). On exhaustion, notify on Telegram. |
| HTTP 401 (auth) | Log error, notify on Telegram, do not retry (config issue). |
| Network error | Retry 3 times with backoff. On exhaustion, log and wait for next trigger. |
| Malformed response | Log full response body at `error` level. Return generic error to user. |
| Tool execution error | Return error string as `tool_result` with `is_error: true`. Claude handles gracefully. |
| Channel closed (mpsc) | Indicates shutdown — exit agent loop cleanly. |
| Max tokens reached | `stop_reason: "max_tokens"` — log warning. Response is partial but usable. |

```rust
#[derive(thiserror::Error, Debug)]
pub enum ApiError {
    #[error("HTTP {status}: {body}")]
    HttpError {
        status: reqwest::StatusCode,
        body: String,
    },
    #[error("Rate limited, retry after {retry_after_secs}s")]
    RateLimited { retry_after_secs: u64 },
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("Deserialization error: {0}")]
    Deserialize(String),
}
```

### Main Loop

```rust
impl AgentLoop {
    pub async fn run(mut self) -> Result<()> {
        tracing::info!("agent loop started, waiting for triggers");

        loop {
            let triggers = self.drain_triggers().await;
            if triggers.is_empty() {
                tracing::info!("trigger channel closed, shutting down");
                break;
            }

            // Check session timeout — clear history if stale
            self.maybe_reset_session();

            // Build messages array
            let memory_context = self.memory.load_relevant_context(&triggers).await?;
            let trigger_text = format_trigger_batch(&triggers);

            self.conversation_history.push(Message::user(format!(
                "{memory_context}\n\n---\n\n{trigger_text}"
            )));

            // Truncate if needed
            truncate_history(&mut self.conversation_history);

            // Call Claude
            match self.client.send_messages(
                &self.system_prompt,
                &self.conversation_history,
                &self.tool_definitions,
            ).await {
                Ok(response) => {
                    // Run tool loop if needed
                    let final_content = self.run_tool_loop(response).await?;

                    // Route response to appropriate channel
                    self.route_response(&final_content, &triggers).await?;

                    self.last_activity = Instant::now();
                }
                Err(e) => {
                    tracing::error!(error = %e, "claude API call failed");
                    self.handle_api_error(e, &triggers).await;
                }
            }
        }

        Ok(())
    }
}
```

## Verification

- Message NV on Telegram with "hello" -- Claude processes and returns a meaningful response
- Send multiple messages rapidly -- they are batched into a single Claude call
- Agent responds with tool use stubs when relevant (e.g., "check my tasks" triggers `query_jira` stub)
- API errors (simulated via invalid key) produce Telegram notification, not a crash
- Agent loop survives and continues after transient errors
