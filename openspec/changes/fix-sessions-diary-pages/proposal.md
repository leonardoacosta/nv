# Proposal: fix-sessions-diary-pages

## Problem

Two dashboard pages have broken API integrations that prevent them from rendering data.

### /sessions — "Failed to load sessions" error

The sessions page (`apps/dashboard/app/sessions/page.tsx`) calls `/api/sessions`, which proxies to
the daemon at `daemon_url/api/sessions`. **That route does not exist on the daemon.** The daemon
only exposes `/api/cc-sessions` (for CC subprocess sessions managed by `CcSessionManager`). The
real session data lives in `TeamAgentDispatcher::list_agents()` but is never exposed over HTTP.

Additional issue: `/app/nexus/page.tsx` still exists as a standalone page alongside `/sessions`.
These pages overlap in purpose; `/nexus` should be removed or redirected.

### /diary — HTTP 404 on load

The diary page (`apps/dashboard/app/diary/page.tsx`) calls `/api/diary?date=...`, but **there is no
Next.js proxy route at `apps/dashboard/app/api/diary/route.ts`**. The daemon does expose
`GET /api/diary` (registered at line 149 of `http.rs`), and the component already has the correct
shape assumptions (`DiaryGetResponse`, `DiaryEntryItem`) based on `types/api.ts`. The fix is purely
adding the missing proxy route.

The diary page itself already has date navigation arrows, a summary bar, and entry rendering — these
are implemented and correct. They simply fail to render because the fetch returns 404.

## Root Causes

| Page | Root Cause |
|------|------------|
| /sessions | Daemon missing `GET /api/sessions` handler; `TeamAgentDispatcher::list_agents()` not wired to HTTP |
| /diary | Next.js proxy route `apps/dashboard/app/api/diary/route.ts` does not exist |
| /sessions | `/app/nexus/page.tsx` still exists alongside `/sessions` (duplicate) |

## Solution

### 1. Add daemon `GET /api/sessions` handler

In `crates/nv-daemon/src/http.rs`:

- Register `.route("/api/sessions", get(get_sessions_handler))` in `build_router`
- Implement `get_sessions_handler`: call `state.dispatcher.list_agents().await`, serialize each
  `SessionSummary` to match the `SessionsGetResponse` shape the frontend expects:
  ```json
  {
    "sessions": [
      {
        "id": "...",
        "project": "...",
        "status": "active|idle|completed",
        "agent_name": "...",
        "started_at": "2026-03-25T...",
        "duration_display": "5m",
        "branch": null,
        "spec": null,
        "progress": null
      }
    ]
  }
  ```
- The `SessionSummary` struct in `team_agent/types.rs` already has all required fields except
  `progress`. The handler should serialize with `progress: null` for now (progress tracking is a
  separate concern).
- `HttpState` must expose the `TeamAgentDispatcher` — check whether it is already accessible; if
  not, add it.

### 2. Add `GET /api/diary` Next.js proxy route

Create `apps/dashboard/app/api/diary/route.ts`:

```typescript
import { NextRequest, NextResponse } from "next/server";
import { DAEMON_URL } from "@/lib/daemon";

export async function GET(req: NextRequest) {
  try {
    const { searchParams } = new URL(req.url);
    const params = new URLSearchParams();
    if (searchParams.get("date")) params.set("date", searchParams.get("date")!);
    if (searchParams.get("limit")) params.set("limit", searchParams.get("limit")!);
    const url = new URL(`/api/diary?${params.toString()}`, DAEMON_URL);
    const res = await fetch(url.toString());
    const data = await res.json();
    return NextResponse.json(data, { status: res.status });
  } catch {
    return NextResponse.json({ error: "Daemon unreachable" }, { status: 502 });
  }
}
```

### 3. Remove /nexus page

Delete `apps/dashboard/app/nexus/page.tsx` and its directory. Update the sidebar navigation to
remove any link to `/nexus` (sidebar is at `apps/dashboard/components/Sidebar.tsx`). Add a Next.js
redirect in `next.config.ts` or a `redirect()` in the deleted route's place if any external links
need to be preserved.

## Out of Scope

- Progress tracking for sessions (`progress_pct`, `phase_label`) — the field is already `null`-safe
  in the frontend; daemon-side progress emission is a separate feature.
- Daemon-side `HttpState` restructuring beyond wiring the dispatcher for sessions.
- Pagination for diary or sessions — the current per-page limits are acceptable.

## Dependencies

- Depends on: **fix-dashboard-api-proxy** (must land first to ensure DAEMON_URL and proxy pattern
  are stable).

## Acceptance Criteria

1. `GET /api/sessions` returns `{ sessions: [...] }` with the correct shape from the daemon.
2. `/sessions` page loads without error banner; Active/Idle/Completed sections render with real data
   when sessions exist; empty state renders when no sessions are running.
3. `GET /api/diary` proxy route returns daemon data with correct query param forwarding.
4. `/diary` page loads without error banner; entries render for today; date navigation prev/next
   changes the displayed date and re-fetches; summary bar shows correct counts.
5. `/nexus` route no longer renders a duplicate sessions page (deleted or redirected).
6. TypeScript compiles clean (`pnpm typecheck`) after changes.
