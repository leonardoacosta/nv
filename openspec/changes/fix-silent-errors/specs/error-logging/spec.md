# Error Logging and Shared Quiet Hours

## ADDED Requirements

### Requirement: detectObligations logs errors instead of swallowing them

The `catch` block in `detectObligations()` in `packages/daemon/src/features/obligations/detector.ts` MUST log via a pino `Logger` before returning `[]`. The logger MUST be accepted as an optional parameter (`logger?: Logger`) with a no-op fallback so all existing callers remain valid without changes. The log level MUST be `warn` because returning `[]` is a graceful degradation. Immediately above the `createClient()` function, a `// TODO(extract-agent-query-factory)` comment SHALL be added identifying the raw Anthropic SDK usage for future replacement.

#### Scenario: API failure is logged at warn level

Given `detectObligations()` is called with a logger and the Anthropic API call throws a network error,
when the catch block executes,
then a `warn` log entry is emitted containing the error details and `[]` is returned to the caller.

#### Scenario: Existing callers work without a logger argument

Given a call site that invokes `detectObligations(messages, config)` without passing a logger,
when an API error occurs,
then the function returns `[]` silently using the no-op fallback and no runtime error is thrown.

### Requirement: isQuietHours extracted to a shared module used by all callers

`packages/daemon/src/lib/quiet-hours.ts` MUST be created exporting `isQuietHours(now: Date, quietStart: string, quietEnd: string): boolean`. The implementation MUST handle normal same-day windows, midnight-wrap windows, and the zero-length window (`quietStart === quietEnd` returns `false`). The private `isQuietHours` function MUST be deleted from both `proactive.ts` and `scheduler.ts`, and all call sites in those files MUST be updated to import from `lib/quiet-hours.ts`. After all changes, `pnpm tsc --noEmit` in `packages/daemon/` SHALL produce zero errors.

#### Scenario: Midnight-wrap window is handled correctly

Given `quietStart = "23:00"` and `quietEnd = "06:00"` and `now` is 02:00,
when `isQuietHours(now, quietStart, quietEnd)` is called,
then it returns `true` (the time falls within the wrapped quiet window).

#### Scenario: Zero-length window is never quiet

Given `quietStart = "08:00"` and `quietEnd = "08:00"`,
when `isQuietHours(now, quietStart, quietEnd)` is called for any `now`,
then it returns `false`.
