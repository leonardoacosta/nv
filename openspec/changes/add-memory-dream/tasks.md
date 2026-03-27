# Implementation Tasks

<!-- beads:epic:TBD -->

## Memory-svc: Dream Types

- [x] [1.1] [P-1] Create packages/tools/memory-svc/src/dream/types.ts -- DreamConfig (topic_max_kb, debounce_hours), TopicStats (topic, sizeBytes, lineCount, updatedAt), DreamOrientation (topics, totalSizeBytes, timestamp), RuleStats (dedupedLines, datesNormalized, stalePathsRemoved, whitespaceFixed), RuleResult (content, needsLlm, stats), DreamResult (topicsProcessed, bytesBefore, bytesAfter, llmTopics, durationMs) [owner:api-engineer]
- [x] [1.2] [P-1] Create packages/tools/memory-svc/src/dream/index.ts -- barrel export for orient, rules, compress, writeback, types [owner:api-engineer]

## Memory-svc: Orient Phase

- [x] [2.1] [P-1] Create packages/tools/memory-svc/src/dream/orient.ts -- read all rows from memory table via Drizzle (db.select().from(memory)), compute per-topic stats (Buffer.byteLength for size, split newlines for lineCount), return DreamOrientation [owner:api-engineer]

## Memory-svc: Deterministic Rules Phase

- [x] [3.1] [P-1] Create packages/tools/memory-svc/src/dream/rules.ts -- main applyRules(content, updatedAt, topicMaxKb) function returning RuleResult [owner:api-engineer]
- [x] [3.2] [P-1] Implement deduplication rule -- remove exact duplicate lines, for near-duplicates (Levenshtein distance < 10% of line length, min line length 20 chars) keep the longer variant. Track count in RuleStats.dedupedLines [owner:api-engineer]
- [x] [3.3] [P-1] Implement date normalization rule -- regex match relative date phrases ("yesterday", "last week", "recently", "a few days ago", "today", "this morning", "last month") and replace with absolute dates computed from the topic's updatedAt. Track count in RuleStats.datesNormalized [owner:api-engineer]
- [x] [3.4] [P-1] Implement whitespace cleanup rule -- collapse 3+ consecutive blank lines to 2, trim trailing whitespace per line, remove leading/trailing blank lines. Track changes in RuleStats.whitespaceFixed [owner:api-engineer]
- [x] [3.5] [P-2] Implement stale path removal rule -- regex match lines with file paths (~/*, /home/*, packages/*, apps/*), expand ~ to homedir, check fs.existsSync, only remove lines where path is primary content (line starts with path or "- path" pattern). Track count in RuleStats.stalePathsRemoved [owner:api-engineer]
- [x] [3.6] [P-1] After all rules, check if Buffer.byteLength(content) > topicMaxKb * 1024 and set needsLlm accordingly [owner:api-engineer]

## Memory-svc: LLM Compression Phase

- [x] [4.1] [P-2] Create packages/tools/memory-svc/src/dream/compress.ts -- compressTopic(topic, content, targetKb, compressor) function accepting injected LLM callback with consolidation system prompt, 60s timeout. Return compressed content string or null on failure [owner:api-engineer]
- [x] [4.2] [P-2] Handle Agent SDK unavailability gracefully -- if compressor callback throws or times out, return null. Caller keeps rules-phase result [owner:api-engineer]

## Memory-svc: Writeback Phase

- [x] [5.1] [P-1] Create packages/tools/memory-svc/src/dream/writeback.ts -- writeBackTopics(changes: Array<{topic, content}>, memorySvcUrl) function that calls POST /write for each changed topic [owner:api-engineer]
- [x] [5.2] [P-2] Implement _dream_meta topic write -- JSON payload with lastDreamAt (ISO string), stats object (topicsProcessed, bytesBeforeSummed, bytesAfterSummed, topicsCompressedByLlm count, rulesApplied aggregate) [owner:api-engineer]
- [x] [5.3] [P-2] Implement diary entry write -- call writeEntry() via injected DiaryWriter callback with trigger_type "dream", slug summarizing stats (e.g., "Dream: 16 topics, 75KB -> 52KB"), no tools_used [owner:api-engineer]

## Daemon: Dream Orchestrator

- [ ] [6.1] [P-1] Create packages/daemon/src/features/dream/orchestrator.ts -- runDream(config, agent, memorySvcUrl) function that executes full 4-phase cycle: orient -> rules -> LLM compress -> writeback. Return DreamResult. Support dry_run flag that skips writeback [owner:api-engineer]
- [ ] [6.2] [P-1] Create packages/daemon/src/features/dream/index.ts -- barrel export for orchestrator, scheduler [owner:api-engineer]
- [ ] [6.3] [P-2] Expose POST /dream on daemon health port (:8400) -- run dream cycle, return DreamResult JSON. Accept ?dry_run=true query param [owner:api-engineer]
- [ ] [6.4] [P-2] Expose GET /dream/status on daemon health port -- read _dream_meta topic from memory-svc, return stats + per-topic sizes [owner:api-engineer]

## Daemon: Dream Scheduler

- [ ] [7.1] [P-2] Create packages/daemon/src/features/dream/scheduler.ts -- DreamScheduler class following ProactiveWatcher pattern (start/stop, setInterval, config-driven) [owner:api-engineer]
- [ ] [7.2] [P-2] Implement cron trigger -- check hourly if current hour matches config.dream.cron_hour (CDT), trigger dream if match [owner:api-engineer]
- [ ] [7.3] [P-2] Implement interaction counter trigger -- export incrementInteractionCount() called from NovaAgent.processMessage(). When count >= config.dream.interaction_threshold, trigger dream and reset [owner:api-engineer]
- [ ] [7.4] [P-2] Implement size trigger -- on scheduler tick, read total memory size from _dream_meta or compute via orient. If >= config.dream.size_threshold_kb, trigger dream [owner:api-engineer]
- [ ] [7.5] [P-2] Implement debounce -- read _dream_meta.lastDreamAt, skip if elapsed < config.dream.debounce_hours [owner:api-engineer]
- [ ] [7.6] [P-2] Wire DreamScheduler into daemon main init -- start after Agent SDK and memory-svc are ready, stop on shutdown [owner:api-engineer]

## Daemon: Interaction Counter Hook

- [ ] [8.1] [P-2] Modify packages/daemon/src/brain/agent.ts -- after processMessage returns, call incrementInteractionCount() from dream scheduler module. Import is lazy to avoid circular deps [owner:api-engineer]

## Config: nv.toml

- [ ] [9.1] [P-1] Add [dream] section to config/nv.toml with defaults: enabled=true, cron_hour=3, interaction_threshold=50, size_threshold_kb=60, debounce_hours=12, topic_max_kb=4 [owner:api-engineer]
- [ ] [9.2] [P-1] Parse [dream] config in daemon config loader -- add DreamConfig type to daemon config types, parse from TOML with defaults [owner:api-engineer]

## Telegram: /dream Command

- [ ] [10.1] [P-2] Create packages/daemon/src/telegram/commands/dream.ts -- buildDreamReply() that calls POST :8400/dream, formats before/after stats as Telegram-friendly text. buildDreamStatusReply() that calls GET :8400/dream/status, formats per-topic sizes sorted by size desc with total KB and last dream timestamp [owner:api-engineer]
- [ ] [10.2] [P-2] Register /dream in packages/daemon/src/channels/telegram.ts -- add bot.onText(/^\/dream(@\S+)?(\s+(.+))?$/) handler, route "status" subarg to buildDreamStatusReply(), default to buildDreamReply(). Import buildDreamReply and buildDreamStatusReply [owner:api-engineer]

## CLI: nv dream Command

- [ ] [11.1] [P-2] Create packages/cli/src/commands/dream.ts -- dreamCmd(subcommand, flags) function. "nv dream" calls POST :8400/dream and prints stats table. "nv dream status" calls GET :8400/dream/status and prints per-topic sizes. "nv dream --dry-run" calls POST :8400/dream?dry_run=true and prints what would change [owner:api-engineer]
- [ ] [11.2] [P-2] Register in packages/cli/src/index.ts -- add case "dream" to switch, call dreamCmd with argv[3] and --dry-run flag detection [owner:api-engineer]

## Verify

- [ ] [12.1] tsc --noEmit passes for @nova/memory-svc (dream module) [owner:api-engineer]
- [ ] [12.2] tsc --noEmit passes for daemon (dream scheduler + orchestrator) [owner:api-engineer]
- [ ] [12.3] tsc --noEmit passes for @nova/cli (dream command) [owner:api-engineer]
- [ ] [12.4] Daemon starts with [dream] config and DreamScheduler initializes without error [owner:api-engineer]
- [ ] [12.5] POST :8400/dream returns DreamResult JSON with correct stats [owner:api-engineer]
- [ ] [12.6] GET :8400/dream/status returns per-topic sizes and _dream_meta [owner:api-engineer]
- [ ] [12.7] Rules phase correctly deduplicates exact duplicate lines in a test topic [owner:api-engineer]
- [ ] [12.8] Rules phase converts "yesterday" to an absolute date based on updatedAt [owner:api-engineer]
- [ ] [12.9] nv dream calls daemon and prints stats [owner:api-engineer]
- [ ] [12.10] [user] Manual test: run /dream in Telegram, verify before/after stats displayed [owner:api-engineer]
- [ ] [12.11] [user] Manual test: run nv dream --dry-run, verify no writes occur but stats shown [owner:api-engineer]
- [ ] [12.12] [user] Manual test: verify _dream_meta topic created after dream cycle with correct JSON [owner:api-engineer]
