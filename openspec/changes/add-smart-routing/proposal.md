# Proposal: Add Smart Routing

## Change ID
`add-smart-routing`

## Summary
Add a 4-tier message routing cascade to the daemon that classifies incoming Telegram messages and routes simple ones directly to fleet tools via HTTP, bypassing the Agent SDK entirely. Tier 0 (existing commands) and Tier 3 (full Agent SDK) are unchanged; this spec adds Tier 1 (regex/keyword matching) and Tier 2 (embedding similarity via local model). Estimated 80% of messages skip the Agent SDK, reducing response time from 5-120s to ~2s and cost from $0.10-2.00 to $0.00 per routed message.

## Context
- Depends on: `slim-daemon` (completed), `add-tool-router` (completed -- fleet services on ports 4100-4109)
- Conflicts with: none (new module, single integration point in index.ts)
- Roadmap: Nova v11, Wave 1
- Current state: Every non-callback message in `index.ts` goes through `agent.processMessage()`, which calls the Claude Agent SDK via Vercel AI Gateway. Simple queries like "what's on my calendar?" invoke the full agent loop (system prompt, MCP tool discovery, multi-turn reasoning) even when the answer is a single fleet tool call.

## Motivation
The Agent SDK path costs $0.10-2.00 per message and takes 5-120 seconds. Most user messages (~80%) are simple single-tool queries: "what's on my calendar?", "check my email", "what's the weather?". These can be answered by calling one fleet tool via HTTP and formatting the result -- no LLM reasoning needed. The smart router intercepts these messages before the agent loop, calls the fleet tool directly, and returns the formatted response in ~2 seconds at zero API cost.

The safe default is always Tier 3 (agent). The router only bypasses the agent when confidence is high. False negatives (routing to agent when the router could have handled it) cost money but give correct answers. False positives (routing to a tool when the agent was needed) give wrong answers. The threshold is tuned to strongly prefer false negatives.

## Requirements

### Req-1: Router cascade module
Create `packages/daemon/src/brain/router.ts` exporting a `MessageRouter` class with a single `route(text: string): Promise<RouteResult>` method. The cascade evaluates tiers 0-3 in order, returning on the first match:

```typescript
type RouteTier = 0 | 1 | 2 | 3;

interface RouteResult {
  tier: RouteTier;
  tool?: string;       // fleet tool name (tiers 1-2)
  params?: Record<string, unknown>;  // tool parameters
  confidence: number;  // 0.0 - 1.0
}
```

- Tier 0: Returns `{ tier: 0, confidence: 1.0 }` if the message starts with `/` (existing command handler in index.ts already catches these before the router, so this is a no-op guard)
- Tier 1: Delegates to keyword router
- Tier 2: Delegates to embedding router
- Tier 3: Default fallback `{ tier: 3, confidence: 0.0 }` -- route to Agent SDK

### Req-2: Keyword router (Tier 1)
Create `packages/daemon/src/brain/keyword-router.ts` exporting a `KeywordRouter` class. Contains a static pattern table of 20+ regex-to-tool mappings:

| Pattern Category | Example Utterances | Fleet Tool | Port |
|------------------|--------------------|------------|------|
| Calendar today | "what's on my calendar", "today's schedule", "my agenda" | `calendar_today` | 4106 |
| Calendar upcoming | "upcoming events", "what's next", "next meeting" | `calendar_upcoming` | 4106 |
| Email inbox | "check my email", "any new emails", "inbox" | `email_inbox` | 4103 |
| Email send | "send an email to", "email [name] about" | `email_send` | 4103 |
| Weather | "weather", "forecast", "temperature" | `weather_current` | 4104 |
| Reminders list | "my reminders", "what do I need to do", "pending tasks" | `reminders_list` | 4106 |
| Reminder create | "remind me to", "set a reminder" | `reminder_create` | 4106 |
| Memory read | "what do you know about", "recall" | `memory_read` | 4101 |
| Messages recent | "recent messages", "latest messages" | `messages_recent` | 4102 |
| Contacts lookup | "who is", "contact info for" | `contact_lookup` | 4105 |
| System health | "system status", "health check", "are services running" | `health_check` | 4100 |
| Time/date | "what time is it", "current date" | `datetime_now` | 4108 |

Each pattern entry: `{ regex: RegExp, tool: string, port: number, extractParams?: (match: RegExpMatchArray, text: string) => Record<string, unknown> }`.

The `match(text: string): KeywordMatch | null` method lowercases the input, tests each regex, and returns the first match with `confidence: 0.95` (high confidence because regex matches are deterministic). Returns `null` if no pattern matches.

### Req-3: Embedding router (Tier 2)
Create `packages/daemon/src/brain/embedding-router.ts` exporting an `EmbeddingRouter` class. Uses `@xenova/transformers` to load `Xenova/all-MiniLM-L6-v2` (80MB, quantized) for local inference in Node.js -- no Python, no external API.

**Startup:**
- Load model once on daemon startup (async init)
- Load pre-computed intent centroids from JSON files in `packages/daemon/src/brain/intents/`
- Each intent JSON file contains: `{ tool: string, port: number, utterances: string[], centroid?: number[] }`
- On first startup (no centroid), compute centroid by encoding all utterances and averaging the embeddings
- Cache computed centroids back to the JSON files so subsequent starts skip encoding

**Runtime:**
- Encode the incoming message text (single inference, ~50ms)
- Compute cosine similarity against each intent centroid
- If max similarity >= 0.82, return `{ tool, port, confidence: similarity }`
- If max similarity < 0.82, return `null` (fall through to Tier 3)

The threshold 0.82 is deliberately conservative. Can be tuned via `NV_EMBEDDING_THRESHOLD` env var.

### Req-4: Intent seed data
Create `packages/daemon/src/brain/intents/` directory with one JSON file per intent. Each file contains 5-10 seed utterances covering natural language variations:

Start with intents matching the Tier 1 keyword categories, providing a second layer of catch for paraphrases that regex misses. Example `calendar-today.json`:

```json
{
  "tool": "calendar_today",
  "port": 4106,
  "utterances": [
    "what's on my calendar today",
    "show me today's events",
    "do I have any meetings today",
    "what's my schedule for today",
    "today's agenda please",
    "am I free today",
    "what meetings do I have"
  ]
}
```

### Req-5: Direct fleet tool execution
When Tier 1 or 2 matches, the router calls the fleet tool directly via `fleetPost(port, "/execute", { tool, params })` using the existing `fleet-client.ts`. The tool-router at :4100 routes `/execute` to the appropriate tool service. Format the JSON response into a human-readable Telegram message (Markdown).

Create a response formatter in `packages/daemon/src/brain/router.ts` (or a separate `format-response.ts` if it grows large) that converts fleet tool JSON responses into readable text. Keep it simple: if the response has a `text` field, use it. If it is structured data (array of events, list of emails), format as a bulleted list. Fall back to `JSON.stringify(result, null, 2)` wrapped in a code block.

### Req-6: Integration into message loop
Modify `packages/daemon/src/index.ts` to route messages through the `MessageRouter` before calling `agent.processMessage()`:

1. After existing callback routing (watcher, obligations) and before the agent loop
2. Call `router.route(msg.text ?? msg.content)`
3. If result tier is 1 or 2: execute the fleet tool directly, format the response, send via Telegram, log to diary, and return (skip agent)
4. If result tier is 0 or 3: fall through to existing agent loop

The typing indicator logic should still apply for routed messages (though they resolve in ~2s, the indicator signals Nova is working).

### Req-7: Diary logging for routed messages
Modify `packages/daemon/src/features/diary/index.ts` (or the diary writer) to accept routing metadata. Log every routed message with:

```typescript
{
  triggerType: "message",
  triggerSource: msg.senderId,
  channel: msg.channel,
  slug: msg.content.slice(0, 50),
  content: responseText,
  toolsUsed: [routeResult.tool],
  responseLatencyMs: elapsed,
  // New fields for routing analytics
  routingTier: routeResult.tier,
  routingConfidence: routeResult.confidence,
}
```

Extend the `DiaryWriteInput` type and the Drizzle diary schema (if needed) with `routingTier` (integer, nullable) and `routingConfidence` (real, nullable) columns.

### Req-8: Router initialization
In `packages/daemon/src/index.ts`, initialize the router after fleet client init and before the message handler:

```typescript
const keywordRouter = new KeywordRouter();
const embeddingRouter = await EmbeddingRouter.create(); // loads model + centroids
const router = new MessageRouter(keywordRouter, embeddingRouter);
```

The embedding model load is async and takes 2-5 seconds on first run. This should happen during daemon startup (before "Nova daemon ready" log). If model loading fails, log a warning and disable Tier 2 (keyword routing still works, agent fallback always available).

## Scope
- **IN**: Router cascade module, keyword router (20+ patterns), embedding router (local MiniLM model), intent seed data (JSON files), fleet tool execution path, response formatting, integration into index.ts message loop, diary logging extension, router initialization
- **OUT**: Changes to fleet services, new fleet tools, Telegram adapter changes, conversation history for routed messages (routed responses are one-shot -- no multi-turn), dashboard analytics UI for routing stats, Tier 0 command changes, Agent SDK changes

## Impact
| Area | Change |
|------|--------|
| `packages/daemon/src/brain/router.ts` | NEW -- MessageRouter class, RouteResult type, response formatter |
| `packages/daemon/src/brain/keyword-router.ts` | NEW -- KeywordRouter class, 20+ regex pattern table |
| `packages/daemon/src/brain/embedding-router.ts` | NEW -- EmbeddingRouter class, local MiniLM inference, cosine similarity |
| `packages/daemon/src/brain/intents/*.json` | NEW -- 12+ intent seed files with utterances and centroids |
| `packages/daemon/src/index.ts` | MODIFY -- add router init + route-before-agent logic in message handler |
| `packages/daemon/src/features/diary/writer.ts` | MODIFY -- extend DiaryWriteInput with routingTier, routingConfidence |
| `packages/daemon/package.json` | MODIFY -- add `@xenova/transformers` dependency |
| `@nova/db` diary schema | MODIFY -- add routing_tier (integer) and routing_confidence (real) columns (nullable) |

## Risks
| Risk | Mitigation |
|------|-----------|
| False positives route to wrong tool, user gets bad answer | Conservative threshold (0.82). Tier 1 regex patterns are hand-crafted and tested. Safe default is always agent fallback. Log confidence for tuning. |
| `@xenova/transformers` model loading slow or fails | Model loads at startup (not per-message). If load fails, Tier 2 is disabled gracefully -- Tier 1 and Tier 3 still work. First-run downloads 80MB model; subsequent runs use cached model. |
| Fleet tool returns unexpected shape, formatter breaks | Formatter has JSON.stringify fallback. Fleet tools already return structured `{ result, error }` shapes. |
| Pattern table maintenance burden | Patterns are static and declarative. Adding a new pattern is one line in the table. Intent JSON files are similarly low-maintenance. |
| Embedding model accuracy degrades for domain-specific queries | Centroids are computed from domain-specific seed utterances, not generic embeddings. Threshold is tunable via env var. |
| Memory cost of keeping model loaded | all-MiniLM-L6-v2 quantized uses ~80MB. The daemon already runs an Agent SDK session consuming far more. Negligible overhead. |
