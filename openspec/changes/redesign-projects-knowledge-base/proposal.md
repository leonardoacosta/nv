# Proposal: Redesign Projects as Knowledge Base

## Change ID
`redesign-projects-knowledge-base`

## Summary
Replace the flat filesystem-path project list (driven by `NV_PROJECTS` env var) with a database-backed hierarchical knowledge base where each project is a knowledge entity with auto-generated markdown docs, categorized into groups (Work, Personal, Open Source, Archived), and displayed as an Obsidian-style collapsible tree with rich detail panels.

## Context
- Extends: `packages/db/src/schema/` (new projects table), `apps/dashboard/app/projects/page.tsx` (full rewrite), `apps/dashboard/app/api/projects/route.ts` (rewrite + new endpoints), `apps/dashboard/lib/entity-resolution/project-enrichment.ts` (extend to extraction pipeline), `apps/dashboard/types/api.ts` (new response types)
- Related: `redesign-contacts-graph` (archived) -- identical pattern of discovering entities from conversation data, category filter tabs, detail panel. This spec mirrors that architecture for projects.
- Existing infra: `sessions` table has `project` column, `obligations` table has `project_code` column, `memory` table has `projects-*` topics, `diary` and `messages` tables contain unstructured project references

## Motivation
The current Projects page is a static registry: it reads `NV_PROJECTS` (a JSON array of `{code, path}`) and renders a flat accordion list. Every project shows status "UNKNOWN" and the only enrichment is obligation/session counts from a simple DB lookup. This is inadequate for a personal AI assistant:

- **Projects are knowledge entities, not filesystem paths.** A project has history, context, people, obligations, and activity patterns that Nova already tracks across messages, diary entries, memory, and sessions.
- **No discoverability.** Only projects in the env var appear. Projects mentioned in conversations, obligations, or memory but not in the env var are invisible.
- **No hierarchical organization.** Real projects have categories (work vs personal vs open source) and varying levels of activity. A flat list with no grouping scales poorly.
- **No knowledge synthesis.** The Contacts page already demonstrates entity extraction from conversation data. Projects deserve the same treatment: scan all data sources, auto-generate a rich knowledge document per project.

## Requirements

### Req-1: Database-backed project entities
Projects are stored in a new `projects` Postgres table with: id, code (unique), name, category (enum: work, personal, open_source, archived), markdown content (auto-generated knowledge doc), status (active, paused, completed, archived), and timestamps. The `NV_PROJECTS` env var becomes a seed source for initial project creation, not the source of truth.

### Req-2: Predefined category hierarchy
Projects belong to one of four categories: Work, Personal, Open Source, Archived. The UI displays projects grouped by category in a collapsible tree structure. Filter tabs allow quick category switching (All, Work, Personal, Open Source, Archived).

### Req-3: Entity extraction pipeline
A manual extraction pipeline (triggered by Refresh button) scans all data sources -- messages, diary, memory, sessions, obligations -- for project references. For each known project code, it aggregates: message count and last mention, session count and last activity, obligation counts by status, related contacts (people who mention the project), memory topic summaries, and recent diary entries. The pipeline generates a markdown knowledge document per project.

### Req-4: Obsidian-style tree display
Replace the flat accordion list with a collapsible tree: top-level nodes are categories, child nodes are project cards. Each project card shows: name, status badge, category pill, last activity (relative time), open obligation count, session count, and a 1-line description extracted from memory or the knowledge doc.

### Req-5: Project detail panel
Clicking a project opens a slide-in detail panel (matching the Contacts `ContactDetailPanel` pattern): full markdown knowledge doc rendered as prose, activity timeline, obligation summary, recent sessions list, related contacts, and memory context. The panel supports manual project creation/editing for name, category, and status.

## Scope
- **IN**: New `projects` DB table, entity extraction from existing tables (messages, diary, memory, sessions, obligations), category-based tree UI, project detail panel, Refresh button triggers extraction, manual project creation from UI, seed from `NV_PROJECTS` on first load
- **OUT**: Automatic extraction on message ingest (future: daemon-side hook), full NER for discovering unknown project names from free text, filesystem markdown export, git history integration (requires local git access unavailable from dashboard), real-time status monitoring (healthchecks, CI status)

## Impact
| Area | Change |
|------|--------|
| DB schema | New `projects` table with category enum, markdown content column |
| API | Rewrite `GET /api/projects`, add `POST /api/projects`, `PUT /api/projects/:code`, `POST /api/projects/extract` |
| UI | Full rewrite of projects page: tree view, filter tabs, detail panel, create dialog |
| Types | New `ProjectEntity`, `ProjectCategory`, `ProjectExtractionResponse` types |
| Entity resolution | Extend `project-enrichment.ts` into full extraction pipeline |
| Existing code removed | `ProjectAccordion.tsx` component, `NV_PROJECTS` env var dependency (becomes optional seed) |

## Risks
| Risk | Mitigation |
|------|-----------|
| Extraction pipeline slow on large message tables | Use indexed queries, aggregate in SQL not JS, show progress indicator during extraction |
| Category assignment ambiguity (is "nv" work or personal?) | Default to "work", let user reassign via detail panel |
| Migration: existing `NV_PROJECTS` users lose nothing | Seed endpoint auto-creates DB rows from env var on first GET if table is empty |
| Knowledge doc content quality | Start with structured sections (stats, contacts, timeline), improve prompts later |
