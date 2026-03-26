# Proposal: Add Schedule Service

## Change ID
`add-schedule-svc`

## Summary

Build the schedule tool service at `packages/tools/schedule-svc/` on port 4006. Implements 9 tools
across 3 domains (reminders, schedules, sessions) as a Hono+MCP microservice backed by the shared
Postgres instance via `@nova/db`. Follows the `scaffold-tool-service` template pattern.

## Context

- Phase: 4 -- Automation Tools | Wave: 5
- Feature area: tools
- Roadmap: `docs/plan/nova-v10/wave-plan.json`
- Depends on: `scaffold-tool-service` (Wave 1 -- provides the service template), `add-schedule-tables`
  (Wave 1 -- provides the reminders, schedules, sessions Drizzle schemas in `@nova/db`)
- Depended on by: nothing downstream in v10
- Extends: `packages/tools/service-template/` (copy-and-customize pattern)
- Related: Rust `crates/nv-daemon/src/reminders.rs` (legacy reminder logic being replaced),
  Rust `crates/nv-daemon/src/tools/schedule.rs` (legacy schedule tool logic being replaced),
  `openspec/changes/add-schedule-tables/proposal.md` (schema definition)

## Motivation

Nova v10 decomposes all tools into independently deployable microservices. The schedule domain owns
3 tool groups:

1. **Reminders** -- one-shot timers that fire at a specified time. The agent uses `set_reminder` to
   schedule something, `cancel_reminder` to revoke it, and `list_reminders` to see what is pending.
   These were previously managed by the Rust daemon's SQLite-backed reminder system.

2. **Schedules** -- user-defined recurring cron jobs. `add_schedule` creates a recurring action with
   a cron expression, `modify_schedule` updates it, `remove_schedule` deletes it, and
   `list_schedules` shows active schedules. Previously in the Rust daemon's SQLite.

3. **Sessions** -- CC session lifecycle tracking. `start_session` records when a Claude session
   begins work on a project, `stop_session` marks it as ended. This was previously in-memory only.

All three domains read and write to the Postgres tables defined by the `add-schedule-tables` spec
(reminders, schedules, sessions). The service exposes dual HTTP (for the dashboard) and MCP stdio
(for the Agent SDK) transports, consistent with all other fleet services.

## Requirements

### Req-1: Package Setup

Create `packages/tools/schedule-svc/` by copying from `packages/tools/service-template/`:

- `package.json`: name `@nova/schedule-svc`, port 4006, add `@nova/db: workspace:*` and
  `drizzle-orm` as runtime dependencies
- `tsconfig.json`: same as template
- `src/index.ts`: entry point with dual HTTP+MCP transport, service name `schedule-svc`, port 4006

### Req-2: Reminder Tools

Implement 3 tools that operate on the `reminders` table:

**`set_reminder`**
- Input: `{ description: string, due_at: string }` (due_at is ISO 8601 timestamp)
- Behavior: insert a row with `message = description`, `due_at` parsed as timestamptz,
  `channel = "schedule-svc"`, `cancelled = false`
- Returns: `"Reminder set for {due_at} — id: {id}"`

**`cancel_reminder`**
- Input: `{ id: string }` (UUID)
- Behavior: update the row with matching id, set `cancelled = true`
- Error: if no row found, return `"Reminder {id} not found"`
- Returns: `"Reminder {id} cancelled"`

**`list_reminders`**
- Input: `{ status?: "active" | "all" }` (default: `"active"`)
- Behavior: when `"active"`, query WHERE `cancelled = false AND delivered_at IS NULL`;
  when `"all"`, no filter. Order by `due_at ASC`.
- Returns: JSON array of reminder objects

### Req-3: Schedule Tools

Implement 4 tools that operate on the `schedules` table:

**`add_schedule`**
- Input: `{ name: string, cron: string, action: string }`
- Behavior: validate cron expression format (5-field standard cron), insert row with
  `cron_expr = cron`, `action`, `channel = "schedule-svc"`, `enabled = true`
- Error: if name already exists (unique constraint), return `"Schedule '{name}' already exists"`
- Returns: `"Schedule '{name}' created — id: {id}"`

**`modify_schedule`**
- Input: `{ id: string, updates: { name?: string, cron?: string, action?: string, enabled?: boolean } }`
- Behavior: update the row with matching id, applying only provided fields. If `cron` is provided,
  validate format before updating.
- Error: if no row found, return `"Schedule {id} not found"`
- Returns: `"Schedule {id} updated"`

**`remove_schedule`**
- Input: `{ id: string }` (UUID)
- Behavior: delete the row with matching id
- Error: if no row found, return `"Schedule {id} not found"`
- Returns: `"Schedule {id} removed"`

**`list_schedules`**
- Input: `{ active?: boolean }` (default: `true`)
- Behavior: when `true`, query WHERE `enabled = true`; when `false`, no filter.
  Order by `name ASC`.
- Returns: JSON array of schedule objects

### Req-4: Session Tools

Implement 2 tools that operate on the `sessions` table:

**`start_session`**
- Input: `{ name: string, metadata?: Record<string, unknown> }`
- Behavior: insert row with `project = name`, `command = metadata ? JSON.stringify(metadata) : ""``,
  `status = "running"`, `started_at = now()`
- Returns: `"Session '{name}' started — id: {id}"`

**`stop_session`**
- Input: `{ name?: string }` (optional -- if omitted, stop the most recent running session)
- Behavior: if name provided, find the most recent running session with matching project name and
  update `status = "stopped"`, `stopped_at = now()`. If name omitted, find the most recent running
  session across all projects.
- Error: if no running session found, return `"No running session found"`
- Returns: `"Session '{project}' stopped — duration: {minutes}m"`

### Req-5: HTTP Routes

Register Hono routes for all 9 tools:

| Method | Path | Tool |
|--------|------|------|
| POST | /reminders | set_reminder |
| DELETE | /reminders/:id | cancel_reminder |
| GET | /reminders | list_reminders |
| POST | /schedules | add_schedule |
| PATCH | /schedules/:id | modify_schedule |
| DELETE | /schedules/:id | remove_schedule |
| GET | /schedules | list_schedules |
| POST | /sessions/start | start_session |
| POST | /sessions/stop | stop_session |
| GET | /health | health check (from template) |

Each route maps request body/params to the tool handler and returns the tool result as JSON.

### Req-6: MCP Tool Registration

Register all 9 tools in the MCP stdio server with proper `name`, `description`, and JSON Schema
`inputSchema` for each. Tool handlers are shared between HTTP and MCP transports via the
`ToolRegistry` from the template.

### Req-7: Cron Expression Validation

Implement a `validateCron(expr: string): boolean` utility that validates 5-field standard cron
expressions (minute, hour, day-of-month, month, day-of-week). Used by `add_schedule` and
`modify_schedule` before writing to the database. No external dependencies -- simple regex
validation is sufficient for v1.

### Req-8: Database Access Pattern

All tools use the Drizzle query builder via `@nova/db`:

```typescript
import { db } from "@nova/db";
import { reminders, schedules, sessions } from "@nova/db";
import { eq, and, isNull, desc } from "drizzle-orm";
```

No raw SQL. No `pg` Pool. The `DATABASE_URL` env var is consumed by `@nova/db/client.ts` at import
time.

## Scope

- **IN**: Package setup, 9 tool implementations (set_reminder, cancel_reminder, list_reminders,
  add_schedule, modify_schedule, remove_schedule, list_schedules, start_session, stop_session),
  HTTP routes, MCP registration, cron validation utility
- **OUT**: Reminder delivery scheduler (background job that fires reminders at due_at -- separate
  spec), cron execution engine (background job that runs scheduled actions -- separate spec), data
  migration from Rust SQLite to Postgres, session dispatch logic (actually launching CC sessions),
  Traefik routing (add-fleet-deploy), systemd service file (add-fleet-deploy), MCP registration in
  ~/.claude/mcp.json (register-mcp-servers)

## Impact

| Area | Change |
|------|--------|
| `packages/tools/schedule-svc/` | New: complete service package |
| `packages/tools/schedule-svc/package.json` | New: @nova/schedule-svc with @nova/db dependency |
| `packages/tools/schedule-svc/tsconfig.json` | New: TypeScript config (from template) |
| `packages/tools/schedule-svc/src/index.ts` | New: entry point with dual transport |
| `packages/tools/schedule-svc/src/tools/reminders.ts` | New: 3 reminder tool handlers |
| `packages/tools/schedule-svc/src/tools/schedules.ts` | New: 4 schedule tool handlers |
| `packages/tools/schedule-svc/src/tools/sessions.ts` | New: 2 session tool handlers |
| `packages/tools/schedule-svc/src/tools/cron.ts` | New: cron expression validator |
| `packages/tools/schedule-svc/src/http.ts` | New: Hono routes for all 9 tools |
| `packages/tools/schedule-svc/src/mcp.ts` | New: MCP stdio server with 9 tools |

No changes to existing code. All new files in a new directory.

## Risks

| Risk | Mitigation |
|------|-----------|
| `add-schedule-tables` not applied yet (Wave 1 dependency) | Wave 5 runs after Wave 1; tables will exist. Typecheck will catch missing schema imports. |
| `scaffold-tool-service` template not yet applied | Same wave ordering. Service copies the template pattern manually if template isn't scaffolded yet. |
| Cron validation too permissive or too strict | v1 uses simple regex for standard 5-field cron; can tighten with a library (e.g., cron-parser) later |
| stop_session without name is ambiguous with multiple running sessions | Spec defines "most recent running session" — deterministic via `ORDER BY started_at DESC LIMIT 1` |
| `@nova/db` client.ts uses `DATABASE_URL` but daemon uses it too | Each process gets its own `DATABASE_URL` via Doppler injection at runtime. No conflict — Postgres handles concurrent connections. |
