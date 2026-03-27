# Proposal: Add Proactive Digest

## Change ID
`add-proactive-digest`

## Summary

Build a two-tier scheduled digest system that proactively surfaces issues before Leo asks. Tier 1 (thin gather + template, daily at 7am/noon/5pm, $0 cost) calls fleet services via HTTP, classifies items with rule-based heuristics, applies suppression (content hash dedup, cooldown, nothing-changed skip), and sends a formatted Telegram notification with inline action buttons. Tier 2 (LLM synthesis, weekly Monday 9am, ~$0.03) uses Agent SDK to aggregate weekly trends. A P0 real-time loop (every 5 min) catches PIM role expirations and production CI failures for immediate notification.

## Context
- Extends: `packages/daemon/src/features/` (new `digest/` feature directory)
- Fleet services: memory-svc :4101, messages-svc :4102, channels-svc :4103, teams-svc :4105, graph-svc :4107 (calendar, mail, ADO, PIM)
- Fleet client: `packages/daemon/src/fleet-client.ts` (`fleetGet`, `fleetPost` with 5s timeout)
- Telegram: `packages/daemon/src/channels/telegram.ts` (`TelegramAdapter`, `buildKeyboard`, `SendMessageOptions`)
- Inline keyboards: already used for obligations (`obligationKeyboard`) and watcher (`watcherKeyboard`) with callback data routing in `index.ts`
- Config: `packages/daemon/src/config.ts` (`Config` interface, `loadConfig`, TOML parsing from `config/nv.toml`)
- Prior art: briefing scheduler uses `setInterval` poll with hour check; watcher uses quiet hours pattern (`quietStart`, `quietEnd`); dream scheduler uses `_dream_meta` memory topic for state
- Agent SDK: `query()` from `@anthropic-ai/claude-agent-sdk` used in briefing synthesizer for LLM calls
- Existing briefing: `packages/daemon/src/features/briefing/` (daily 7am, LLM-based, full synthesis) -- digest is complementary, not a replacement

## Motivation

The existing morning briefing runs once daily at 7am with full LLM synthesis (~$0.03/run). Between briefings, Leo has no visibility into accumulating issues unless he manually checks each channel. Important items slip through:

1. **Human emails buried under automated noise**: Without classification, Leo must manually scan inbox to find emails from real people among CI notifications and marketing.
2. **Unanswered Teams DMs**: No proactive alert when a coworker has been waiting for a reply.
3. **PIM role expirations**: Azure PIM roles expire silently -- missing the 2-hour renewal window means losing access to production resources.
4. **ADO production failures**: Build failures in production pipelines need immediate attention but currently require manual dashboard checks.
5. **Midday context loss**: By noon, the morning briefing is stale. The 5pm digest catches items before end of day.

The digest system fills these gaps with a zero-cost thin gather layer (no LLM for daily digests) and surgical real-time checks for P0 items.

## Requirements

### Req-1: Gather module -- call fleet services via HTTP

Create `packages/daemon/src/features/digest/gather.ts` that fetches data from fleet services using the existing `fleetGet`/`fleetPost` functions from `fleet-client.ts`.

Data sources to gather:

| Source | Fleet Call | Port | Path | Returns |
|--------|-----------|------|------|---------|
| Unread email | `fleetGet(4107, "/outlook/unread")` | graph-svc | `/outlook/unread` | `{ result: string }` with unread email summary |
| Teams chats | `fleetGet(4105, "/teams/chats")` | teams-svc | `/teams/chats` | `{ result: string }` with recent chats |
| Calendar today | `fleetGet(4107, "/calendar/today")` | graph-svc | `/calendar/today` | `{ result: string }` with today's events |
| PIM status | `fleetGet(4107, "/pim/roles")` | graph-svc | `/pim/roles` | `{ result: string }` with active/eligible roles |
| ADO builds | `fleetGet(4107, "/ado/builds")` | graph-svc | `/ado/builds` | `{ result: string }` with recent build status |
| Obligations | Direct DB query via `pool` | N/A | N/A | Pending/in-progress obligations (same query as briefing synthesizer) |

All fleet calls use `Promise.allSettled` with the existing fleet-client 5s timeout. Each source records a status (`"ok"` | `"unavailable"` | `"empty"`) in a `GatherResult.sourcesStatus` map, matching the briefing pattern.

The gather function accepts a `GatherDeps` interface: `{ pool: Pool; logger: Logger }`.

Return type: `GatherResult` with typed fields for each source's parsed data (not raw strings -- parse into structured items where possible).

### Req-2: Classify module -- rule-based P0/P1/P2/suppress heuristics

Create `packages/daemon/src/features/digest/classify.ts` that takes `GatherResult` and produces an array of `DigestItem` objects, each with a priority level.

Priority levels:
- **P0** (immediate): PIM roles expiring within 2 hours, ADO production build failures
- **P1** (important): Email from humans (not noreply@/notifications@/no-reply@), Teams DM unanswered > 1 hour, calendar events starting within 30 minutes, overdue obligations
- **P2** (informational): Email from known contacts (non-urgent), Teams @mentions, upcoming calendar events (> 30 min out), other obligations
- **Suppress**: Email from noreply@/notifications@/no-reply@/mailer-daemon@, Teams channel chatter (not DM, not @mentioned), automated CI notification emails

Classification rules (applied in order):
1. **Email**: Check sender against suppression list (domain patterns: `noreply@*`, `notifications@*`, `no-reply@*`, `mailer-daemon@*`). Remaining: check if from contacts (P1) or unknown (P2).
2. **Teams**: DM unanswered > 1h = P1. @mentioned in channel = P2. Everything else = suppress.
3. **Calendar**: Event starting within 30 min = P1. Others = P2.
4. **PIM**: Role expiring within 2h = P0. Others = P2.
5. **ADO**: Production pipeline failure = P0. Non-prod failure = P2. Success = suppress.
6. **Obligations**: Overdue = P1. Pending with deadline today = P1. Others = P2.

Each `DigestItem` has: `id: string` (deterministic from source + content hash), `source: string`, `priority: "P0" | "P1" | "P2"`, `title: string`, `detail: string`, `actionable: boolean`, `sourceId?: string` (for callback routing).

### Req-3: Suppress module -- content hash dedup, cooldown, nothing-changed

Create `packages/daemon/src/features/digest/suppress.ts` that filters out items already sent or unchanged.

**Content hash dedup**: Compute a SHA-256 hash of `item.source + item.title + item.detail`. Store sent hashes in the `_digest_meta` memory topic (JSON blob with `{ sentHashes: Record<string, number> }` where value is the unix timestamp of when it was sent). Skip items whose hash was sent within the last 24 hours.

**Cooldown per item**: After sending a P1 item, apply a 4-hour cooldown before re-sending the same hash. P0 items have a 30-minute cooldown (they are urgent). P2 items have a 12-hour cooldown.

**Nothing-changed skip**: If the entire digest output (after classification + dedup) is empty, skip sending the Telegram message entirely. Log "Digest: nothing new to report" at debug level.

**Hash cleanup**: On each digest run, prune hashes older than 48 hours from the `_digest_meta` topic to prevent unbounded growth.

State persistence: Read/write `_digest_meta` memory topic via direct pool query (same pattern as `_dream_meta` in the dream orchestrator). The memory topic stores JSON with `sentHashes` and `lastDigestAt` timestamp.

### Req-4: Format module -- Telegram markdown template

Create `packages/daemon/src/features/digest/format.ts` that renders `DigestItem[]` into Telegram Markdown (V1, not V2 -- matching the existing briefing pattern).

Template structure:
```
*[Tier] Digest* -- [time]

[P0 section if any P0 items]
*URGENT*
- [icon] [title]: [detail]

[P1 section if any P1 items]
*Action Needed*
- [icon] [title]: [detail]

[P2 section if any P2 items]
*FYI*
- [icon] [title]: [detail]

_[N] items | [sources with data]_
```

Icons per source (plain text, no emoji per project rules): `[Mail]`, `[Teams]`, `[Cal]`, `[PIM]`, `[ADO]`, `[Ob]`.

Inline keyboards on P0 and P1 items:
- P0 PIM: `[Activate Role]` (callback: `digest:pim:activate:{roleId}`)
- P0 ADO: `[View Build]` (callback: `digest:ado:view:{buildId}`)
- P1 Email: `[Reply]` (callback: `digest:mail:reply:{emailId}`) | `[Dismiss]` (callback: `digest:dismiss:{itemId}`)
- P1 Teams: `[Reply]` (callback: `digest:teams:reply:{chatId}`) | `[Dismiss]` (callback: `digest:dismiss:{itemId}`)
- P1 Obligations: `[Mark Done]` (callback: `digest:ob:done:{obId}`) | `[Snooze 24h]` (callback: `digest:ob:snooze:{obId}`)
- All items: `[Dismiss]` (callback: `digest:dismiss:{itemId}`)

Use `buildKeyboard` from `channels/telegram.ts` for keyboard construction.

Truncate to 4096 chars (Telegram limit). If truncated, append "... [N more items]" and a `[View All]` button pointing to the dashboard.

### Req-5: Scheduler -- cron-based scheduling

Create `packages/daemon/src/features/digest/scheduler.ts` with a `startDigestScheduler` function that returns a cleanup `() => void` (matching the briefing scheduler pattern).

Three independent intervals:

1. **Tier 1 thin digest**: Fires at 7am, 12pm, 5pm weekdays (Mon-Fri). Uses `setInterval` with 60s poll (same as briefing scheduler). Tracks `lastTier1Dates` map to prevent double-fire within the same hour slot.
2. **Tier 2 weekly LLM digest**: Fires Monday 9am. Uses `setInterval` with 60s poll. Tracks `lastTier2Date` to prevent double-fire.
3. **P0 real-time check**: `setInterval` every 5 minutes. Only checks PIM roles and ADO builds (not the full gather). Sends immediately on P0 detection.

**Quiet hours**: Respect the `quietStart`/`quietEnd` from `DigestConfig` (default 22:00-07:00). P0 real-time checks bypass quiet hours (urgent items always send). Tier 1 and Tier 2 digests skip during quiet hours.

**Weekend handling**: Tier 1 thin digest does not fire on Saturday/Sunday (day 0 and 6). Tier 2 fires Monday only. P0 real-time runs 7 days a week.

The scheduler accepts `DigestSchedulerDeps`: `{ pool: Pool; logger: Logger; telegram: TelegramAdapter | null; telegramChatId: string | null; config: Config }`.

### Req-6: Real-time P0 check

Create `packages/daemon/src/features/digest/realtime.ts` with the logic for the 5-minute P0 check loop.

This module only calls two fleet endpoints:
- `fleetGet(4107, "/pim/roles")` -- check for roles expiring within 2 hours
- `fleetGet(4107, "/ado/builds")` -- check for production pipeline failures

Applies the same suppression logic (30-minute cooldown for P0 hashes via `_digest_meta`).

On P0 detection: immediately sends a Telegram message with the P0 item formatted as a standalone urgent notification (not the full digest template). Uses inline keyboard for the specific action.

### Req-7: Tier 2 weekly LLM synthesis

The weekly Tier 2 digest aggregates the past week's Tier 1 digest data (gathered items, classification counts, action rates) and passes it to the Agent SDK `query()` function for trend analysis.

This runs inside the scheduler on Monday 9am. It:
1. Reads the `_digest_meta` memory topic for `weeklyStats` (accumulated from each Tier 1 run).
2. Builds a prompt with: item counts by source/priority over the week, top recurring items, items that were dismissed vs acted on.
3. Calls `query()` with a system prompt asking for a 200-word weekly trend summary with 3 suggested focus areas.
4. Sends the result via Telegram with a `[View Weekly Report]` dashboard link.
5. Resets the `weeklyStats` accumulator in `_digest_meta`.

Cost: ~$0.03 per run (one Agent SDK call with ~2K tokens input).

### Req-8: DigestConfig type and TOML section

Add `DigestConfig` interface to `packages/daemon/src/config.ts`:

```typescript
export interface DigestConfig {
  enabled: boolean;           // default: true
  quietStart: string;         // default: "22:00"
  quietEnd: string;           // default: "07:00"
  tier1Hours: number[];       // default: [7, 12, 17]
  tier2Day: number;           // default: 1 (Monday, 0=Sunday)
  tier2Hour: number;          // default: 9
  realtimeIntervalMs: number; // default: 300_000 (5 min)
  p0CooldownMs: number;       // default: 1_800_000 (30 min)
  p1CooldownMs: number;       // default: 14_400_000 (4 hours)
  p2CooldownMs: number;       // default: 43_200_000 (12 hours)
  hashTtlMs: number;          // default: 172_800_000 (48 hours)
}
```

Parse from `[digest]` section in `config/nv.toml`:

```toml
[digest]
enabled = true
quiet_start = "22:00"
quiet_end = "07:00"
tier1_hours = [7, 12, 17]
tier2_day = 1
tier2_hour = 9
realtime_interval_ms = 300000
```

Add `digest: DigestConfig` to the `Config` interface. Wire parsing in `loadConfig()` with defaults.

### Req-9: Wire scheduler into daemon index.ts

In `packages/daemon/src/index.ts`:
1. Import `startDigestScheduler` from `features/digest/index.ts`.
2. After the briefing scheduler block, conditionally start the digest scheduler if `config.digest.enabled` and `telegram` is available.
3. Pass `DigestSchedulerDeps` with pool, logger, telegram, telegramChatId, config.
4. Store the cleanup function and call it during graceful shutdown.

### Req-10: Telegram /digest command handler

Create `packages/daemon/src/telegram/commands/digest.ts`:

- `/digest` -- trigger an immediate Tier 1 thin digest (same as the scheduled 7am/noon/5pm run)
- `/digest weekly` -- trigger an immediate Tier 2 weekly LLM digest

Register the command in the Telegram adapter's command routing (same pattern as `/brief`).

### Req-11: Barrel export

Create `packages/daemon/src/features/digest/index.ts` that re-exports `startDigestScheduler` and any types needed by `index.ts` and `http.ts`.

## Scope

**IN**: Gather module (6 fleet sources + DB obligations), classify module (rule-based P0/P1/P2/suppress), suppress module (content hash dedup, cooldown, nothing-changed), format module (Telegram markdown + inline keyboards), scheduler (Tier 1 3x/day, Tier 2 weekly, P0 real-time 5min), DigestConfig + TOML section, daemon wiring, /digest Telegram command.

**OUT**: Dashboard UI for digest history (follow-up spec), digest callback handlers for inline keyboard actions (follow-up -- for now callbacks log and acknowledge), email/Teams reply actions (requires OAuth flow), configurable classification rules via API, digest analytics/metrics, notification preferences UI.

## Impact

| Area | Change |
|------|--------|
| `packages/daemon/src/features/digest/gather.ts` | NEW -- fleet HTTP calls + DB obligation query |
| `packages/daemon/src/features/digest/classify.ts` | NEW -- rule-based P0/P1/P2/suppress heuristics |
| `packages/daemon/src/features/digest/suppress.ts` | NEW -- content hash dedup, cooldown, state in `_digest_meta` |
| `packages/daemon/src/features/digest/format.ts` | NEW -- Telegram markdown template + inline keyboards |
| `packages/daemon/src/features/digest/scheduler.ts` | NEW -- cron-based Tier 1/Tier 2/P0 scheduler |
| `packages/daemon/src/features/digest/realtime.ts` | NEW -- P0-only 5-minute check loop |
| `packages/daemon/src/features/digest/index.ts` | NEW -- barrel export |
| `packages/daemon/src/config.ts` | MODIFY -- add `DigestConfig` type, `digest` field to `Config`, TOML parsing |
| `packages/daemon/src/index.ts` | MODIFY -- wire digest scheduler, add to graceful shutdown |
| `config/nv.toml` | MODIFY -- add `[digest]` section |
| `packages/daemon/src/telegram/commands/digest.ts` | NEW -- /digest command handler |

## Risks

| Risk | Mitigation |
|------|-----------|
| Fleet services may be down at digest time | Each source is independent via `Promise.allSettled`. Missing sources show "unavailable" in the digest footer. Digest still sends with available data. |
| `_digest_meta` memory topic grows unbounded | Hash cleanup on every run prunes entries older than 48 hours. JSON blob stays under 10KB even with hundreds of hashes. |
| P0 real-time loop adds load to graph-svc | Only 2 HTTP calls every 5 minutes (PIM + ADO). Graph-svc already handles these endpoints for Telegram commands with no issues. |
| Telegram rate limiting from frequent P0 alerts | 30-minute P0 cooldown limits to max 2 messages/hour per P0 item. Telegram bot API allows 30 messages/second -- not a concern. |
| Agent SDK cost for Tier 2 weekly | ~$0.03/week ($1.56/year). One call with constrained input (~2K tokens). Acceptable. |
| Quiet hours bypass for P0 could be disruptive | P0 items (PIM expiry, prod failure) are genuinely urgent and warrant overnight notification. This is intentional. |
| Content hash collision (SHA-256) | Astronomically unlikely. Hash includes source + title + detail, so even similar items from different sources produce different hashes. |
| Inline keyboard callbacks not yet handled | Out of scope. Callbacks will be logged and acknowledged with "Action noted" until a follow-up spec implements handlers. Documented in Scope > OUT. |
