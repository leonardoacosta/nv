# gather-all Specification

## Purpose
The Nova daemon currently requires 11+ sequential MCP tool calls (~45 seconds) to assemble a cross-source status picture. `gather_all` is a unified source aggregator that fans out to every data source concurrently, collects results into a unified typed payload, and returns it in one call. The same module powers both on-demand tool calls from the Nova agent and the cron-triggered briefing pipeline gather phase.

Implementation lives in `packages/daemon/src/features/gather/` with modules `sources.ts`, `fan-out.ts`, `types.ts`, and `index.ts`.

## Requirements
### Requirement: Typed per-source response envelope

Every source adapter MUST return a `SourceResult<T>` envelope: `{ status: 'ok' | 'timeout' | 'error' | 'unavailable', data: T, latencyMs: number }`. The `data` field holds the source-specific typed payload when `status` is `'ok'`; it is `null` for all other statuses. `latencyMs` records wall-clock time from fan-out dispatch to settlement. Zod schemas for `SourceResult` and all per-source payloads SHALL be defined in `types.ts`.

#### Scenario: Successful source returns ok envelope

Given the Outlook source adapter resolves with inbox and unread data within the timeout,
when the fan-out collects its result,
then the envelope has `status: 'ok'`, `data` is the typed Outlook payload containing `inbox` and `unread` arrays, and `latencyMs` is a positive number.

#### Scenario: Timed-out source returns timeout envelope

Given the Sentry source adapter does not resolve within the configured timeout,
when `Promise.allSettled` settles,
then the envelope has `status: 'timeout'`, `data` is `null`, and `latencyMs` is approximately equal to the timeout value.

#### Scenario: Errored source returns error envelope

Given the ADO source adapter rejects with a network error,
when the fan-out collects its result,
then the envelope has `status: 'error'`, `data` is `null`, and `latencyMs` reflects the time until rejection.

#### Scenario: Unavailable source returns unavailable envelope

Given the Discord source is not configured (missing credentials or disabled),
when the fan-out evaluates the source registry,
then the envelope has `status: 'unavailable'`, `data` is `null`, and `latencyMs` is `0`.

### Requirement: Source registry with typed interfaces

`sources.ts` SHALL export a `SourceRegistry` that maps source names to adapter functions. Each adapter is typed as `() => Promise<T>` where `T` is the source-specific payload. The registry MUST include adapters for: ADO (builds, PRs, pipelines), Outlook (inbox, unread), Calendar (today, upcoming), Teams (channels, chats), and Discord (messages across servers). Future sources (Sentry, Vercel, PostHog, Docker) SHALL be represented as registry entries that return `'unavailable'` until implemented.

#### Scenario: All current sources registered

Given the source registry is initialized,
when its keys are enumerated,
then the set includes `ado`, `outlook`, `calendar`, `teams`, and `discord`.

#### Scenario: Future source stubs present

Given the source registry is initialized,
when the `sentry` adapter is invoked,
then it returns a `SourceResult` with `status: 'unavailable'` without making any network call.

#### Scenario: Each adapter is independently typed

Given the `ado` adapter resolves successfully,
when its `data` field is accessed,
then it conforms to the `AdoPayload` Zod schema containing `builds`, `pullRequests`, and `pipelines` arrays.

### Requirement: Fan-out with Promise.allSettled-based concurrent execution

`fan-out.ts` SHALL export a `gatherAll(options?: GatherOptions): Promise<GatherResult>` function that invokes all selected source adapters concurrently using `Promise.allSettled`. Each adapter invocation MUST be wrapped with `AbortSignal.timeout` (or equivalent) set to the per-source timeout (default 10 seconds). A slow or failing source MUST NOT block any other source from returning.

#### Scenario: All sources resolve concurrently

Given five sources each taking 200ms,
when `gatherAll()` is called,
then total wall-clock time is approximately 200ms (not 1000ms) and all five envelopes have `status: 'ok'`.

#### Scenario: One source times out without blocking others

Given Sentry takes 15 seconds and Outlook takes 100ms with a 10-second per-source timeout,
when `gatherAll()` is called,
then the Outlook envelope arrives with `status: 'ok'` and the Sentry envelope arrives with `status: 'timeout'`, and total wall-clock time is approximately 10 seconds.

#### Scenario: One source errors without blocking others

Given the Teams adapter throws an exception at 50ms and Calendar resolves at 100ms,
when `gatherAll()` is called,
then the Teams envelope has `status: 'error'` and the Calendar envelope has `status: 'ok'`.

### Requirement: Source health metadata in response

The `GatherResult` payload MUST include a top-level `sources` array where each entry contains `{ name: string, status: 'ok' | 'timeout' | 'error' | 'unavailable', latencyMs: number }`. This provides a quick summary of which sources responded, timed out, or errored without requiring inspection of individual data payloads.

#### Scenario: Health summary reflects mixed results

Given ADO returns ok, Sentry times out, and Discord is unavailable,
when the `GatherResult.sources` array is inspected,
then it contains entries `{ name: 'ado', status: 'ok', latencyMs: <number> }`, `{ name: 'sentry', status: 'timeout', latencyMs: <number> }`, and `{ name: 'discord', status: 'unavailable', latencyMs: 0 }`.

#### Scenario: All sources healthy

Given all configured sources respond within the timeout,
when the `GatherResult.sources` array is inspected,
then every entry has `status: 'ok'` and `latencyMs` is a positive number.

### Requirement: Configurable source selection via include/exclude

`GatherOptions` SHALL accept optional `include` and `exclude` string arrays. When `include` is provided, only the listed sources are invoked. When `exclude` is provided, all sources except the listed ones are invoked. If both are provided, `include` takes precedence and `exclude` is ignored. When neither is provided, all registered sources are invoked.

#### Scenario: Include limits to specified sources

Given `include: ['outlook', 'calendar']`,
when `gatherAll({ include: ['outlook', 'calendar'] })` is called,
then only the Outlook and Calendar adapters are invoked and the result contains exactly two source entries.

#### Scenario: Exclude removes specified sources

Given `exclude: ['discord', 'sentry']`,
when `gatherAll({ exclude: ['discord', 'sentry'] })` is called,
then all sources except Discord and Sentry are invoked.

#### Scenario: Include takes precedence over exclude

Given `include: ['ado']` and `exclude: ['ado']`,
when `gatherAll({ include: ['ado'], exclude: ['ado'] })` is called,
then the ADO adapter is invoked (include wins).

#### Scenario: Neither include nor exclude invokes all sources

Given no `include` or `exclude` is specified,
when `gatherAll()` is called,
then all registered sources are invoked.

### Requirement: Hono HTTP endpoint

`index.ts` SHALL register a `GET /api/gather` route on the Hono app. The route accepts an optional `sources` query parameter as a comma-separated list of source names which maps to the `include` option. When `sources` is omitted, all sources are gathered. The response MUST be JSON with content-type `application/json` and the body is the full `GatherResult` payload.

#### Scenario: Full gather with no query param

Given a GET request to `/api/gather` with no query parameters,
when the handler executes,
then `gatherAll()` is called with no include/exclude and the response is a 200 JSON body containing all source results.

#### Scenario: Filtered gather via sources query param

Given a GET request to `/api/gather?sources=ado,outlook,calendar`,
when the handler executes,
then `gatherAll({ include: ['ado', 'outlook', 'calendar'] })` is called and the response contains exactly three source entries.

#### Scenario: Invalid source name ignored gracefully

Given a GET request to `/api/gather?sources=ado,nonexistent`,
when the handler executes,
then `ado` is gathered normally and `nonexistent` appears in the sources array with `status: 'unavailable'`.

### Requirement: MCP tool registration

`index.ts` SHALL register a `gather_all` MCP tool so the Nova agent can invoke it directly. The tool accepts an optional `sources` parameter (array of strings) mapping to the `include` option. The tool returns the full `GatherResult` payload serialized as JSON text.

#### Scenario: Agent calls gather_all with no parameters

Given the Nova agent invokes the `gather_all` tool with no arguments,
when the tool handler executes,
then all sources are gathered and the tool returns a JSON string containing the full `GatherResult`.

#### Scenario: Agent calls gather_all with source filter

Given the Nova agent invokes `gather_all` with `{ sources: ['outlook', 'calendar'] }`,
when the tool handler executes,
then only Outlook and Calendar are gathered and the result contains exactly two source entries.

### Requirement: Dual-use architecture for on-demand and briefing pipeline

The `gatherAll` function SHALL be the single entry point used by both the MCP tool / HTTP endpoint (on-demand) and the cron-triggered briefing pipeline (scheduled). The function MUST NOT depend on HTTP request context or MCP tool context; it operates purely on `GatherOptions` and returns `GatherResult`. This ensures the briefing pipeline can call `gatherAll()` directly without going through HTTP or MCP layers.

#### Scenario: Briefing pipeline calls gatherAll directly

Given the briefing cron job triggers,
when it calls `gatherAll()` directly (not via HTTP or MCP),
then it receives a valid `GatherResult` identical in shape to what the HTTP and MCP paths return.

#### Scenario: No request context dependency

Given `gatherAll` is called without any Hono `Context` or MCP `ToolCall` object,
when the function executes,
then it resolves successfully with a `GatherResult` (no implicit dependency on request context).
