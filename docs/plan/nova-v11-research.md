# Nova v11 Research — 5 Game-Changing Features

Compiled from parallel research agents (2026-03-27). Each section includes competitive analysis, architecture recommendation, and implementation approach.

## 1. Async Job Queue (Unblock Chat During Heavy Tasks)

**Problem**: Nova's heavy tasks (261 tool calls, 21 min) block Telegram chat. User can't send another message.

**Best pattern**: In-memory JobQueue with bounded concurrency (2), message splitting (immediate ack + async result), cancellation via "cancel"/"stop".

**Key competitors**: OpenAI Responses API (background: true + polling), LangGraph (checkpoint/resume), AutoGPT (task queue with specialized sub-agents).

**Architecture**:
- `JobQueue` class: priority queue (high/normal/low), concurrency limit of 2, AbortController for cancellation
- Message splitting: "Queued (1 ahead). I'll respond when ready." → deliver result when done
- Cancel detection: regex for "cancel"/"stop"/"never mind" aborts running job
- Progress via `editMessageText` or `sendMessageDraft` (Telegram Bot API 9.5)

**Files**: `packages/daemon/src/queue/job-queue.ts` (new), `packages/daemon/src/index.ts` (modify handler)

**Effort**: Medium. **Impact**: Highest — transforms Nova from blocking to concurrent.

## 2. Proactive Digest System (Surface Issues Before Leo Asks)

**Problem**: Nova only responds when asked. No automated monitoring of email, Teams, ADO failures, PIM expiry.

**Best pattern**: Two-tier — thin gather + template ($0, <2s) for daily digests, LLM only for weekly summaries.

**Key competitors**: Google Gemini Scheduled Actions, Apple Intelligence Priority Notifications, Microsoft Copilot Prioritize My Inbox, Linear AI Triage Intelligence.

**Architecture**:
- Tier 1 (daily, $0): Direct fleet HTTP calls → rule-based classification → template formatting → Telegram with inline keyboards
- Tier 2 (weekly, ~$0.03): Agent SDK synthesis for weekly trend analysis
- P0 real-time (every 5 min): PIM expiry, production CI failures → immediate notification

**Classification rules (no LLM)**:
- Email: suppress noreply@/notifications@, P1 if from contacts/manager, P2 otherwise
- Teams: P1 if DM unanswered, P2 if @mentioned in channel
- ADO: P0 if prod pipeline failed, P1 if dev failed
- PIM: P0 if role expires within 2h

**Cron schedule**: 7am/noon/5pm weekdays (thin), Monday 9am (LLM weekly), every 5 min (P0 check)

**Files**: `packages/daemon/src/features/digest/` (new module: gather, classify, format, scheduler, realtime)

**Effort**: Medium. **Impact**: Very high — makes Nova proactive instead of reactive.

## 3. Smart Message Routing (80% of Queries in 2s Instead of 2min)

**Problem**: ALL non-command messages go to Agent SDK ($0.10-2.00, 5-120s). Most are simple lookups.

**Best pattern**: 4-tier cascade — regex (35% catch rate) → embedding similarity (15%) → agent (20%).

**Key competitors**: vLLM Semantic Router (ModernBERT, 98x faster), NadirClaw (all-MiniLM-L6-v2, 10ms, 60% cost savings), LangChain semantic-router.

**Architecture**:
- Tier 0: Exact command match (existing /commands) — <1ms, $0
- Tier 1: Regex/keyword router (20 patterns) — <1ms, $0, catches "what's on my calendar?"
- Tier 2: Embedding similarity (all-MiniLM-L6-v2, 80MB local) — 50ms, $0, catches paraphrases
- Tier 3: Full Agent SDK — 5-120s, $0.10-2.00, only for complex reasoning

**Threshold**: similarity >= 0.82 → direct tool call. < 0.82 → agent. Safe default: route to agent on uncertainty.

**Estimated savings**: 80% cost reduction ($50/day → $10/day at 100 messages)

**Files**: `packages/daemon/src/brain/router.ts`, `keyword-router.ts`, `embedding-router.ts` (all new)

**Effort**: Medium. **Impact**: Very high — transforms cost structure and response speed.

## 4. Response Streaming (Show Progress, Not Silence)

**Problem**: Nova takes 1-21 min. User sees only "typing..." with no indication of what's happening.

**Best pattern**: Telegram `sendMessageDraft` (Bot API 9.5, native progressive rendering) with tool status indicators.

**Key competitors**: ChatGPT (token-by-token SSE), Claude.ai (tool indicators), Perplexity (step-by-step plan execution).

**Architecture**:
- Agent SDK: enable `includePartialMessages: true` for `stream_event` messages
- Yield rich events: `text_delta`, `tool_start`, `tool_progress`, `tool_done`, `done`
- TelegramStreamWriter: manages draft lifecycle, throttles at 300ms, humanizes tool names
- Fallback: `editMessageText` at 1000ms throttle if `sendMessageDraft` unavailable

**User sees**:
```
[Draft] Thinking...
[Draft] Checking Teams... (3s)
[Draft] Reading Teams chat... (2s)
[Draft] The Azure team discussed three topics:
[Draft grows as tokens stream]
[Final message with full formatting]
```

**Files**: `packages/daemon/src/brain/agent.ts` (richer stream events), `packages/daemon/src/channels/telegram.ts` (TelegramStreamWriter)

**Effort**: Small-Medium. **Impact**: High — dramatically improves perceived responsiveness.

## 5. Autonomous Obligation System (Nova Works Without Being Asked)

**Problem**: Obligation system exists end-to-end but table is empty. Infrastructure unused.

**Key competitors**: Devin (task assignment, ACU billing, $2/15min), BabyAGI (create/execute/prioritize loop), Claude Code /loop + /schedule.

**Root cause**: Likely classifier prompt too conservative OR messages not reaching detector.

**Architecture (mostly exists, needs wiring)**:
- Debug empty table: check classifier prompt, message routing to detector
- Add budget gate: $5/day cap for autonomous work, reserves 20% of weekly budget
- Model routing: P0-P1 → Sonnet, P2+ → Haiku (4x cheaper for routine tasks)
- Escalation: after 3 failed attempts → status "escalated" + Telegram notification
- Quiet hours: reuse watcher's 22:00-07:00 config, no autonomous work overnight
- Manual creation: `/obligation "Check Teams for client feedback"` Telegram command
- Obligation sources: watcher findings → auto-create obligations

**Cost model with controls**: worst case $35/week (with $5/day cap + Haiku for P2+), realistic $10-15/week.

**Files**: `packages/daemon/src/config.ts` (autonomy config additions), `index.ts` or `orchestrator` (budget gate), `telegram.ts` (`/obligation` command)

**Effort**: Small (infrastructure exists). **Impact**: High — Nova becomes genuinely autonomous.

## Implementation Priority

| Wave | Feature | Effort | Cost Savings | UX Impact |
|------|---------|--------|-------------|-----------|
| 1 | Smart Message Routing | Medium | 80% | Fast responses for 80% of queries |
| 1 | Response Streaming | Small | None | Dramatic perceived speed improvement |
| 2 | Async Job Queue | Medium | Indirect | Unblocks chat during heavy tasks |
| 2 | Proactive Digest | Medium | Replaces expensive briefing | Nova surfaces issues before asked |
| 3 | Obligation Revival | Small | Haiku routing saves 60-70% | Nova works autonomously |

Wave 1 targets the biggest pain points (slow + expensive responses).
Wave 2 transforms the interaction model (async + proactive).
Wave 3 enables autonomous behavior (self-directed work).
