# Proposal: Enhance Settings Page Verbosity

## Change ID
`enhance-settings-verbosity`

## Summary

Transform the Settings page from a generic flat key-value config editor into a rich, section-aware
settings experience with field descriptions, live service status, validation constraints, config
source indicators, and "Test Connection" buttons for channels and integrations.

## Context
- Extends: `apps/dashboard/app/settings/page.tsx` (524 lines), `apps/dashboard/app/settings/components/SettingsSection.tsx`, `apps/dashboard/app/settings/components/SaveRestartBar.tsx`
- Consumes: `system.config` tRPC procedure (returns flattened env-derived config), `system.memory` (topic list + content), `system.fleetStatus` (static fleet registry), `system.stats` (entity counts)
- Consumes (fleet): channels-svc `:4103/channels` (live channel status with `connected`/`disconnected`/`error`), meta-svc `:4108/services` (fleet health probes with latency)
- Related: daemon `Config` interface in `packages/daemon/src/config.ts` (TOML + env var layering), `packages/db/src/schema/memory.ts` (topic, content, updated_at, embedding), `packages/db/src/schema/settings.ts` (key-value settings table)
- Related proposals: `redesign-integrations-status` (fleet/channel status page), `improve-system-pages` (predecessor -- added section grouping and collapsible sections)

## Motivation

The Settings page was scaffolded as a generic config editor: it flattens the daemon's TOML config
into key-value pairs, auto-generates labels from key names, and renders them as text inputs or
toggles. This was adequate for v1 but creates several UX problems:

1. **No field descriptions** -- users see `Quiet Start` with no explanation of what quiet hours
   mean, what format is expected (`HH:MM`), or what the default is. Every field requires
   consulting the TOML config file or source code.

2. **Channels section is config-only** -- shows keys like `telegram.chat_id` with masked values.
   Does not indicate whether the bot is actually connected, what username it operates as, when the
   last message was sent, or whether sending works. Users cannot verify channel health without
   checking logs.

3. **Integrations section is config-only** -- shows API key presence but not whether keys are
   valid, expired, or functional. Users have no way to verify that the Anthropic or OpenAI key
   actually works without triggering a real conversation.

4. **Memory section is config-only** -- shows config keys for memory storage settings but not the
   actual memory state: how many topics exist, when the last write happened, or which topics are
   stored. The Memory page exists separately but has no summary in Settings.

5. **No validation** -- number fields accept any string, URL fields accept random text, time fields
   (`HH:MM`) have no format enforcement. Invalid config is saved and causes daemon crashes on
   restart.

6. **No config source visibility** -- users cannot tell whether a value comes from an environment
   variable override, the TOML file, or a hardcoded default. This matters for debugging deployment
   issues where env vars silently override file config.

7. **Secrets are fully opaque** -- masked fields show `************` with no way to verify which
   key is set. Even showing the last 4 characters or a key prefix would help operators confirm
   the right credential is configured.

## Requirements

### Req-1: Field Description Registry

Create a static `FIELD_REGISTRY` mapping in `apps/dashboard/app/settings/lib/field-registry.ts`
that maps each config key path to metadata:

```typescript
interface FieldMeta {
  description: string;
  placeholder?: string;
  validation?: {
    min?: number;
    max?: number;
    pattern?: string;       // regex string
    patternHint?: string;   // human-readable format hint, e.g. "HH:MM"
  };
  default?: string | number | boolean;
  unit?: string;            // e.g. "ms", "hours", "USD"
  restartRequired?: boolean;
  group?: string;           // sub-group within section for collapsible nesting
}
```

Populate for all known config keys from the daemon `Config` interface and `TomlConfig` shape.
Example entries:

| Key | Description | Validation |
|-----|-------------|------------|
| `daemon.port` | Port the daemon HTTP server listens on | min: 1024, max: 65535 |
| `daemon.log_level` | Log verbosity level | pattern: `^(trace\|debug\|info\|warn\|error)$` |
| `agent.model` | Claude model identifier for the brain agent | -- |
| `agent.max_turns` | Maximum conversation turns per session | min: 1, max: 500 |
| `digest.quiet_start` | Start of quiet hours (no digest delivery) | pattern: `^\d{2}:\d{2}$`, hint: "HH:MM" |
| `digest.quiet_end` | End of quiet hours | pattern: `^\d{2}:\d{2}$`, hint: "HH:MM" |
| `autonomy.daily_budget_usd` | Maximum daily API spend for autonomous actions | min: 0, max: 100, unit: "USD" |
| `autonomy.timeout_ms` | Timeout for autonomous action execution | min: 1000, unit: "ms" |
| `proactive_watcher.interval_minutes` | How often the watcher scans for stale obligations | min: 1, max: 1440, unit: "min" |
| `queue.concurrency` | Maximum parallel job queue workers | min: 1, max: 10 |
| `queue.max_queue_size` | Maximum pending jobs before rejecting new work | min: 1, max: 100 |
| `conversation.history_depth` | Number of past messages included in agent context | min: 1, max: 100 |

Fields not in the registry render with their existing auto-generated label and no description
(graceful fallback -- no regressions for unknown keys).

### Req-2: Field Description UI

Extend the `FieldRow` component to show the description text below the label in `text-copy-12
text-ds-gray-700`. If a unit is defined, show it as a suffix badge after the input
(e.g., `[300000] ms`). If a default value is defined, show it as placeholder text in the input.
If validation is defined, show the `patternHint` as helper text below the input on the right side.

Layout change per row:
```
[ Label                              ] [ Input _______ ] [unit]
  Description text in muted gray         HH:MM format
```

### Req-3: Inline Validation

When a field has validation constraints in the registry, validate on blur and on save. Show
validation errors inline below the input in `text-copy-12 text-red-700`. Block save if any
field has a validation error (disable Save button, show error count in the save bar).

Number fields: enforce `min`/`max` via HTML `min`/`max` attributes and JS validation.
Pattern fields: test against regex on blur, show `patternHint` as the error message.

### Req-4: Config Source Indicator

Add a new tRPC procedure `system.configSources` that returns the resolved source for each config
key. The daemon already layers env vars over TOML over defaults in `loadConfig()`. Extend the
config endpoint (or add a companion endpoint) to return a source map:

```typescript
interface ConfigSource {
  key: string;
  source: "env" | "file" | "default";
  envVar?: string;       // e.g. "NV_DAEMON_PORT" -- only when source is "env"
}
```

Display as a small colored badge next to the field label:
- `ENV` badge (blue) -- value comes from environment variable, show the env var name on hover
- `FILE` badge (gray) -- value comes from TOML config file
- `DEFAULT` badge (dim) -- using the hardcoded default
- Fields overridden by env vars should have their inputs disabled (env vars take precedence
  and cannot be changed from the UI)

Implementation: the daemon's `loadConfig()` already checks env vars first, then TOML, then
defaults. Capture the resolution order into a parallel `sources` map and expose it via a new
`/config/sources` HTTP endpoint on the daemon (port 7700), then proxy through a tRPC procedure.

### Req-5: Secret Field Enhancement

For secret/masked fields, change the display from `************` to show:
- A presence indicator: green dot = set, red dot = not set
- Last 4 characters when set (e.g., `••••••••k7Qx`), matching the common pattern for API keys
- A "Reveal" toggle button (eye icon) that shows the full value for 5 seconds, then re-masks.
  The reveal uses the existing config data (already transmitted to the client); no additional
  API call needed.

### Req-6: Channel Settings Enhancement

Replace the generic key-value display for channel-related config keys with a purpose-built
Channel Status card per configured channel. Each card shows:

- **Channel name** (Telegram, Discord, Teams, Email, iMessage) with the channel's brand icon
- **Connection status** dot (green = connected, yellow = error, red = disconnected) --
  sourced from channels-svc `/channels` endpoint via the existing `system.fleetStatus` tRPC
  procedure (extend it to call channels-svc live instead of returning static data)
- **Bot identity** -- for Telegram: bot username; for Discord: bot user tag; for Teams: app name.
  Source from channels-svc adapter metadata (requires adding an `identity()` method to the
  `ChannelAdapter` interface that returns `{ username?: string; displayName?: string }`)
- **Last message time** -- query `messages` table for the most recent message per channel:
  `SELECT MAX(created_at) FROM messages WHERE channel = $1`
- **"Test Connection" button** -- sends a POST to channels-svc `/send` with a test payload
  (`channel: "telegram", target: <configured chat id>, message: "Nova connection test"`).
  Shows a spinner during the request, then green check or red X with error message for 5 seconds.

Channel config keys (e.g., `telegram.chat_id`) still appear as editable fields below the status
card, but the card provides the at-a-glance health view.

New tRPC procedures needed:
- `system.channelStatus` -- calls channels-svc `/channels` and enriches with last message
  timestamps from the DB
- `system.testChannel` -- mutation that calls channels-svc `/send` with a test message

### Req-7: Integration Settings Enhancement

Replace the generic key-value display for integration-related config keys with Integration Status
cards. Each card shows:

- **Service name** (Anthropic, OpenAI, ElevenLabs, GitHub, Sentry, PostHog) with brand icon
- **API key status** badge:
  - `valid` (green) -- key is set and last API call succeeded
  - `expired` (amber) -- key is set but last API call returned 401/403
  - `missing` (red) -- key is not configured
  - `unknown` (gray) -- key is set but has never been tested
- **Last successful call** -- timestamp of the last successful tool invocation for tools
  associated with this integration. Source: query `diary` table for the most recent entry
  with a matching tool slug (e.g., diary entries with `tools_called` containing `anthropic_*`
  patterns)
- **"Test Connection" button** -- fires a lightweight validation request per service:
  - Anthropic: `GET /v1/models` with the API key
  - OpenAI: `GET /v1/models` with the API key
  - ElevenLabs: `GET /v1/voices` with the API key
  - GitHub: `GET /user` with the token
  - Sentry: `GET /api/0/` with the auth token
  - PostHog: health check endpoint with project key

New tRPC procedure:
- `system.testIntegration` -- mutation accepting service name, dispatches the appropriate
  validation request and returns `{ valid: boolean; error?: string; latencyMs: number }`

The test requests run server-side (tRPC procedure in `@nova/api`) to avoid exposing API keys
to the browser. Keys are read from the daemon config (env vars or TOML).

### Req-8: Memory Settings Enhancement

Replace the generic key-value display for memory-related config keys with a Memory Summary card
at the top of the Memory section. The card shows:

- **Entry count** -- total number of memory topics (from `system.stats` counts.memory or a
  `SELECT count(*) FROM memory` query)
- **Topic list** -- horizontal chip/tag list showing all topic names (from `system.memory`
  with no topic param). Clicking a topic navigates to `/memory?topic=<name>`.
- **Last write timestamp** -- `SELECT MAX(updated_at) FROM memory`, displayed as relative time
  (e.g., "2 hours ago")
- **Total size** -- `SELECT SUM(LENGTH(content)) FROM memory`, displayed in human-readable
  format (e.g., "142 KB")
- **Link to memory viewer** -- "View all topics" link that navigates to `/memory`

Memory config keys (e.g., `dream.enabled`, `dream.cron_hour`) still appear as editable fields
below the summary card.

New tRPC procedure:
- `system.memorySummary` -- returns `{ count: number; topics: string[]; lastWriteAt: string | null; totalSizeBytes: number }`

### Req-9: Collapsible Sub-Groups

Within each section, related fields should be grouped into collapsible sub-groups using the
`group` field from the registry. For example, in the Daemon section:

- **Core** -- `daemon.port`, `daemon.log_level`
- **Agent** -- `agent.model`, `agent.max_turns`
- **Proactive Watcher** -- `proactive_watcher.enabled`, `proactive_watcher.interval_minutes`, etc.
- **Digest** -- `digest.enabled`, `digest.quiet_start`, `digest.quiet_end`, etc.
- **Dream** -- `dream.enabled`, `dream.cron_hour`, etc.
- **Queue** -- `queue.concurrency`, `queue.max_queue_size`
- **Autonomy** -- `autonomy.enabled`, `autonomy.daily_budget_usd`, etc.

Sub-groups use the same collapsible pattern as top-level sections (chevron toggle, localStorage
persistence) but with reduced visual weight: no card border, just an indented header with a
hairline separator.

Fields without a `group` assignment appear at the top of the section, ungrouped.

## Scope
- **IN**: Field description registry, inline validation, config source indicators, secret
  reveal/last-4, channel status cards with test connection, integration status cards with test
  connection, memory summary card, collapsible sub-groups, new tRPC procedures
  (`system.configSources`, `system.channelStatus`, `system.testChannel`,
  `system.testIntegration`, `system.memorySummary`)
- **OUT**: Editing the TOML file from the UI (settings page writes to the daemon config API,
  not the file system), channel management (adding/removing channels), integration OAuth flows,
  memory CRUD from settings (use the dedicated Memory page), real-time WebSocket status updates
  (polling is sufficient), config versioning/history

## Impact
| Area | Change |
|------|--------|
| `apps/dashboard/app/settings/page.tsx` | Major refactor: section-specific rendering (status cards + field rows), validation state, source badges |
| `apps/dashboard/app/settings/lib/field-registry.ts` | New: static field metadata registry (~80 entries) |
| `apps/dashboard/app/settings/components/FieldRow.tsx` | New: extracted from page.tsx, extended with description, validation, source badge, unit suffix |
| `apps/dashboard/app/settings/components/ChannelStatusCard.tsx` | New: per-channel health card with identity, last message, test button |
| `apps/dashboard/app/settings/components/IntegrationStatusCard.tsx` | New: per-integration status card with key validity, last call, test button |
| `apps/dashboard/app/settings/components/MemorySummaryCard.tsx` | New: topic count, chip list, last write, size, link |
| `apps/dashboard/app/settings/components/SubGroup.tsx` | New: collapsible sub-group container (lightweight SettingsSection variant) |
| `apps/dashboard/app/settings/components/SecretField.tsx` | New: enhanced secret display with reveal toggle and last-4 chars |
| `apps/dashboard/app/settings/components/SettingsSection.tsx` | Modified: accept optional header slot for status cards |
| `packages/api/src/routers/system.ts` | Extended: add `configSources`, `channelStatus`, `testChannel`, `testIntegration`, `memorySummary` procedures |
| `packages/tools/channels-svc/src/adapters/registry.ts` | Modified: add `identity()` method to `ChannelAdapter` interface |
| `packages/tools/channels-svc/src/adapters/telegram.ts` | Modified: implement `identity()` returning bot username |
| `packages/tools/channels-svc/src/adapters/discord.ts` | Modified: implement `identity()` returning bot user tag |
| `packages/tools/channels-svc/src/adapters/teams.ts` | Modified: implement `identity()` returning app name |
| `packages/tools/channels-svc/src/server.ts` | Modified: add `/channels/status` endpoint returning enriched status with identity |
| `packages/daemon/src/http.ts` | Modified: add `GET /config/sources` endpoint |
| `packages/daemon/src/config.ts` | Modified: capture resolution sources during `loadConfig()` |
| `apps/dashboard/types/api.ts` | Extended: new response types for config sources, channel status, integration test, memory summary |

## Risks
| Risk | Mitigation |
|------|-----------|
| Test Connection buttons cause side effects (e.g., Telegram test message visible to users) | Use a distinctive message format: "[Nova] Connection test at {timestamp}" so it's clearly identifiable; consider a `--dry-run` mode for channels that support it |
| Integration test requests expose API keys in transit | All test requests run server-side via tRPC mutations; keys never reach the browser |
| Config source endpoint exposes env var names | Env var names (not values) are non-sensitive metadata; the actual secret values remain masked |
| Field registry becomes stale as daemon config evolves | Registry falls back gracefully -- unregistered fields render with auto-generated labels and no description; add a CI check that compares registry keys against `TomlConfig` interface |
| Validation blocks save for legacy configs with invalid values | Show validation errors as warnings (amber) on initial load, only block save for newly-introduced errors; existing values pass through unchanged |
| channels-svc `/channels` endpoint unavailable (service down) | Channel status cards show "unreachable" state with gray dot; config fields below remain editable |
| Collapsible sub-groups add visual complexity | Default sub-groups to expanded; only collapse when the user explicitly collapses them; persist state in localStorage |
