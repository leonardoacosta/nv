# Proposal: Fix Dashboard Empty States

## Change ID
`fix-dashboard-empty-states`

## Summary
Four dashboard UI issues remain after proxy routing is resolved: stat cards show eternal skeletons
instead of zero-value content, the contacts page stays in perpetual loading when the contact store
is unconfigured (503 response treated as error rather than empty), the settings page shows "No
fields configured" for all sections because the config endpoint returns `{}`, and the home page
"CC Session" section shows a loading skeleton when no CC session process is running.

## Context
- Extends: `apps/dashboard/app/page.tsx` (stat cards + SessionWidget), `apps/dashboard/app/contacts/page.tsx` (loading state), `apps/dashboard/app/settings/page.tsx` (empty config handling), `apps/dashboard/components/SessionWidget.tsx` (stopped state rendering)
- Depends on: `fix-daemon-url-port`, `fix-stubbed-proxy-routes` (proxies must be reachable before empty-state fixes apply)
- Related: `fix-dashboard-api-proxy` (completed — contacts proxy now routes correctly), `fix-dashboard-content-rendering` (completed — settings config endpoint added)

## Motivation
After fixing routing issues, several pages will still show degraded states because they lack proper
transitions from loading to empty or from error to "not configured." The result is four separate
user-visible regressions:

1. **Stat cards**: `loading` state is driven by a single `setLoading(false)` call at the end of
   `fetchData`. Because `sessions`, `projects`, and `server-health` proxies are 501 stubs, all four
   `Promise.allSettled` branches may resolve without setting `summary` to a non-null value.
   `summary` stays `null` → the ternary keeps rendering the skeleton grid. Fix: initialize
   `summary` eagerly to zero values so it is always non-null after `fetchData` completes.

2. **Contacts perpetual loading**: When `contact_store` is not configured the daemon returns
   `HTTP 503`. The page fetches `/api/contacts` → proxy mirrors the 503 → `if (!res.ok)` throws →
   `setError(...)` is called, but `setLoading(false)` still fires in `finally`. The `loading` flag
   transitions correctly to false, so the skeleton does go away — the actual symptom is an error
   banner appearing for what is an expected "empty" condition on a fresh install. The fix is to
   treat a 503 from the contacts endpoint as an empty list rather than an error, and show the empty
   state instead of the error banner.

3. **Settings "No fields configured"**: Settings already handles `{}` config correctly — the
   `ConfigSection` component shows "No fields configured." for sections with no matching keys.
   This is by design and is acceptable empty-state behaviour. The underlying issue for users is
   that the proxy was a 501 stub. Once `fix-stubbed-proxy-routes` wires the real `/api/config`
   route, the settings page will populate from the actual daemon config TOML. No code change is
   needed in the settings page itself — the spec documents the expected populated behaviour and
   adds a loading skeleton that covers the 4-section card frame (already present).

4. **SessionWidget eternal skeleton**: `SessionWidget` fetches `/api/session/status` from
   `session-manager` — a local server-side singleton, not the daemon. When the CC container is
   stopped (common on a fresh install), `getStatus()` returns `{ state: "stopped", ... }`. The
   component already renders a `StateBadge` for `state === "stopped"` correctly. The skeleton only
   shows during the initial `loading` fetch. The issue is the fetch sets `loading = false` only in
   `finally` — which it already does. The skeleton duration is acceptable. However, if the
   `/api/session/status` route call itself fails (network error, SSR guard), `loading` never
   transitions to false because the `.ok` check returns without setting `loading`. Fix: ensure
   `setLoading(false)` is always reached regardless of response status.

## Requirements

### Req-1: Stat cards — always render values after first fetch completes

`DashboardPage.fetchData` must initialize `summary` to zero values immediately after all
`Promise.allSettled` calls complete, regardless of which individual fetches succeed. The stat cards
must never remain as skeletons once `loading` is false.

Fix: Replace the conditional `setSummary(...)` call with an unconditional one that always sets
`summary` to computed values (defaulting to `0` for any failed branch):

```
summary = {
  obligations_count: oblList.length,         // already defaults to [] on failure
  active_sessions: sessData.filter(active).length,
  idle_sessions: sessData.filter(idle).length,
  projects_count: projectsCount ?? 0,        // already defaults to 0 on failure
  messages_today: 0,
  tools_today: 0,
  cost_today_usd: 0,
}
```

The current code already does this (lines 340–348 of `page.tsx`). The root cause is that the
`loading` flag starts as `true` and the stat card grid renders the skeleton when `loading === true`
(line 440). If `fetchData` is never called, or if an exception is thrown before `setSummary`, the
cards stay as skeletons. The fix ensures `loading` transitions to `false` even when the outer
`try/catch` catches a top-level error before reaching `setSummary`. Move `setSummary` outside the
inner conditional blocks so it always runs in the `try` block.

#### Scenario: stat cards render zeros on empty daemon

Given the daemon is running but has no obligations or sessions,
when the home page loads and all proxies return `{ obligations: [], sessions: [], projects: [] }`,
then the six stat cards show `0`, `0`, `0`, `"ok"`, `"—"`, `"—"` values, not skeletons.

#### Scenario: stat cards render zeros when proxies return 501

Given the sessions and projects proxies return HTTP 501 (not implemented),
when the home page loads,
then `loading` transitions to `false` after `fetchData` completes, stat cards show `0` for counts
and `"—"` for health/cpu/memory (no daemon health data), not eternal skeletons.

### Req-2: Contacts — treat 503 as empty list, not error

`ContactsPage.fetchContacts` currently throws on any non-ok status. When the daemon's contact store
is not configured (returns 503), this surfaces as an error banner. On a fresh install, "contact
store not configured" is an expected operational state, not a user error.

Fix: In the `fetchContacts` catch block, check for `HTTP 503` specifically. If the response status
is 503, call `setContacts([])` and `setError(null)` rather than setting an error string. The empty
state ("No contacts yet") will then display correctly.

Additionally, the `if (!res.ok)` throw at line 482 should be replaced with a status-aware branch:
- `503` → treat as empty (`setContacts([])`, no error)
- Any other non-ok → throw error as today

#### Scenario: contacts empty state on fresh install

Given the daemon's contact store is not configured and `/api/contacts` returns HTTP 503,
when the contacts page loads,
then the "No contacts yet" empty state is shown — not an error banner and not a loading skeleton.

#### Scenario: contacts error state on real API failure

Given the daemon is unreachable and `/api/contacts` returns HTTP 502,
then the error banner appears as before.

### Req-3: Settings — document expected populated behaviour (no code change)

Once `fix-stubbed-proxy-routes` is applied and `GET /api/config` routes to the real daemon config
endpoint, the settings page will populate field rows from the TOML config. The existing
`assignFieldsToSections` logic maps top-level TOML keys to sections:

- `daemon`, `server`, `port`, `host`, `log_level` → Daemon section
- `telegram`, `discord`, `teams`, `channels` → Channels section
- `anthropic`, `github`, `stripe`, `sentry`, `posthog`, `integrations` → Integrations section
- `memory`, `context`, `db`, `storage` → Memory section
- Any unmatched key falls back to Daemon section

No dashboard code change is needed. The existing empty-section "No fields configured" placeholder
is the correct behaviour when the config has no keys for a given section. This requirement is
met by the existing code.

#### Scenario: settings populated from daemon config

Given the daemon config contains `{ "anthropic": { "api_key": "sk-..." }, "telegram": { "token": "..." } }`,
when SettingsPage loads,
then the Integrations section shows "Api Key" (masked as `••••••••••••`) and "Token" (masked),
the Channels section shows "Token" (masked), and Daemon and Memory sections show
"No fields configured."

### Req-4: SessionWidget — ensure loading state clears on any fetch outcome

`SessionWidget.fetchStatus` currently only sets `loading = false` in the `finally` block of a
try/catch, which means it always runs. However, the `if (!res.ok) return` guard at line 81 exits
without setting `loading = false` — this is the bug. If `/api/session/status` returns a non-200
response (e.g. 500 from an SSR guard during cold boot), `loading` stays `true` and the skeleton
never disappears.

Fix: Move `setLoading(false)` out of `finally` into an unconditional call at the end of
`fetchStatus`, or change the early return to still call `setLoading(false)` before returning:

```typescript
const fetchStatus = async () => {
  try {
    const res = await fetch("/api/session/status");
    if (!res.ok) {
      setLoading(false);   // <-- add this
      return;
    }
    const data = (await res.json()) as SessionStatus;
    setStatus(data);
  } catch {
    // Silently ignore
  } finally {
    setLoading(false);
  }
};
```

When no session is active, the component renders the `StateBadge` with `state = "stopped"` and
shows the Restart button (disabled, since `canRestart` is false for stopped). This is the correct
non-skeleton empty state.

#### Scenario: SessionWidget shows stopped state after fetch

Given `/api/session/status` returns `{ state: "stopped", message_count: 0, restart_count: 0 }`,
when SessionWidget mounts and the fetch completes,
then the skeleton disappears and the stopped badge is rendered with 0 messages.

#### Scenario: SessionWidget shows stopped state on non-200 response

Given `/api/session/status` returns HTTP 500 (SSR guard or cold boot issue),
when SessionWidget mounts and the fetch completes,
then `loading` transitions to `false`, the component renders with `state = "stopped"` (default),
not an eternal skeleton.

## Scope
- **IN**: Fix `loading` state transition in stat cards (`page.tsx`), treat 503 as empty in contacts (`contacts/page.tsx`), fix `setLoading(false)` coverage in `SessionWidget.tsx`
- **OUT**: Settings page code (no change needed), implementing missing daemon endpoints, redesigning stat card layout, adding new dashboard sections

## Impact
| Area | Change |
|------|--------|
| `apps/dashboard/app/page.tsx` | Guard `setSummary` call so it always executes in `try` block |
| `apps/dashboard/app/contacts/page.tsx` | Treat HTTP 503 as empty list rather than error |
| `apps/dashboard/components/SessionWidget.tsx` | Add `setLoading(false)` to the early-return branch |
| `apps/dashboard/app/settings/page.tsx` | No code change — existing "No fields configured" is correct |

## Risks
| Risk | Mitigation |
|------|-----------|
| Treating 503 as empty hides a real store misconfiguration | The empty state message "No contacts yet" is accurate for a fresh install; a separate monitoring alert can catch persistent 503s in production |
| Stat cards showing `0` when data fetch is mid-flight could mislead | Cards transition from skeleton to values only after `fetchData` fully resolves — no intermediate `0` flash |
| SessionWidget `stopped` state shows a Restart button (disabled) that may confuse users with no CC configured | Button is disabled with reduced opacity; "Manage" link to `/session` page provides context — acceptable for v1 |
