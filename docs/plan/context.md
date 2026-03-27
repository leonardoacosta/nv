# Nova Dashboard Redesign — Session Context

> Pass to next session: `/apply:all --continue --context=docs/plan/context.md`

## Wave Plan State

| Wave | Name | Status | Specs | Tasks |
|------|------|--------|-------|-------|
| 1 | Foundation | Done | fix-viewport-overflow, global-density-pass | 1 applied (density was pre-applied) |
| 2 | Page Redesigns | Done | cleanup-diary-page, redesign-dashboard-home, redesign-contacts-graph | 7 applied (dashboard+contacts pre-applied) |
| 3 | Data Pages | Pending | redesign-sessions-timeline (20), redesign-projects-knowledge-base (21) | 41 tasks |
| 4 | Automations | Pending | enhance-automations-page (20) | 20 tasks |

Plan file: `docs/plan/wave-plan.json`
Tags: `nova-redesign-wave-1`, `nova-redesign-wave-2`

## Pre-Applied Specs (discovered during execution)

These had all implementation tasks `[x]` before this session. Only `[user]` visual review tasks remain:
- `global-density-pass` — all pages density-tightened
- `redesign-dashboard-home` — command center layout with activity feed
- `redesign-contacts-graph` — auto-discovered contacts from messages
- `redesign-integrations-status` — 0 open tasks, excluded from plan

## Key Technical Notes

1. **DATABASE_URL not POSTGRES_URL** — Nova's DB client (`packages/db/src/client.ts`) reads `DATABASE_URL`. Docker builds pass a fake one (`DATABASE_URL=postgresql://build:build@localhost:5432/build`). Local `pnpm build` fails without it — use `doppler run` or build in Docker.

2. **Deploy is automatic** — `git push` to main triggers a pre-push hook that selectively rebuilds only changed services (daemon/fleet/dashboard). Dashboard changes trigger Docker container rebuild via `docker compose`.

3. **docs/screenshots/ is gitignored** — audit screenshots saved locally at `docs/screenshots/nova-audit/` but can't be committed.

4. **Contacts data gap** — only 1 contact discovered from 164 conversations. SQL groups by `sender` field (mostly Leo's Telegram ID). The `redesign-projects-knowledge-base` and `redesign-contacts-graph` specs add entity extraction pipelines.

5. **nv check disconnected from dashboard** — Rust CLI does real health probes (Stripe, Vercel, Sentry, etc.) but Status page shows static "unknown" due to Docker network isolation. `redesign-integrations-status` spec addresses this but was already applied.

6. **"Brief" overloaded** — `enhance-automations-page` renames Telegram `/brief` to `/snapshot`. Dashboard "Briefing" stays as-is (full morning digest).

## Wave 3 Notes

Sessions timeline (20 tasks): New `session_events` table, paginated historical list, vertical interaction timeline on detail page, CC sessions widget moved to dashboard.

Projects knowledge base (21 tasks): New `projects` table with markdown content column, predefined categories (work/personal/open_source/archived), entity extraction from 5 data sources, Obsidian-style tree UI.

Both share `types/api.ts` and `packages/db/src/index.ts` (barrel file additions only — no destructive conflicts).

## Wave 4 Notes

Automations enhancement (20 tasks): New `settings` table (key-value, DB-persisted), custom prompt textareas, configurable briefing hour, cross-page nav links, reminder creation form, Telegram `/brief` renamed to `/snapshot`.

Depends on Wave 3 because it adds `?command=` filter to the sessions page (which Wave 3 rewrites).
