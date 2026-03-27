# Proposal: Add Memory Dream (Consolidation)

## Change ID
`add-memory-dream`

## Summary

Build a memory consolidation system ("dream") that compresses, prunes, and organizes Nova's memory topics to prevent context bloat. Runs a 4-phase pipeline: orient (stats), deterministic rules (dedup, date normalization, stale removal), LLM compression (for oversize topics), and writeback. Triggered automatically (daily cron, interaction count, size threshold) or manually via Telegram `/dream` and CLI `nv dream`.

## Context
- Depends on: `add-memory-svc` (completed -- memory-svc at :4101 with read/write/search)
- Conflicts with: none
- Current state: 16 memory topics totaling ~75KB in Postgres (`memory` table). The system prompt instructs Nova to read `conversations` + `tasks` + `people` before every response (~35KB loaded per interaction). Memory grows unbounded with no pruning, deduplication, or date normalization.
- Memory schema: `packages/db/src/schema/memory.ts` -- uuid id, text topic (unique), text content, vector embedding, timestamp updatedAt
- Daemon watcher: `packages/daemon/src/features/watcher/proactive.ts` -- setInterval-based scheduler with quiet hours, used as reference pattern
- Agent SDK: `packages/daemon/src/brain/agent.ts` -- `query()` from `@anthropic-ai/claude-agent-sdk` with MCP servers, used for LLM-based reasoning
- Config: `config/nv.toml` -- TOML config with `[proactive_watcher]`, `[autonomy]`, `[telegram]` sections
- CLI: `packages/cli/src/index.ts` -- switch-case command dispatch
- Telegram: `packages/daemon/src/channels/telegram.ts` -- `bot.onText()` + `buildXxxReply()` pattern

## Motivation

Nova's memory system accumulates content without any pruning or consolidation. Over time this causes:

1. **Context bloat**: ~35KB loaded into every agent interaction from just 3 topics. As memory grows, token usage increases linearly and the signal-to-noise ratio degrades.
2. **Stale information**: Relative dates ("yesterday", "last week") become meaningless after the original context is lost. Resolved issues, completed projects, and outdated patterns linger.
3. **Redundancy**: The same fact may appear in multiple places within a topic as Nova appends new observations without checking for duplicates.
4. **No budget enforcement**: There is no per-topic size limit, so a single topic can grow to dominate the context window.

A periodic consolidation system (modeled after sleep/dream consolidation) will compress memories, prune stale content, enforce per-topic budgets, and keep memory lean for efficient agent interactions.

## Requirements

### Req-1: Consolidation Engine -- Orient Phase

Create `packages/tools/memory-svc/src/dream/orient.ts`:

- Read all memory topics from Postgres via Drizzle (`db.select().from(memory)`)
- Compute per-topic stats: `{ topic, sizeBytes, lineCount, updatedAt }`
- Compute total size across all topics
- Return `DreamOrientation` object: `{ topics: TopicStats[], totalSizeBytes, timestamp }`

### Req-2: Consolidation Engine -- Deterministic Rules Phase

Create `packages/tools/memory-svc/src/dream/rules.ts`:

Apply these rules to each topic's content string (no LLM, fast):

1. **Deduplicate**: Remove exact duplicate lines. For near-duplicates (Levenshtein distance < 10% of line length), keep the longer/more recent variant.
2. **Date normalization**: Convert relative date phrases ("yesterday", "last week", "recently", "a few days ago", "today") to absolute dates using the topic's `updatedAt` timestamp as the reference point. Pattern-match common English relative date expressions.
3. **Whitespace cleanup**: Collapse 3+ consecutive blank lines to 2. Trim trailing whitespace from each line. Remove leading/trailing blank lines from the topic.
4. **Stale path removal**: Remove lines that reference file paths matching `~/`, `/home/`, `packages/`, `apps/` patterns where the path does not exist on disk (verify with `fs.existsSync` after expanding `~`). Only remove lines where the path is the primary content (not incidental mentions).
5. **Budget check**: After all rules, if a topic still exceeds `topic_max_kb` (default 4KB), flag it for LLM compression in the next phase.

Input/output: `(content: string, updatedAt: Date, topicMaxKb: number) => { content: string, needsLlm: boolean, stats: RuleStats }` where `RuleStats` tracks counts of each rule applied.

### Req-3: Consolidation Engine -- LLM Compression Phase

Create `packages/tools/memory-svc/src/dream/compress.ts`:

For each topic flagged by the rules phase as still exceeding the budget:

- Use the Agent SDK `query()` (imported from `@anthropic-ai/claude-agent-sdk`) to call the LLM with a consolidation prompt:
  - System prompt: "You are a memory compressor. Compress the following memory topic to under {target_kb}KB. Preserve: recent decisions, active projects, key relationships, dates, names, technical details. Remove: stale context, resolved issues, outdated patterns, redundant information. Output only the compressed content -- no preamble, no explanation."
  - User prompt: the topic content
  - No tools allowed (`allowedTools: []`), no MCP servers -- pure text compression
  - `maxTurns: 1` -- single response, no conversation
- If the Agent SDK is unavailable (import fails, no API key), skip LLM compression and log a warning. The topic stays at its post-rules size.
- The LLM phase runs inside the daemon process (which already has Agent SDK wired). The memory-svc exposes a `/dream` HTTP endpoint that the daemon calls after running LLM compression, or the memory-svc itself calls the daemon's LLM endpoint.

**Architecture decision**: The consolidation engine (orient + rules + writeback) lives in memory-svc. The LLM compression step is exposed as a daemon HTTP endpoint (`POST /dream/compress`) that the memory-svc calls when a topic needs LLM help. This keeps the LLM dependency in the daemon (where the Agent SDK already lives) and keeps memory-svc focused on data operations.

Alternative: The daemon orchestrates the full dream cycle, calling memory-svc for reads/writes. This is simpler but couples the daemon to the dream lifecycle.

**Chosen approach**: Daemon orchestrates the full cycle. The daemon reads all topics (via memory-svc `/read` or direct Drizzle), runs rules (imported from a shared dream module), runs LLM compression (via Agent SDK), and writes back (via memory-svc `/write`). This matches the existing pattern where the daemon owns all LLM-dependent features (briefing synthesis, obligation execution, self-assessment).

### Req-4: Consolidation Engine -- Write Back Phase

Create `packages/tools/memory-svc/src/dream/writeback.ts`:

- For each topic that changed (rules or LLM modified the content):
  - Call memory-svc `POST /write` with the consolidated content (this handles Postgres upsert + filesystem sync + re-embedding)
- After all topics written:
  - Write a `_dream_meta` topic with JSON: `{ lastDreamAt, stats: { topicsProcessed, bytesBeforeSummed, bytesAfterSummed, topicsCompressedByLlm, rulesApplied: { deduped, datesNormalized, staleRemoved, whitespaceFixed } } }`
  - Write a diary entry via `writeEntry()` with trigger_type `"dream"`, summarizing the consolidation stats
- Return `DreamResult`: `{ topicsProcessed, bytesBefore, bytesAfter, llmTopics: string[], duration_ms }`

### Req-5: Daemon Dream Scheduler

Create `packages/daemon/src/features/dream/scheduler.ts`:

Integrate with the daemon's lifecycle (started in the daemon's main init, stopped on shutdown):

- **Cron trigger**: Run daily at `config.dream.cron_hour` (default 3 AM). Use `setInterval` checking hourly (like ProactiveWatcher) or a simple cron-like check on the main watcher interval.
- **Interaction trigger**: Maintain an in-memory counter of agent responses (increment in `NovaAgent.processMessage`). When counter reaches `config.dream.interaction_threshold` (default 50), trigger dream and reset counter.
- **Size trigger**: On each memory read (or periodically), check total memory size. If exceeds `config.dream.size_threshold_kb` (default 60), trigger dream.
- **Debounce**: Skip if last dream ran less than `config.dream.debounce_hours` (default 12) hours ago. Read `_dream_meta` topic to get `lastDreamAt`.

Config from `nv.toml` `[dream]` section:
```toml
[dream]
enabled = true
cron_hour = 3
interaction_threshold = 50
size_threshold_kb = 60
debounce_hours = 12
topic_max_kb = 4
```

### Req-6: Daemon Dream Orchestrator

Create `packages/daemon/src/features/dream/orchestrator.ts`:

The full dream cycle function callable from scheduler, Telegram command, or CLI endpoint:

1. Call memory-svc `POST /read` for each topic (or use `db.select().from(memory)` directly since daemon has `@nova/db`)
2. Run orient phase (compute stats)
3. Run rules phase on each topic
4. Run LLM compression on flagged topics (via Agent SDK `query()`)
5. Write back changed topics via memory-svc `POST /write`
6. Write `_dream_meta` + diary entry
7. Return `DreamResult`

Expose as an HTTP endpoint on the daemon's health port (:8400): `POST /dream` (run dream cycle, return stats) and `GET /dream/status` (return `_dream_meta` content).

### Req-7: Telegram /dream Command

Create `packages/daemon/src/telegram/commands/dream.ts`:

- `/dream` -- run consolidation manually. Show "Dreaming..." message, run the cycle, then edit the message with before/after stats (topics processed, KB before/after, duration).
- `/dream status` -- show memory stats: per-topic sizes (sorted by size desc), total KB, last dream timestamp from `_dream_meta`, time since last dream.

Register in `packages/daemon/src/channels/telegram.ts` using the existing `bot.onText()` + `handleDirectCommand()` pattern.

### Req-8: CLI nv dream Command

Create `packages/cli/src/commands/dream.ts`:

- `nv dream` -- call daemon `POST :8400/dream`, display stats table
- `nv dream status` -- call daemon `GET :8400/dream/status`, display per-topic sizes + last dream info
- `nv dream --dry-run` -- call daemon `POST :8400/dream?dry_run=true`, show what would be pruned without writing

Register in `packages/cli/src/index.ts` switch statement.

### Req-9: nv.toml Configuration

Add `[dream]` section to `config/nv.toml`:

```toml
[dream]
enabled = true
cron_hour = 3
interaction_threshold = 50
size_threshold_kb = 60
debounce_hours = 12
topic_max_kb = 4
```

Parse in the daemon's config loader. The dream scheduler reads these values at startup.

## Scope
- **IN**: 4-phase consolidation engine (orient, rules, LLM compress, writeback), daemon scheduler (cron + interaction + size triggers), Telegram `/dream` + `/dream status`, CLI `nv dream` + `nv dream status` + `nv dream --dry-run`, `[dream]` config in nv.toml, `_dream_meta` topic for state, diary entry logging
- **OUT**: Message table compression (separate concern), conversation history pruning, cross-topic merging/splitting, memory topic creation/deletion (only content modification), embedding retraining, dashboard UI for dream stats

## Impact

| Area | Change |
|------|--------|
| `packages/tools/memory-svc/src/dream/orient.ts` | NEW -- orient phase (read all + compute stats) |
| `packages/tools/memory-svc/src/dream/rules.ts` | NEW -- deterministic rules (dedup, dates, stale, whitespace) |
| `packages/tools/memory-svc/src/dream/compress.ts` | NEW -- LLM compression wrapper |
| `packages/tools/memory-svc/src/dream/writeback.ts` | NEW -- write back + meta + diary |
| `packages/tools/memory-svc/src/dream/types.ts` | NEW -- shared types (DreamOrientation, RuleStats, DreamResult, DreamConfig) |
| `packages/tools/memory-svc/src/dream/index.ts` | NEW -- barrel export |
| `packages/daemon/src/features/dream/scheduler.ts` | NEW -- cron + interaction + size triggers |
| `packages/daemon/src/features/dream/orchestrator.ts` | NEW -- full dream cycle orchestration |
| `packages/daemon/src/features/dream/index.ts` | NEW -- barrel export |
| `packages/daemon/src/telegram/commands/dream.ts` | NEW -- /dream + /dream status |
| `packages/daemon/src/channels/telegram.ts` | MODIFY -- register /dream onText handler + import |
| `packages/cli/src/commands/dream.ts` | NEW -- nv dream + nv dream status + --dry-run |
| `packages/cli/src/index.ts` | MODIFY -- add dream command to switch |
| `config/nv.toml` | MODIFY -- add [dream] section |
| Daemon main init | MODIFY -- start dream scheduler |

## Risks

| Risk | Mitigation |
|------|-----------|
| LLM compression loses critical information | Consolidation prompt explicitly lists preservation priorities (recent decisions, active projects, names, dates). Dry-run mode allows preview. `_dream_meta` logs stats for audit. Original content can be recovered from diary entries or git history of filesystem sync files. |
| Agent SDK unavailable in daemon context | LLM phase is best-effort -- if Agent SDK import fails or API key is missing, dream completes with rules-only compression and logs a warning. |
| Filesystem path validation false positives | Only remove lines where a file path is the primary content (e.g., "Refer to ~/dev/nv/old-file.ts" where the file no longer exists). Lines with incidental path mentions (e.g., "the pattern in packages/db/ uses...") are preserved. Conservative regex matching. |
| Dream runs during active conversation | Debounce prevents running within 12 hours of last dream. Cron is at 3 AM (quiet hours). Write operations are atomic per-topic (single upsert). No lock needed -- concurrent reads see either old or new content. |
| Near-duplicate detection too aggressive | Levenshtein threshold at 10% of line length is conservative. Short lines (< 20 chars) are excluded from near-duplicate matching to avoid false positives on common phrases. |
| Large topic causes slow LLM compression | Per-topic timeout of 60 seconds. If LLM does not respond in time, keep the rules-phase result and log a warning. |
