# Proposal: Add Agent SDK Integration

## Change ID
`add-agent-sdk-integration`

## Summary

Wire `@anthropic-ai/claude-agent-sdk` into the TypeScript daemon as Nova's brain. Every inbound message becomes a `query()` call routed through the Vercel AI Gateway, with conversation history loaded from and saved to Postgres.

## Context

- Extends: `packages/daemon/src/` (scaffolded by `scaffold-ts-daemon`)
- Depends on: `scaffold-ts-daemon` (must be applied first — provides `packages/daemon/`, `src/types.ts`, config, and logger)
- Related: archived `replace-anthropic-with-agent-sdk`, archived `migrate-nova-brain`
- System prompt: `config/system-prompt.md` (same file consumed by the Rust daemon)

## Motivation

The Rust daemon's agent loop (`crates/nv-daemon/src/agent.rs`, `worker.rs`) is functional but tightly coupled to the Rust type system, making it slow to iterate on AI behavior. The TypeScript daemon needs its own agent layer that:

1. Uses the `@anthropic-ai/claude-agent-sdk` `query()` API for a clean, SDK-native call path
2. Routes through the Vercel AI Gateway (no direct API key required — uses Claude MAX OAuth)
3. Loads conversation history from Postgres before each call and saves the exchange after
4. Exposes a clean `NovaAgent` class and `ConversationManager` for downstream wiring

## Requirements

### Req-1: NovaAgent class

Create `packages/daemon/src/brain/agent.ts` — the primary agent interface.

`NovaAgent` must:

- Accept a `Config` instance at construction (from `src/config.ts`)
- Load the system prompt from `config/system-prompt.md` at construction time (relative to `process.cwd()` or an absolute path from config); fall back to an empty string with a warning log if the file is missing
- Expose `processMessage(message: Message, history: Message[]): Promise<AgentResponse>`
- Inside `processMessage`, call `query()` from `@anthropic-ai/claude-agent-sdk` with:
  - `prompt`: `message.content`
  - `system_prompt`: the loaded system prompt
  - `allowed_tools`: `["Read", "Write", "Bash", "Glob", "Grep", "WebSearch", "WebFetch"]`
  - `permission_mode`: `"bypassPermissions"`
  - `max_turns`: `30`
  - `env`: `{ ANTHROPIC_BASE_URL: "https://ai-gateway.vercel.sh", ANTHROPIC_CUSTOM_HEADERS: "x-ai-gateway-api-key: Bearer {key}" }` where `{key}` is sourced from `config.vercelGatewayKey` or the `VERCEL_GATEWAY_KEY` env var
- Collect the `ResultMessage` stream, extract the final text response and any tool call records
- Return `AgentResponse`

`AgentResponse` type (defined in `src/brain/types.ts`):

```typescript
export interface AgentResponse {
  text: string;
  toolCalls: ToolCall[];
  stopReason: string;
}

export interface ToolCall {
  name: string;
  input: Record<string, unknown>;
  result: unknown;
}
```

#### Scenario: Successful message processing

Given a `Message` with `content: "What's on my calendar today?"` and an empty history,
when `processMessage` is called,
then it invokes `query()` with the correct parameters,
collects the `ResultMessage`, and returns an `AgentResponse` with `text` populated and `stopReason: "end_turn"`.

#### Scenario: Missing system prompt file

Given `config/system-prompt.md` does not exist,
when `NovaAgent` is constructed,
then it logs a warning and sets `systemPrompt` to an empty string without throwing.

#### Scenario: Missing gateway key

Given `VERCEL_GATEWAY_KEY` is not set and `config.vercelGatewayKey` is undefined,
when `processMessage` is called,
then it throws an `Error` with a descriptive message before invoking `query()`.

### Req-2: ConversationManager class

Create `packages/daemon/src/brain/conversation.ts` — Postgres-backed history management.

`ConversationManager` must:

- Accept a `pg.Pool` instance (from `node-postgres`) at construction
- Expose `loadHistory(channelId: string, limit: number): Promise<Message[]>` — queries the `messages` table ordered by `received_at DESC`, returns up to `limit` rows reversed to chronological order
- Expose `saveExchange(channelId: string, userMsg: Message, assistantMsg: Message): Promise<void>` — inserts both messages into the `messages` table in a single transaction

Database table assumed to exist (managed by `setup-postgres-drizzle`):

```sql
-- messages table (read/write only — schema owned by setup-postgres-drizzle)
messages (
  id          UUID PRIMARY KEY,
  channel_id  TEXT NOT NULL,
  role        TEXT NOT NULL,  -- 'user' | 'assistant'
  content     TEXT NOT NULL,
  received_at TIMESTAMPTZ NOT NULL DEFAULT now()
)
```

`Message` type from `src/types.ts` (defined in `scaffold-ts-daemon`) is used as-is. For assistant messages stored in Postgres, `senderId` and `senderName` are set to `"nova"`.

#### Scenario: Load history for a channel

Given a `channelId` with 5 existing rows in the `messages` table,
when `loadHistory(channelId, 3)` is called,
then it returns the 3 most recent messages in ascending chronological order.

#### Scenario: Save exchange

Given a user `Message` and an assistant `Message`,
when `saveExchange` is called,
then both rows are inserted to the `messages` table within the same transaction, and a subsequent `loadHistory` call returns them.

#### Scenario: Empty history

Given a `channelId` with no rows,
when `loadHistory(channelId, 10)` is called,
then it returns an empty array without error.

### Req-3: Dependency addition

Add `@anthropic-ai/claude-agent-sdk` and `pg` + `@types/pg` to `packages/daemon/package.json`.

- `@anthropic-ai/claude-agent-sdk`: latest available version
- `pg`: `^8.13` (node-postgres)
- `@types/pg`: dev dependency

### Req-4: Config extension

Extend `packages/daemon/src/config.ts` (from `scaffold-ts-daemon`) to include:

- `vercelGatewayKey?: string` — sourced from `VERCEL_GATEWAY_KEY` env var
- `databaseUrl: string` — sourced from `DATABASE_URL` env var (required; throw if missing)
- `systemPromptPath: string` — defaults to `"config/system-prompt.md"`, overridable via `NV_SYSTEM_PROMPT_PATH`

`Config` type updated accordingly.

## Scope

- **IN**: `packages/daemon/src/brain/agent.ts`, `packages/daemon/src/brain/conversation.ts`, `packages/daemon/src/brain/types.ts`, `packages/daemon/src/config.ts` (extended), `packages/daemon/package.json` (new deps)
- **OUT**: HTTP server wiring, channel adapters, tool dispatch, obligation executor, any Rust daemon changes, Postgres schema migrations (owned by `setup-postgres-drizzle`)

## Impact

| Area | Change |
|------|--------|
| `packages/daemon/src/brain/agent.ts` | New file — NovaAgent class |
| `packages/daemon/src/brain/conversation.ts` | New file — ConversationManager class |
| `packages/daemon/src/brain/types.ts` | New file — AgentResponse and ToolCall types |
| `packages/daemon/src/config.ts` | Extended — vercelGatewayKey, databaseUrl, systemPromptPath |
| `packages/daemon/package.json` | Extended — add claude-agent-sdk, pg, @types/pg |

## Risks

| Risk | Mitigation |
|------|-----------|
| `@anthropic-ai/claude-agent-sdk` API surface changes | Pin version in package.json; review CHANGELOG before upgrading |
| Vercel AI Gateway auth format changes | Isolate gateway env var injection in `NovaAgent`; easy to swap |
| `ResultMessage` type shape from SDK is unclear | Read SDK typedefs and adjust extraction logic accordingly; wrap in a helper function |
| Postgres connection pool not yet wired at startup | `ConversationManager` accepts a pool at construction — caller is responsible for providing it; no singleton |
| `scaffold-ts-daemon` not yet applied | This spec MUST NOT be applied until `scaffold-ts-daemon` is complete |
