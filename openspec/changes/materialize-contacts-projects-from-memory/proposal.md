# Proposal: Materialize Contacts & Projects from Memory

## Change ID
`materialize-contacts-projects-from-memory`

## Summary

Build materialization pipelines that read Nova's memory topics and nv.toml config to upsert structured records into the `contacts` and `projects` Postgres tables. The "people" memory topic is parsed via the existing `people-parser.ts` into `PersonProfile[]` and upserted into `contacts` by matching on channel IDs. Projects are upserted from `projects-*` memory topics and the Rust daemon's `[projects]` config section (exposed via the daemon HTTP API). Both the Contacts and Projects page "Refresh" buttons are rewired to trigger materialization. A scheduled job runs materialization daily.

## Context
- Depends on: `add-memory-svc` (completed), `add-trpc-api` (completed)
- Conflicts with: none
- Current state:
  - `apps/dashboard/lib/entity-resolution/people-parser.ts` parses the "people" memory topic into `PersonProfile[]` with name, channelIds, role, notes -- but nothing writes these back to the `contacts` table
  - `packages/api/src/routers/contact.ts` line 332-398: `discovered` endpoint scans `messages` table for senders and LEFT JOINs to contacts by name -- does not read memory at all
  - `packages/api/src/routers/project.ts` line 59-97: seeds projects from `NV_PROJECTS` env var only when the table is empty; line 293-431: `extract` mutation enriches existing project rows with cross-table stats but never discovers new projects
  - Rust daemon owns `config.projects: HashMap<String, PathBuf>` loaded from `[projects]` section in `config/nv.toml` -- exposed via `GET /api/projects` on port 8400
  - Memory topics: `people` (freeform text about contacts), `projects-*` (per-project knowledge written by Nova)
  - Schema: `contacts` has `channel_ids` jsonb, `projects` has `code` text (unique)
  - Contacts page Refresh button invalidates the `discovered` query (which only scans messages)
  - Projects page Refresh button calls `extract` mutation (which only enriches existing rows, never creates new ones)

## Motivation

Nova accumulates rich knowledge about people and projects in its memory topics, but this knowledge is siloed -- it never materializes into the structured Postgres tables that power the dashboard. The result:

1. **Contacts gap**: Nova knows about people (names, channel IDs, roles) from the "people" memory topic, but the contacts page only discovers senders from raw message history. People Nova knows about but hasn't exchanged messages with are invisible.
2. **Projects gap**: Nova's nv.toml config lists 12+ managed projects with filesystem paths, and `projects-*` memory topics contain accumulated project knowledge. But the projects table only populates from a JSON env var seed, and the "Refresh" button only enriches what already exists -- it never discovers new projects.
3. **Stale data**: Even when contacts or projects exist in Postgres, they lack the enriched context (roles, notes, descriptions) that Nova has already captured in memory.

## Requirements

### Req-1: Contact Materialization Procedure

Create `packages/api/src/lib/materialize-contacts.ts`:

- Read the "people" memory topic from Postgres (`db.select().from(memory).where(eq(memory.topic, "people"))`)
- Parse via `parsePeopleMemory()` from `apps/dashboard/lib/entity-resolution/people-parser.ts` -- move this parser to `packages/api/src/lib/people-parser.ts` so it is importable from the API package without cross-referencing the dashboard app
- For each `PersonProfile`:
  - **Match by channel ID**: Query existing contacts where any value in `contacts.channel_ids` jsonb matches any value in the profile's `channelIds`. Use SQL jsonb containment or iterate existing contacts in-memory (the contacts table is small, <1000 rows)
  - **Match by name** (fallback): If no channel ID match, check for case-insensitive name match (`LOWER(contacts.name) = LOWER(profile.name)`)
  - **If matched**: Upsert -- merge `channelIds` (union of existing + parsed), update `notes` if parsed notes are non-empty and different, update `relationshipType` from parsed role if currently null
  - **If no match**: Insert new contact with `name`, `channelIds`, `notes`, `relationshipType` from role
- Return `{ created: number, updated: number, unchanged: number }`

### Req-2: Project Materialization Procedure

Create `packages/api/src/lib/materialize-projects.ts`:

- **Source 1 -- nv.toml project registry**: Call the Rust daemon's `GET http://localhost:8400/api/projects` to get `[{ code, path }]`. Fall back to parsing `NV_PROJECTS` env var if the daemon is unreachable.
- **Source 2 -- memory topics**: Query all `projects-*` memory topics (`db.select().from(memory).where(like(memory.topic, "projects-%"))`). Extract project codes from topic names (e.g., `projects-oo` -> `oo`, `projects-tribal-cities` -> `tribal-cities`).
- Merge both sources into a deduplicated list by project code.
- For each project:
  - **Match by code**: `db.select().from(projects).where(eq(projects.code, code))`
  - **If matched**: Update `path` from daemon registry if currently null, update `description` from memory topic content (first 500 chars) if currently null
  - **If no match**: Insert with `code`, `name` (same as code initially), `category: "work"`, `status: "active"`, `path` from daemon registry, `description` from memory topic content (first 500 chars)
- Return `{ created: number, updated: number, unchanged: number }`

### Req-3: Wire Contact Materialization to tRPC

Add a `materialize` mutation to `packages/api/src/routers/contact.ts`:

```typescript
materialize: protectedProcedure.mutation(async () => {
  const result = await materializeContacts();
  return result;
}),
```

### Req-4: Wire Project Materialization to tRPC

Add a `materialize` mutation to `packages/api/src/routers/project.ts`:

```typescript
materialize: protectedProcedure.mutation(async () => {
  const result = await materializeProjects();
  return result;
}),
```

### Req-5: Rewire Contacts Page Refresh Button

Modify `apps/dashboard/app/contacts/page.tsx`:

- The Refresh button currently invalidates the `discovered` query. Change it to:
  1. Call `contact.materialize` mutation first
  2. Then invalidate both `contact.discovered` and `contact.list` queries
- Show a toast with materialization results (e.g., "Synced 3 new contacts, updated 2")

### Req-6: Rewire Projects Page Refresh Button

Modify `apps/dashboard/app/projects/page.tsx`:

- The Refresh button currently calls `project.extract`. Change `handleRefresh` to:
  1. Call `project.materialize` mutation first (creates/updates from memory + daemon registry)
  2. Then call `project.extract` mutation (enriches all projects with cross-table stats)
  3. Then re-fetch the project list
- Show a toast with materialization results

### Req-7: Scheduled Materialization

Create `packages/daemon/src/features/materialize/scheduler.ts`:

- A lightweight scheduler (following the DreamScheduler pattern) that runs materialization daily at a configurable hour (default: 4 AM, after the dream cycle at 3 AM)
- Calls the tRPC API's `contact.materialize` and `project.materialize` endpoints via HTTP (the daemon already makes HTTP calls to the tool fleet services)
- Alternatively, since the daemon has `@nova/db` access, it can call the materialization functions directly (but the functions live in `packages/api` -- so prefer HTTP calls to `localhost:3000/api/trpc/contact.materialize` and `localhost:3000/api/trpc/project.materialize` to avoid duplicating logic)
- Log results to the daemon logger and optionally write a diary entry

Add config to `config/nv.toml`:

```toml
[materialize]
enabled = true
cron_hour = 4
```

### Req-8: Move People Parser to API Package

Move `apps/dashboard/lib/entity-resolution/people-parser.ts` to `packages/api/src/lib/people-parser.ts`. Update the dashboard import to reference the new location (either re-export from API package or duplicate-and-delete if the dashboard cannot import from `@nova/api`).

The parser is pure TypeScript with no external dependencies -- it is safe to move between packages.

## Scope
- **IN**: Contact materialization from "people" memory topic, project materialization from `projects-*` memory topics + daemon project registry, tRPC mutations for both, rewired Refresh buttons on contacts and projects pages, daily scheduled materialization, people-parser relocation to API package
- **OUT**: Contact materialization from message history (already handled by `discovered` endpoint), project materialization from Jira/GitHub (future), contact photo/avatar resolution, memory topic creation (memory is written by Nova during conversations), real-time materialization on memory write (future webhook), Rust daemon code changes (only HTTP calls to existing endpoints)

## Impact

| Area | Change |
|------|--------|
| `packages/api/src/lib/people-parser.ts` | NEW -- relocated from dashboard, `parsePeopleMemory()` + `PersonProfile` type |
| `packages/api/src/lib/materialize-contacts.ts` | NEW -- read "people" memory, parse, match by channel ID / name, upsert contacts |
| `packages/api/src/lib/materialize-projects.ts` | NEW -- read daemon registry + `projects-*` memory, match by code, upsert projects |
| `packages/api/src/routers/contact.ts` | MODIFY -- add `materialize` mutation |
| `packages/api/src/routers/project.ts` | MODIFY -- add `materialize` mutation |
| `apps/dashboard/app/contacts/page.tsx` | MODIFY -- rewire Refresh to call `contact.materialize` then invalidate queries |
| `apps/dashboard/app/projects/page.tsx` | MODIFY -- rewire Refresh to call `project.materialize` before `extract` |
| `apps/dashboard/lib/entity-resolution/people-parser.ts` | DELETE or MODIFY -- replace with re-export from `@nova/api` |
| `packages/daemon/src/features/materialize/scheduler.ts` | NEW -- daily cron calling materialization endpoints |
| `packages/daemon/src/features/materialize/index.ts` | NEW -- barrel export + start/stop lifecycle |
| `packages/daemon/src/config.ts` | MODIFY -- add `MaterializeConfig` interface + parse `[materialize]` from nv.toml |
| `packages/daemon/src/index.ts` | MODIFY -- start materialize scheduler |
| `config/nv.toml` | MODIFY -- add `[materialize]` section |

## Risks

| Risk | Mitigation |
|------|-----------|
| Channel ID matching produces false positives (e.g., numeric Telegram IDs collide) | Match on exact value within the `channel_ids` jsonb. Telegram IDs are 6-16 digits, Discord snowflakes are 17-20 digits -- length-based distinction already handled by the parser. For ambiguous cases, require both platform key and ID to match. |
| Memory "people" topic format changes over time | The parser uses heuristic section detection (headers, bold, caps) and degrades gracefully -- unrecognized sections are skipped, not errored. Parser already handles multiple formats. |
| Daemon HTTP API unreachable during project materialization | Fall back to `NV_PROJECTS` env var (existing pattern in `project.list`). Log a warning. The memory-based discovery still runs independently. |
| Concurrent materialization calls (Refresh button + cron) | Materialization is idempotent -- upsert by channel ID / project code. Worst case: two concurrent runs both insert the same contact, and the second hits a unique constraint. Use `onConflictDoUpdate` to handle gracefully. |
| Dashboard cannot import from `@nova/api` package directly | If workspace dependency does not allow it, copy the parser into `packages/api/src/lib/` and keep the dashboard copy as-is. Both are identical pure functions. The dashboard copy becomes dead code that can be removed in a follow-up. |
| Scheduled HTTP calls to tRPC endpoints require the dashboard to be running | The tRPC API runs inside the Next.js app. If the dashboard is down, the scheduler logs a warning and retries next cycle. Alternatively, extract materialization logic into a shared package importable by both API and daemon -- but this adds complexity. Prefer the HTTP approach for v1. |
