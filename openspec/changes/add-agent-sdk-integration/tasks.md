# Implementation Tasks

<!-- beads:epic:nv-v0ec -->

## Foundation Batch

- [ ] [1.1] Add `@anthropic-ai/claude-agent-sdk`, `pg` (^8.13) to `packages/daemon/package.json` dependencies; add `@types/pg` to devDependencies [owner:api-engineer] [beads:nv-zhcv]
- [ ] [1.2] Extend `packages/daemon/src/config.ts` — add `vercelGatewayKey?: string` (from `VERCEL_GATEWAY_KEY`), `databaseUrl: string` (from `DATABASE_URL`, required), `systemPromptPath: string` (from `NV_SYSTEM_PROMPT_PATH`, defaults to `"config/system-prompt.md"`) [owner:api-engineer] [beads:nv-t7d1]

## Brain Batch

- [ ] [2.1] Create `packages/daemon/src/brain/types.ts` — export `AgentResponse { text: string; toolCalls: ToolCall[]; stopReason: string }` and `ToolCall { name: string; input: Record<string, unknown>; result: unknown }` [owner:api-engineer] [beads:nv-dyxe]
- [ ] [2.2] Create `packages/daemon/src/brain/agent.ts` — `NovaAgent` class: constructor reads `systemPromptPath` from config, loads file via `fs/promises` (warn + fallback to `""` if missing); `processMessage(message, history)` calls `query()` with `allowed_tools`, `permission_mode: "bypassPermissions"`, `max_turns: 30`, `env` block for Vercel AI Gateway; collects `ResultMessage` and returns `AgentResponse`; throws if `VERCEL_GATEWAY_KEY` absent [owner:api-engineer] [beads:nv-ct0l]
- [ ] [2.3] Create `packages/daemon/src/brain/conversation.ts` — `ConversationManager` class: constructor accepts `pg.Pool`; `loadHistory(channelId, limit)` queries `messages` table ordered by `received_at DESC LIMIT $2`, reverses result; `saveExchange(channelId, userMsg, assistantMsg)` inserts both rows in a single transaction, normalizing assistant `senderId`/`senderName` to `"nova"` [owner:api-engineer] [beads:nv-rzoo]

## Validation Batch

- [ ] [3.1] Run `npm run typecheck` in `packages/daemon/` — zero TypeScript errors [owner:api-engineer] [beads:nv-exkh]
