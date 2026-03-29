# shared-utils Specification

## Purpose
TBD - created by archiving change deduplicate-shared-utils. Update Purpose after archive.
## Requirements
### Requirement: Canonical telegram utility module replaces three duplicate constants

`packages/daemon/src/utils/telegram.ts` MUST be created exporting `TELEGRAM_MAX_LEN: 4096` and `splitForTelegram(text: string): string[]`. The `splitForTelegram` function MUST implement newline-aware chunking: while the remaining text exceeds `TELEGRAM_MAX_LEN`, find the last `\n` before the limit; if no suitable newline exists (found position less than 50% of limit), hard-split at the limit; push the chunk and strip the leading newline from the remainder. The local `MAX_LEN` / `TELEGRAM_MAX_LEN` constants in `stream-writer.ts`, `digest/format.ts`, and `briefing/runner.ts` MUST be deleted and replaced with imports from `utils/telegram.ts`.

#### Scenario: Newline-aware split produces clean chunk boundaries

Given a text string of 5000 characters with a `\n` at position 4000,
when `splitForTelegram(text)` is called,
then the first chunk ends at the newline boundary (at most 4096 chars) and the second chunk contains the remainder.

#### Scenario: Hard split when no suitable newline exists

Given a text string of 5000 characters with no `\n` in the first 4096 characters,
when `splitForTelegram(text)` is called,
then the first chunk is exactly 4096 characters (hard split at the limit).

### Requirement: Dead code and conflicting dependency removed

The `processMessage()` method (lines 90–187) in `packages/daemon/src/brain/agent.ts` MUST be deleted — it has zero runtime call sites and its presence creates ambiguity about which processing path is authoritative. The stale JSDoc in `packages/daemon/src/features/dream/scheduler.ts` that references `NovaAgent.processMessage()` MUST be updated to reference `processMessageStream()`. The `import "dotenv/config"` line in `packages/daemon/src/config.ts` MUST be removed and `"dotenv"` MUST be removed from `packages/daemon/package.json` dependencies, as environment variables are managed exclusively by Doppler.

#### Scenario: Daemon starts without dotenv loaded

Given `dotenv` is removed from `config.ts` and `package.json`,
when the daemon starts with Doppler-injected environment variables,
then all required env vars are available and the daemon initialises without errors.

