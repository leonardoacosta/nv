# Tasks: fix-dashboard-contracts

<!-- beads:epic:TBD -->

## Batch: UI

- [ ] `[ui-engineer]` Fix MemoryPage PUT body: rename `path` to `topic` in `handleSave` `JSON.stringify` call (line 59) — `dashboard/src/pages/MemoryPage.tsx`
- [ ] `[ui-engineer]` Fix MemoryPage GET: add `topics` array branch — when `raw.topics` is a string array, map each topic string to a `MemoryFile` with `name` and `path` set to the topic — `dashboard/src/pages/MemoryPage.tsx`
- [ ] `[ui-engineer]` Fix SettingsPage PUT body: wrap config in `{ fields: config }` before `JSON.stringify` (line 213) — `dashboard/src/pages/SettingsPage.tsx`
- [ ] `[ui-engineer]` Fix DashboardPage projects count: extract `response.projects` array before calling `.length` (line 89) — `dashboard/src/pages/DashboardPage.tsx`
- [ ] `[ui-engineer]` Fix DashboardPage sessions: extract `response.sessions` array from the `{ sessions: [...] }` wrapper before filtering and reducing (line 92) — `dashboard/src/pages/DashboardPage.tsx`
- [ ] `[ui-engineer]` Fix IntegrationsPage PUT body: replace `{ integration_id, config }` with `{ fields: config }` in `handleSave` (line 53) — `dashboard/src/pages/IntegrationsPage.tsx`
- [ ] `[ui-engineer]` Fix NexusPage health fetch: extract `data.latest` for metrics and map `data.status` ("healthy"/"degraded"/"critical") to the `HealthMetrics` status union ("ok"/"degraded"/"down") — `dashboard/src/pages/NexusPage.tsx`
- [ ] `[ui-engineer]` Fix UsagePage data source: change fetch from `/api/sessions` to `/stats`, read `response.tool_usage.per_tool`, and map `{ tool_name, count }` entries to the page's `ToolUsage[]` type — `dashboard/src/pages/UsagePage.tsx`
- [ ] `[ui-engineer]` Expand `ServerHealth` component `HealthMetrics.status` union to include backend values: `"ok" | "healthy" | "degraded" | "critical" | "down"` — `dashboard/src/components/ServerHealth.tsx`
- [ ] `[ui-engineer]` Create `dashboard/src/types/api.ts` with canonical TypeScript response types: `MemoryGetResponse`, `ProjectsGetResponse`, `SessionsGetResponse`, `ServerHealthGetResponse`, `StatsGetResponse`, `PutMemoryRequest`, `PutConfigRequest` — `dashboard/src/types/api.ts`
- [ ] `[ui-engineer]` Replace all inline `as SomeType` response casts in the six page files with imports from `dashboard/src/types/api.ts` — `dashboard/src/pages/`
