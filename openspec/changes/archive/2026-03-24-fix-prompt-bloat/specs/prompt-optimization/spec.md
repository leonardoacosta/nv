# Prompt Optimization

## MODIFIED Requirements

### Requirement: Req-1: Persistent subprocess turn payload

The persistent subprocess turn (`build_stream_input`) MUST send only the latest user message
content, not the full system prompt, tool definitions, or conversation history.

#### Scenario: Simple text message turn
Given a persistent subprocess is alive
And the user sends "ping" via Telegram
When `build_stream_input()` is called with system prompt, messages, and tools
Then the returned string contains only the user's message text "ping"
And the returned string does NOT contain the system prompt text
And the returned string does NOT contain tool definition JSON schemas
And the returned string length is less than 1000 bytes

### Requirement: Req-2: Cold-start prompt content

The cold-start fallback prompt MUST contain only the conversation content. The system prompt is
passed via the `--system-prompt` CLI flag. Tool definitions MUST NOT be embedded.

#### Scenario: Cold-start prompt excludes system prompt and tools
Given the persistent subprocess is unavailable
And the user sends "status" via Telegram
When `build_prompt()` is called for cold-start
Then the prompt contains the user message "status"
And the prompt does NOT contain the system prompt text
And the prompt does NOT contain "## Available Tools" header
And the prompt does NOT contain tool input_schema JSON

### Requirement: Req-3: Payload size logging

Both persistent and cold-start paths MUST log payload size at INFO level with fields:
`prompt_bytes`, `system_bytes`, `messages` count, and `tools` count.

#### Scenario: Payload size logged on persistent turn
Given a persistent subprocess is alive
When a turn is sent
Then an INFO log line is emitted containing `prompt_bytes` and `tools` fields
And `prompt_bytes` value is less than 1000 for a simple text message
