# Proposal: Fix Silent Errors in Obligation Detector

## Change ID
`fix-silent-errors`

## Summary
Add error logging to the obligation detector's silent catch block, consolidate the duplicated `isQuietHours()` utility into a shared module, and flag the raw Anthropic SDK usage in `detector.ts` for replacement by `extract-agent-query-factory`.

## Context
- Depends on: none
- Conflicts with: `extract-agent-query-factory` (that spec will replace the raw `Anthropic` client in `detector.ts`; this spec adds a `// TODO` flag only — no structural change to the client)
- Files affected:
  - `packages/daemon/src/features/obligations/detector.ts`
  - `packages/daemon/src/features/watcher/proactive.ts`
  - `packages/daemon/src/features/digest/scheduler.ts`
  - `packages/daemon/src/lib/quiet-hours.ts` (new)

## Motivation

**Problem 1 — Silent failure in obligation detection.** `detectObligations()` at `detector.ts:172-175` wraps the entire API call in a bare `catch {}` that returns `[]` with no logging. If the Haiku call fails for any persistent reason — wrong gateway key, rate limit, network partition — the obligation pipeline silently produces no results on every invocation. The daemon logs give no signal that anything is wrong. This is an observability gap: errors that affect a core feature are completely invisible.

**Problem 2 — Raw Anthropic SDK in the wrong place.** `detector.ts` uses `new Anthropic(...)` and calls `anthropic.messages.create()` directly. Every other LLM call in the daemon goes through the Agent SDK's `query()`. This is a different code path, a different error model, and a different authentication flow. It predates the Agent SDK adoption and should be consolidated, but the full migration is deferred to `extract-agent-query-factory`. This spec adds a clearly marked `// TODO(extract-agent-query-factory)` comment so the divergence is visible in code review.

**Problem 3 — Duplicate `isQuietHours()`.** The function is implemented twice with incompatible signatures:
- `proactive.ts:40-58` — `isQuietHours(now: Date, config: ProactiveWatcherConfig): boolean`
- `scheduler.ts:25-46` — `isQuietHours(quietStart: string, quietEnd: string): boolean` (uses `new Date()` internally)

Both implement midnight-wrap logic. The scheduler variant omits the `now` parameter (hardcoding `new Date()` internally), making it untestable in isolation. A shared `isQuietHours(now: Date, quietStart: string, quietEnd: string): boolean` replaces both.

## Requirements

### Req-1: Log errors in `detectObligations()`
The `catch` block at `detector.ts:172-175` MUST log via a pino logger before returning `[]`. The logger instance MUST be accepted as an optional parameter (`logger?: Logger`) so callers that have a logger can pass it through; when omitted a no-op fallback is used. The log level MUST be `warn` (not `error`) because returning `[]` is a graceful degradation, not a crash.

### Req-2: Add a `TODO` comment for the raw SDK usage
Immediately above the `createClient()` function in `detector.ts`, add:
```
// TODO(extract-agent-query-factory): Replace with createAgentQuery() once that spec lands.
// This file uses the raw Anthropic SDK — different auth flow from the rest of the daemon.
```
No functional change to the client code.

### Req-3: Extract `isQuietHours()` to `packages/daemon/src/lib/quiet-hours.ts`
Create `packages/daemon/src/lib/quiet-hours.ts` exporting a single function:
```typescript
export function isQuietHours(now: Date, quietStart: string, quietEnd: string): boolean
```
The implementation MUST handle: normal same-day windows, midnight-wrap windows, and the zero-length window (start === end → always false). Delete the private `isQuietHours` from both `proactive.ts` and `scheduler.ts` and replace call sites with imports from the new module.

### Req-4: TypeScript must pass
`pnpm tsc --noEmit` in `packages/daemon/` MUST produce zero errors after all changes.

## Scope
- **IN**: Error logging in `detector.ts`, `TODO` comment for SDK flag, shared `quiet-hours.ts`, call-site updates in `proactive.ts` and `scheduler.ts`
- **OUT**: Replacing the raw Anthropic SDK with `createAgentQuery()` (deferred to `extract-agent-query-factory`), changes to obligation store, executor, or any file outside `packages/daemon/src/`

## Impact
| File | Change |
|------|--------|
| `packages/daemon/src/lib/quiet-hours.ts` | NEW — exports `isQuietHours(now, quietStart, quietEnd)` |
| `packages/daemon/src/features/obligations/detector.ts` | Add `logger?: Logger` param, log warn on catch, add TODO comment |
| `packages/daemon/src/features/watcher/proactive.ts` | Delete local `isQuietHours`, import from `lib/quiet-hours.ts`, update call site |
| `packages/daemon/src/features/digest/scheduler.ts` | Delete local `isQuietHours`, import from `lib/quiet-hours.ts`, update 2 call sites |

## Risks
| Risk | Mitigation |
|------|-----------|
| Changing `detectObligations` signature breaks callers | Add `logger` as an optional last parameter with a no-op default — all existing callers work unchanged |
| `scheduler.ts` `isQuietHours` hardcodes `new Date()` — shared version takes `now` | Update the two call sites in `scheduler.ts` to pass `new Date()` explicitly; behaviour is identical |
| `proactive.ts` passes `config` object — shared version takes raw strings | Update the one call site to destructure `config.quietStart` and `config.quietEnd`; behaviour is identical |
