# Proposal: Deduplicate Shared Utils

## Change ID
`deduplicate-shared-utils`

## Summary

Remove three instances of duplicated code in `packages/daemon`: consolidate the `TELEGRAM_MAX_LEN` constant and truncation logic into a single shared utility, delete the dead `processMessage()` method on `NovaAgent`, and remove the `dotenv` import that conflicts with the Doppler-managed environment.

## Context
- `packages/daemon/src/channels/stream-writer.ts:8` — `MAX_LEN = 4096` (local name), plus `splitMessage()` helper using newline-aware splitting
- `packages/daemon/src/features/digest/format.ts:7` — `TELEGRAM_MAX_LEN = 4096`, inline truncation in `formatDigest()` (suffix `\n... [N more items]`) and `formatWeeklySynthesis()` (suffix `\n... [truncated]`)
- `packages/daemon/src/features/briefing/runner.ts:13` — `TELEGRAM_MAX_LEN = 4096`, local `truncateForTelegram()` helper with dashboard link suffix
- `packages/daemon/src/brain/agent.ts:90–187` — `processMessage()`: 97-line non-streaming method; only caller reference is a stale comment in `dream/scheduler.ts:48`; all actual call sites use `processMessageStream()`
- `packages/daemon/src/config.ts:5` — `import "dotenv/config"`; env vars are managed by Doppler; `dotenv` listed in `packages/daemon/package.json` as a runtime dependency

## Motivation

Each of the three `TELEGRAM_MAX_LEN` definitions has diverged: `stream-writer.ts` uses a newline-aware `splitMessage()` that produces clean chunk boundaries; `digest/format.ts` has two ad-hoc inline truncations with different suffixes; `briefing/runner.ts` has a private `truncateForTelegram()` that truncates to a single message. Any future change to the limit or split strategy must be made in three places, and the behaviours are already inconsistent. A single `packages/daemon/src/utils/telegram.ts` module eliminates this.

`processMessage()` was the original blocking implementation before streaming was added. It is no longer called anywhere in the codebase (confirmed: only references are the method definition itself and a stale JSDoc comment). Keeping it creates confusion about which path is authoritative and adds ~100 lines of dead code that must be maintained through future refactors.

The `dotenv` import in `config.ts` is actively harmful: in development it silently loads a `.env` file that may shadow Doppler-injected values; in production it is a no-op dependency that adds to startup cost. Doppler handles all secrets; `dotenv` should be removed from both the import and `package.json`.

## Requirements

### Req-1: Create `packages/daemon/src/utils/telegram.ts`

Create a new utility module that exports:

1. `TELEGRAM_MAX_LEN: 4096` — the canonical constant
2. `splitForTelegram(text: string): string[]` — newline-aware chunker extracted verbatim from `splitMessage()` in `stream-writer.ts`

The `splitForTelegram` signature accepts `text: string` and returns `string[]`. Internal logic: while `remaining.length > TELEGRAM_MAX_LEN`, find the last `\n` before the limit; if no good newline exists (found position < 50% of limit), hard-split at the limit; push the chunk; strip the leading newline from `remaining`. Return the final remainder as the last chunk.

### Req-2: Migrate callers to the shared utility

Update the three existing call sites:

**`channels/stream-writer.ts`**
- Remove `const MAX_LEN = 4096`
- Import `TELEGRAM_MAX_LEN` and `splitForTelegram` from `../utils/telegram.js`
- Replace the local `splitMessage()` function with a call to `splitForTelegram`
- Update the `flush()` truncation guard to use `TELEGRAM_MAX_LEN`

**`features/digest/format.ts`**
- Remove `const TELEGRAM_MAX_LEN = 4096`
- Import `TELEGRAM_MAX_LEN` and `splitForTelegram` from `../../utils/telegram.js`
- In `formatDigest()`: replace the inline truncation block with `splitForTelegram(text)[0]` plus the existing `\n... [N more items]` suffix logic (the digest always fits in one Telegram message; the split is a safety truncation)
- In `formatWeeklySynthesis()`: replace the inline truncation with `splitForTelegram(text)[0]` plus the `\n... [truncated]` suffix

**`features/briefing/runner.ts`**
- Remove `const TELEGRAM_MAX_LEN = 4096` and the local `truncateForTelegram()` function
- Import `splitForTelegram` from `../../utils/telegram.js`
- Replace the `truncateForTelegram(content)` call with `splitForTelegram(content)[0]` plus the existing `DASHBOARD_SUFFIX` append (maintain current single-message truncation behaviour)

### Req-3: Remove dead `processMessage()` from `NovaAgent`

In `packages/daemon/src/brain/agent.ts`:
- Delete the `processMessage()` method (lines 90–187)
- Update the JSDoc comment on `processMessageStream()` to remove the reference to `processMessage()` as the non-streaming variant
- Update the stale JSDoc comment in `packages/daemon/src/features/dream/scheduler.ts:48` that references `NovaAgent.processMessage()` — change to reference `processMessageStream()`

No callers exist outside `agent.ts` itself, so no other files require updates.

### Req-4: Remove `dotenv` from `config.ts` and `package.json`

In `packages/daemon/src/config.ts`:
- Remove `import "dotenv/config"` (line 5)

In `packages/daemon/package.json`:
- Remove `"dotenv": "^16.4.5"` from the `dependencies` block

## Scope

**IN**: New `utils/telegram.ts` module, migration of three call sites, deletion of `processMessage()`, removal of `dotenv` import and dependency.

**OUT**: Changes to truncation suffix strings or split behaviour (callers keep their existing suffixes), changes to other files outside the four listed, test file additions (out of scope for a cleanup spec).

## Impact

| File | Change |
|------|--------|
| `packages/daemon/src/utils/telegram.ts` | New file — `TELEGRAM_MAX_LEN` constant + `splitForTelegram()` |
| `packages/daemon/src/channels/stream-writer.ts` | Remove local `MAX_LEN`, import shared constant + helper, remove `splitMessage()` |
| `packages/daemon/src/features/digest/format.ts` | Remove local `TELEGRAM_MAX_LEN`, import shared constant, replace inline truncation |
| `packages/daemon/src/features/briefing/runner.ts` | Remove local constant + `truncateForTelegram()`, import `splitForTelegram` |
| `packages/daemon/src/brain/agent.ts` | Delete `processMessage()` method (lines 90–187) |
| `packages/daemon/src/features/dream/scheduler.ts` | Update stale JSDoc comment |
| `packages/daemon/src/config.ts` | Remove `import "dotenv/config"` |
| `packages/daemon/package.json` | Remove `dotenv` dependency |

## Risks

| Risk | Mitigation |
|------|-----------|
| `splitForTelegram` behaviour differs from per-site truncation | `stream-writer.ts` already uses the newline-aware split; `digest` and `briefing` used simple `slice` truncations — both produce shorter output, so callers are strictly better. Suffix strings are preserved at each call site. |
| Removing `dotenv` breaks local development | Local dev uses Doppler (`doppler run -- pnpm dev`). Any developer running without Doppler already needed to set `DATABASE_URL` and other vars manually; `dotenv` was not the intended mechanism for that. |
| `processMessage()` called via dynamic dispatch or reflection | `grep` across the entire repo shows zero runtime call sites. The only non-definition reference is a stale JSDoc in `dream/scheduler.ts`. |
