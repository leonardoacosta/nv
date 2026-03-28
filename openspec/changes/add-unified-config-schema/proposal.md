# Proposal: Add Unified Config Schema

## Change ID
`add-unified-config-schema`

## Summary

Create a single Zod-validated configuration schema with clear precedence rules. Currently, configuration lives across 4 sources: TOML file (`nv.toml`), environment variables, DB settings table, and in-memory overrides -- with no validation, no schema enforcement, and inconsistent precedence.

## Context

- TOML config loaded in `packages/daemon/src/config.ts` from `~/.nv/config/nv.toml`
- Env vars override specific TOML values (`TELEGRAM_BOT_TOKEN`, `DATABASE_URL`, etc.)
- DB settings table stores runtime overrides (added by `add-persistent-watcher-config`)
- In-memory overrides for transient state
- No Zod schema validates the merged config
- Missing required values cause runtime errors, not startup failures
- Config shape is implicit (TypeScript interface with optional fields)

## Motivation

Config errors are the #1 cause of silent failures in Nova. A missing `TELEGRAM_BOT_TOKEN` doesn't fail at startup -- it fails when the first Telegram message arrives, 7 hours after deploy. Zod validation at startup catches all config issues immediately.

## Requirements

### Req-1: Zod config schema

Create `packages/daemon/src/config/schema.ts`:

- Define complete config schema with Zod:
  - `daemon`: `{ port, logLevel, toolRouterUrl }` -- `toolRouterUrl` validated as URL
  - `agent`: `{ model, maxTurns, systemPromptPath }`
  - `telegram`: `{ botToken, chatId }` -- optional section (entire section or nothing)
  - `discord`: `{ botToken }` -- optional section
  - `teams`: `{ webhookUrl }` -- optional section
  - `digest`: `{ enabled, quietStart, quietEnd, tier1Hours, cooldowns }` -- `quietStart`/`quietEnd` validated as HH:MM format, `tier1Hours` as array of 0-23
  - `autonomy`: `{ enabled, timeoutMs, cooldownHours, dailyBudgetUsd }`
  - `queue`: `{ concurrency, maxQueueSize }` -- `concurrency` constrained to 1-32, `maxQueueSize` constrained to 1-1000
  - `database`: `{ url }` -- required
- Use Zod transforms for type coercion (string port to number)
- Use Zod refinements for cross-field validation (`quietStart` < `quietEnd`)

### Req-2: Precedence resolution

Create `packages/daemon/src/config/resolver.ts`:

- Precedence order (highest wins): env vars > DB settings > TOML file > defaults
- Load and merge in order: defaults -> TOML -> DB -> env vars
- Validate merged result with Zod schema
- On validation failure: log all errors, throw with formatted message listing every issue

### Req-3: Startup validation

In daemon startup:

- Replace current config loading with schema-validated resolver
- If config invalid: print errors to stderr, exit with code 1
- If config valid: log "Configuration loaded" with source breakdown (which values came from where)
- Log WARN for any deprecated config keys (future migration aid)

### Req-4: Config introspection

Add tRPC procedure `system.configSources`:

- Returns: for each config key, the source (`toml`, `env`, `db`, `default`) and whether it was validated
- Sensitive values (tokens, secrets) redacted to `***set***` or `***unset***`
- Enables dashboard settings page to show where each value comes from

## Scope

- **IN**: `packages/daemon/src/config/` (new directory structure), `packages/daemon/src/index.ts` (startup), `packages/api/src/routers/system.ts` (`configSources` procedure)
- **OUT**: Individual feature configs (unchanged shape, just validated now), dashboard settings UI (unchanged)

## Impact

| Area | Change |
|------|--------|
| `packages/daemon/src/config/schema.ts` | New -- Zod schema |
| `packages/daemon/src/config/resolver.ts` | New -- precedence resolution |
| `packages/daemon/src/config.ts` | Rewrite -- delegates to schema + resolver |
| `packages/daemon/src/index.ts` | Modified -- fail-fast on invalid config |
| `packages/api/src/routers/system.ts` | Extended -- `configSources` procedure with redaction |

## Risks

| Risk | Mitigation |
|------|-----------|
| Existing config breaks validation | Run schema against current `nv.toml` first; fix any issues before deploying |
| Too strict: blocks startup for optional features | Optional sections are entirely optional; only required fields within a section are enforced |
| Performance: DB query + TOML parse + env read at startup | One-time cost (<100ms total), not hot path |
