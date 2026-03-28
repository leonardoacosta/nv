# Implementation Tasks

<!-- beads:epic:nv-zcm7 -->

## DB Batch

(no database changes required)

## API Batch

- [ ] [2.1] [P-1] Capture config resolution sources in daemon `loadConfig()` -- build a parallel `sources: Map<string, ConfigSource>` tracking whether each key resolved from env var, TOML file, or hardcoded default; expose via `GET /config/sources` on daemon HTTP server (port 7700) returning `ConfigSource[]` with `{ key, source, envVar? }` [owner:api-engineer] [beads:nv-i1o1]
- [x] [2.2] [P-1] Add `system.configSources` tRPC procedure in `packages/api/src/routers/system.ts` -- proxy `GET /config/sources` from daemon via `fleetFetch("daemon", "/config/sources")`, return typed `ConfigSource[]` to the dashboard [owner:api-engineer] [beads:nv-xsl5]
- [x] [2.3] [P-1] Add `system.channelStatus` tRPC procedure -- call channels-svc `/channels` for live connection status, enrich each channel with `identity` from new `/channels/status` endpoint and `lastMessageAt` via `SELECT MAX(created_at) FROM messages WHERE channel = $1`; fall back to static registry on error [owner:api-engineer] [beads:nv-f63g]
- [x] [2.4] [P-1] Add `system.testChannel` mutation (POST to channels-svc `/send` with test payload) and `system.testIntegration` mutation (server-side validation request per service: Anthropic `/v1/models`, OpenAI `/v1/models`, ElevenLabs `/v1/voices`, GitHub `/user`, Sentry `/api/0/`, PostHog health check) returning `{ valid, error?, latencyMs }` [owner:api-engineer] [beads:nv-hj2d]
- [x] [2.5] [P-2] Add `system.memorySummary` tRPC procedure -- return `{ count, topics: string[], lastWriteAt, totalSizeBytes }` by querying `COUNT(*)`, topic names, `MAX(updated_at)`, and `SUM(LENGTH(content))` from memory table [owner:api-engineer] [beads:nv-4g13]
- [ ] [2.6] [P-2] Add `identity()` method to `ChannelAdapter` interface in channels-svc `adapters/registry.ts` returning `{ username?, displayName? }`; implement in Telegram (bot username via `getMe`), Discord (bot user tag), and Teams (app name) adapters; expose via `GET /channels/status` endpoint in channels-svc server [owner:api-engineer] [beads:nv-x7oz]

## UI Batch

- [x] [3.1] [P-1] Create `FIELD_REGISTRY` in `apps/dashboard/app/settings/lib/field-registry.ts` -- static `Record<string, FieldMeta>` mapping each config key to `{ description, placeholder?, validation?, default?, unit?, restartRequired?, group? }` for all known daemon config keys (~80 entries from daemon Config/TomlConfig interfaces) [owner:ui-engineer] [beads:nv-chrc]
- [x] [3.2] [P-1] Extract `FieldRow` component to `apps/dashboard/app/settings/components/FieldRow.tsx` -- display description below label in muted text, unit suffix badge after input, default value as placeholder, patternHint as helper text below input on right side [owner:ui-engineer] [beads:nv-hqgj]
- [x] [3.3] [P-1] Add inline validation to FieldRow -- validate on blur and on save using registry constraints (min/max for numbers via HTML attributes + JS, regex pattern for strings); show error inline in red text below input; block Save button when any field has errors, show error count in SaveRestartBar [owner:ui-engineer] [beads:nv-gdvy]
- [x] [3.4] [P-1] Create `ConfigSourceBadge` component in `apps/dashboard/app/settings/components/ConfigSourceBadge.tsx` -- render colored badge next to field label: blue `ENV` (with env var name on hover tooltip), gray `FILE`, dim `DEFAULT`; disable input when source is `env` [owner:ui-engineer] [beads:nv-z33o]
- [x] [3.5] [P-1] Create `SecretField` component in `apps/dashboard/app/settings/components/SecretField.tsx` -- green/red presence dot, last-4-character display when set (e.g. `--------k7Qx`), eye icon reveal toggle that shows full value for 5 seconds then re-masks [owner:ui-engineer] [beads:nv-zdg7]
- [x] [3.6] [P-1] Create `ChannelStatusCard` in `apps/dashboard/app/settings/components/ChannelStatusCard.tsx` -- per-channel card with brand icon, connection status dot (green/yellow/red), bot identity line, last message relative time, "Test Connection" button with spinner/check/X feedback [owner:ui-engineer] [beads:nv-wuus]
- [x] [3.7] [P-1] Create `IntegrationStatusCard` in `apps/dashboard/app/settings/components/IntegrationStatusCard.tsx` -- per-service card with brand icon, key validity badge (valid/expired/missing/unknown), last successful call timestamp from diary, "Test Connection" button with latency result [owner:ui-engineer] [beads:nv-r3eq]
- [x] [3.8] [P-2] Create `MemorySummaryCard` in `apps/dashboard/app/settings/components/MemorySummaryCard.tsx` -- entry count, horizontal topic chip/tag list (clickable to `/memory?topic=<name>`), last write relative time, total size in human-readable format, "View all topics" link to `/memory` [owner:ui-engineer] [beads:nv-ain7]
- [x] [3.9] [P-2] Create `SubGroup` component in `apps/dashboard/app/settings/components/SubGroup.tsx` -- collapsible container with chevron toggle, indented header with hairline separator, localStorage persistence for collapsed state; default to expanded [owner:ui-engineer] [beads:nv-w6yg]
- [x] [3.10] [P-1] Wire settings page (`apps/dashboard/app/settings/page.tsx`) to use new components -- integrate FieldRow with registry lookups, render ChannelStatusCard above channel config fields, IntegrationStatusCard above integration fields, MemorySummaryCard above memory fields, group fields into SubGroups per registry `group` field, fetch configSources/channelStatus/memorySummary queries [owner:ui-engineer] [beads:nv-4cla]

## E2E Batch

- [x] [4.1] [P-2] Verify field descriptions, unit suffixes, and validation errors render correctly -- field with description shows muted text below label, number field with min/max shows error on out-of-range blur, pattern field shows patternHint error on invalid input, Save button disabled when errors exist [owner:e2e-engineer] [beads:nv-1jgz]
- [x] [4.2] [P-2] Verify channel and integration status cards render with test connection flow -- channel card shows connection dot and bot identity, "Test Connection" triggers spinner then result feedback, integration card shows key status badge [owner:e2e-engineer] [beads:nv-vzhd]
- [x] [4.3] [P-2] Verify config source badges, secret reveal toggle, and memory summary card -- ENV badge appears on env-overridden fields with disabled input, secret field shows last-4 chars and reveal toggles for 5s, memory summary shows topic count and clickable chips [owner:e2e-engineer] [beads:nv-llp1]
