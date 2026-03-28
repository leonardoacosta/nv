# Proposal: Add Universal Obligation Detection

## Change ID
`add-universal-obligation-detection`

## Summary

Extend obligation detection to cover Tier 1 (keyword) and Tier 2 (embedding) routed messages. Currently, obligations are only detected when messages reach Tier 3 (full Agent SDK), meaning a significant percentage of actionable messages bypass the obligation lifecycle entirely.

## Context
- Depends on: `add-smart-routing` (completed -- 4-tier message router in `packages/daemon/src/brain/router.ts`), `revive-obligations` (completed -- obligation pipeline wired into daemon)
- Conflicts with: none
- Current state: The 4-tier message router handles messages via Tier 0 (slash commands), Tier 1 (keyword regex), Tier 2 (embedding similarity), and Tier 3 (Agent SDK). Obligation detection in `packages/daemon/src/features/obligations/detector.ts` only runs after Tier 3 responses. The detector uses Claude to analyze user message + Nova response for action items. Tier 1/2 handle the majority of messages (calendar lookups, email sends, reminders) -- many of which contain implicit obligations.

## Motivation

When a user says "remind me to send the invoice tomorrow," Tier 1 routes to set_reminder. The reminder is created, but no obligation is tracked. If the reminder fires and the user doesn't act, there's no follow-up mechanism. The obligation system provides exactly this follow-up capability, but it never sees Tier 1/2 messages.

The same gap applies across many Tier 1/2 interactions. "Email John about the contract" routes to email_send and completes, but the implicit obligation ("ensure John responds" or "follow up on the contract") is lost. Universal detection closes this gap by running a lightweight signal check on every Tier 1/2 message and escalating to LLM classification only when signals are present.

## Requirements

### Req-1: Lightweight keyword-based obligation signals

Create `packages/daemon/src/features/obligations/signal-detector.ts`:

- Pattern-based detection (no LLM call) for common obligation signals:
  - "need to", "should", "must", "have to", "don't forget"
  - "follow up", "get back to", "check on", "make sure"
  - "deadline", "due by", "before Friday"
  - "promise", "committed to", "agreed to"
- Returns: `{ detected: boolean, confidence: number, signals: string[] }`
- Threshold: only flag if 2+ signals detected OR 1 high-confidence signal (high-confidence signals are deadline-related patterns like "due by", "before Friday", "deadline")

### Req-2: Post-routing obligation hook

In `packages/daemon/src/brain/router.ts`:

- After Tier 1/2 dispatch completes, run signal-detector on the original message
- If signals detected: enqueue lightweight obligation detection job
- The job calls Claude (Haiku, not Opus) with a focused prompt: "Given this message and tool response, is there an action item? Return JSON or null."
- If obligation detected: insert into obligation store with source="tier1" or "tier2"
- Signal detection and job enqueue are async (fire-and-forget) -- they do not block the response path

### Req-3: Obligation source tracking

Extend the obligations table and types:

- Add `detectionSource` field: "tier1" | "tier2" | "tier3" | "manual"
- Add `routedTool` field: which tool handled the original message (e.g., "calendar_today", "set_reminder")
- Update `ObligationRecord` and `ObligationRow` types in the store
- Enables analytics on detection coverage per tier

### Req-4: Cost controls

- Signal detector is free (regex/keyword, no LLM)
- LLM obligation detection only runs when signals are detected (not every Tier 1/2 message)
- Use Haiku model for Tier 1/2 obligation detection (cheaper than the model used in Tier 3)
- Max 10 obligation detection jobs per hour to cap costs (tracked via in-memory counter, resets hourly)
- Log detection rate and cost in diary

## Scope
- **IN**: `packages/daemon/src/features/obligations/signal-detector.ts` (new), `packages/daemon/src/brain/router.ts` (post-routing hook), obligation store types, obligations table schema
- **OUT**: Tier 3 detection (unchanged), obligation executor (unchanged), dashboard obligation UI (unchanged)

## Impact

| Area | Change |
|------|--------|
| `packages/daemon/src/features/obligations/signal-detector.ts` | NEW -- keyword-based signal detection, pattern matching, confidence scoring |
| `packages/daemon/src/brain/router.ts` | MODIFY -- post-routing obligation hook after Tier 1/2 dispatch |
| `packages/daemon/src/features/obligations/detector.ts` | MODIFY -- Haiku model option for lightweight detection |
| `packages/daemon/src/features/obligations/store.ts` | MODIFY -- detectionSource and routedTool fields on ObligationRecord |
| `@nova/db` obligations schema | MODIFY -- add detection_source and routed_tool columns |

## Risks

| Risk | Mitigation |
|------|-----------|
| Too many false positives from signal detector | 2+ signal threshold for low-confidence patterns, hourly rate cap of 10 detection jobs |
| Cost creep from Haiku calls | Rate limiting (10/hour max), diary cost tracking, signal gating ensures most Tier 1/2 messages never trigger LLM |
| Duplicate obligations (Tier 1 signal + Tier 3 full detection on same message) | Dedup by message ID before inserting -- if an obligation already exists for the source message, skip creation |
| Latency on Tier 1/2 response path | Signal detection and LLM classification are async (fire-and-forget), response is sent to the user before obligation detection begins |
| Signal patterns too broad (e.g., "should" matches casual usage) | Conservative pattern list, require 2+ signals for low-confidence matches, tunable via config |
