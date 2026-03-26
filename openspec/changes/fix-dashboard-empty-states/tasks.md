# Implementation Tasks

<!-- beads:epic:nv-0i7d -->

## UI Batch

- [ ] [3.1] [P-1] Fix `apps/dashboard/app/page.tsx` — ensure `setSummary` always executes after `fetchData` completes: move the `setSummary(...)` call out of any conditional so it always fires in the `try` block even when obligations/projects/sessions/health fetches all return non-ok, guaranteeing `loading` transitions to `false` with zero-value stat cards rather than eternal skeletons [owner:ui-engineer]
- [ ] [3.2] [P-1] Fix `apps/dashboard/app/contacts/page.tsx` — in `fetchContacts`, replace `if (!res.ok) throw new Error(...)` with a status-aware branch: HTTP 503 → `setContacts([])` + `setError(null)` (treat as empty list); any other non-ok status → throw error string as before [owner:ui-engineer]
- [ ] [3.3] [P-1] Fix `apps/dashboard/components/SessionWidget.tsx` — add `setLoading(false)` to the `if (!res.ok) return` early-return branch so the loading skeleton always clears even when `/api/session/status` returns a non-200 response [owner:ui-engineer]
