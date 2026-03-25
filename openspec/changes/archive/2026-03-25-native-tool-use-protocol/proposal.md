# Proposal: Native Tool Use Protocol

## Change ID
`native-tool-use-protocol`

## Summary

Replace the current approach of embedding tool schemas as prose in the system prompt (~49KB) with
the Anthropic Messages API's native `tools` parameter. Tool definitions are sent as structured
`Tool` objects in every request. Claude returns `tool_use` content blocks that the daemon
dispatches natively — no text parsing, no ```tool_call``` fence block heuristics.

## Context
- Modifies: `crates/nv-daemon/src/claude.rs` — `ClaudeClient`, `send_messages_cold_start_with_image`, `build_stream_input`, `parse_tool_calls`
- Modifies: `crates/nv-core/src/tool.rs` — `ToolDefinition` serialization format
- Related: `crates/nv-tools/src/registry.rs`, `crates/nv-tools/src/dispatch.rs` (unchanged — dispatch contract stays the same)
- Depends on: `add-anthropic-api-client` — this spec assumes a direct HTTP `AnthropicClient` (reqwest) is available alongside the CLI path; all refactors target both transports
- Depended on by: `persistent-conversation-state`

## Motivation

The current cold-start path embeds all tool schemas into the system prompt as formatted prose
plus a custom ```tool_call``` JSON fence block convention. This creates three compounding
problems:

**1. Prompt bloat.** The `augmented_system` string in `send_messages_cold_start_with_image`
appends tool names, descriptions, and full JSON schemas as human-readable text. With 100+ tools
in `stateless_tool_definitions()` plus daemon-internal tools, this runs to approximately 49KB.
Every cold-start turn pays this cost — even single-line replies like "ok" ship 49KB of tool
schema to the CLI subprocess.

**2. Brittle parse loop.** `parse_tool_calls` scans for ```tool_call` fence markers in free-text
output. This is fragile: Claude can omit the fence when it reasons about tools, wrap the JSON
differently under long-context pressure, or split a tool call across multiple blocks. Every
failure mode produces silent data loss — a tool call that looks like text and never executes.

**3. Tool use is off-spec.** The Anthropic API natively supports multi-tool batching per turn
(`stop_reason: "tool_use"` with multiple `ContentBlock::ToolUse` entries). The fence-block
convention forces serial, one-tool-per-turn execution and cannot express batched tool calls.
When Claude decides to call three tools in one response, only the first fence block is dispatched.

The fix: pass `tools` as the API-level parameter on both the direct HTTP path (post-`add-anthropic-api-client`)
and the CLI cold-start path. The CLI already supports `--tools-json` (a JSON array of Anthropic
tool objects piped to the subprocess). Responses arrive as structured `ContentBlock::ToolUse`
blocks — no text parsing needed. The system prompt drops from ~49KB to ~5KB (identity, soul,
operator rules — no schema prose).

## Design

### ToolDefinition Serialization

`nv_core::ToolDefinition` already has the right fields (`name`, `description`, `input_schema`).
The only change is its serialization format: the Anthropic API expects the key `input_schema`
(which already matches). No struct change needed.

```rust
// nv-core/src/tool.rs — unchanged shape, confirmed wire format
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,   // { "type": "object", "properties": {...} }
}
```

The `tool_to_json` helper in `nv-tools/src/registry.rs` emits `inputSchema` (camelCase) for the
MCP `tools/list` response. A separate serialization path for the Anthropic API emits
`input_schema` (snake_case). These are two distinct output contexts — do not unify them.

### Cold-Start Path: --tools-json Flag

The Claude CLI accepts `--tools-json '<json-array>'` — a JSON array of Anthropic tool objects
passed at subprocess spawn time. This replaces the inline prose in `augmented_system`.

```rust
// In send_messages_cold_start_with_image:
// BEFORE — appends tool schemas to augmented_system string (49KB)
let augmented_system = if tools.is_empty() {
    system.to_string()
} else {
    let mut s = system.to_string();
    s.push_str("\n\n## Available Tools\n\n ...");  // <-- 49KB prose
    s
};

// AFTER — system prompt is unchanged; tools serialized to CLI flag
let tools_json = if tools.is_empty() {
    None
} else {
    let tool_objects: Vec<serde_json::Value> = tools
        .iter()
        .map(|t| serde_json::json!({
            "name": t.name,
            "description": t.description,
            "input_schema": t.input_schema,
        }))
        .collect();
    Some(serde_json::to_string(&tool_objects).expect("tools serialization"))
};

// Base args: drop --tools "Read,Glob,..." and replace with --tools-json
if let Some(ref json) = tools_json {
    base_args.push("--tools-json".into());
    base_args.push(json.clone());
}
```

The existing `--tools "Read,Glob,Grep,Bash(*)"` flag that grants the CLI its own filesystem
tools is removed — Nova's tool dispatch is handled by the daemon, not the CC CLI's built-in
tools. The CLI subprocess becomes a pure language model endpoint.

### Response Parsing: Native tool_use Blocks

With `--tools-json`, the CLI's `--output-format json` response changes shape. Tool invocations
arrive as structured entries in the `content` array rather than as fence-block text. The
`parse_tool_calls` function is removed. The existing `ContentBlock::ToolUse` deserialization
(already present in the `#[serde(tag = "type")]` enum) handles this natively.

```rust
// parse_tool_calls() — DELETED (250 lines of fence-block parsing)

// ContentBlock deserialization — UNCHANGED, already handles tool_use:
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse { id: String, name: String, input: serde_json::Value },
    #[serde(rename = "tool_result")]
    ToolResult { tool_use_id: String, content: String, is_error: bool },
}
```

The cold-start response parser (`send_messages_cold_start_with_image`) currently calls
`parse_tool_calls(&cli_response.result)` on the string result field. After this change, the
CLI response `content` array is parsed directly.

### Direct HTTP Path (post-add-anthropic-api-client)

When the `AnthropicClient` is available, tools pass through the `tools` field in the request
body per the Messages API spec:

```rust
let body = serde_json::json!({
    "model": self.model,
    "max_tokens": self.max_tokens,
    "system": system,
    "messages": messages,
    "tools": tools,   // Vec<ToolDefinition> serialized directly
});
```

No prompt augmentation. Response content blocks are deserialized directly into `Vec<ContentBlock>`.

### Persistent Session Path

The persistent subprocess path (`send_turn` / `build_stream_input`) currently ignores the `tools`
parameter (already a known limitation — tools were embedded in the system prompt at spawn time
via the cold-start's augmented system). After this change:

- The persistent subprocess is spawned with `--tools-json <all-tools>` at spawn time, once.
- Per-turn messages do not repeat tool schemas.
- If tool definitions change at runtime (rare), the session must be restarted to pick up the new set.

### Tool Result Wire Format

Tool results returned to Claude follow the existing `Message::tool_results()` path unchanged.
The `tool_use_id` linkage from `ContentBlock::ToolUse { id }` to `ContentBlock::ToolResult { tool_use_id }` is already implemented correctly in the agent loop.

### Multi-Tool Batching

A single Claude turn may now return multiple `ContentBlock::ToolUse` entries in one response.
The agent loop's `run_tool_loop` already collects all tool_use blocks per turn and dispatches
them. No agent-loop changes are required — the loop already handles batched tool calls correctly.
Previously, fence-block parsing could only surface one tool call per response. This is now lifted.

### System Prompt Size Impact

| Component | Before | After |
|-----------|--------|-------|
| Identity + soul + operator rules | ~5KB | ~5KB |
| Tool schema prose | ~44KB | 0 |
| Total system prompt | ~49KB | ~5KB |
| Tools (API parameter) | 0 | ~44KB |

Total bytes sent per turn is unchanged — the schemas move from the system prompt string to the
structured `tools` array. The gain is correctness and reliability, not bandwidth.

### Scope
- **IN**: Remove `parse_tool_calls`, remove tool schema augmentation in `send_messages_cold_start_with_image`, add `--tools-json` CLI flag, wire `tools` array in HTTP path, update persistent spawn to pass tools at spawn time
- **OUT**: Changes to tool definitions themselves, changes to `ToolRegistry` or `dispatch.rs`, changes to the agent loop's `execute_tool` dispatch, changes to `nv-tools` MCP server protocol

## Impact

| File | Change |
|------|--------|
| `crates/nv-daemon/src/claude.rs` | Remove `parse_tool_calls`, remove tool prose augmentation, add `--tools-json` CLI flag, update cold-start response parsing to use content array |
| `crates/nv-daemon/src/claude.rs` | Update `build_stream_input` to pass `--tools-json` to persistent subprocess spawn |
| `crates/nv-core/src/tool.rs` | Add `anthropic_json()` helper method that emits `{ name, description, input_schema }` |

## Risks

| Risk | Mitigation |
|------|-----------|
| `--tools-json` flag not available in the installed CC CLI version | Detect via `claude --help` at startup; fall back to augmented-system path if flag absent, log deprecation warning |
| CLI response shape changes when `--tools-json` is active | Read live CLI output against a canary tool call in integration test before removing parse_tool_calls entirely |
| Tool definitions with malformed `input_schema` silently rejected by API | Add `validate_tool_definitions()` at startup — verify each schema is a valid JSON object with `type: object` |
| Persistent session out-of-sync if tools change at runtime | Log a warning when `send_turn` is called with a tools list that differs from the spawn-time list; restart session |
| Removing `--tools "Read,Glob,Grep,Bash(*)"` breaks daemon-internal tool resolution | Confirm daemon tool dispatch is the sole resolver — the CC built-in tools were unused by the daemon loop; verify no path relied on them |
