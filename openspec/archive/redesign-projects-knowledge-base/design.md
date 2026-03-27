# Design: Redesign Projects as Knowledge Base

## Data Model

### projects table

```sql
CREATE TABLE projects (
  id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  code       TEXT NOT NULL UNIQUE,
  name       TEXT NOT NULL,
  category   TEXT NOT NULL DEFAULT 'work',    -- work | personal | open_source | archived
  status     TEXT NOT NULL DEFAULT 'active',  -- active | paused | completed | archived
  description TEXT,                           -- one-liner extracted from content
  content    TEXT,                            -- full markdown knowledge doc
  path       TEXT,                            -- optional filesystem path (legacy compat)
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

Category and status use `text` with application-level validation (Zod enum) rather than Postgres enums, matching the existing pattern used by `obligations.status` and `obligations.owner`.

### Relationship to existing tables

- `sessions.project` (text) -- matches `projects.code`
- `obligations.project_code` (text) -- matches `projects.code`
- `memory.topic` -- topics matching `projects-*` pattern contain project context
- `messages.content` -- full-text search for project code mentions
- `diary.content` -- full-text search for project code mentions

No foreign keys are added to avoid breaking existing insert paths. The extraction pipeline joins on code/project_code at query time.

## Entity Extraction Pipeline

The extraction endpoint (`POST /api/projects/extract`) runs these queries in parallel per project:

1. **Messages**: `SELECT COUNT(*), MAX(created_at) FROM messages WHERE content ILIKE '%{code}%'`
2. **Sessions**: `SELECT COUNT(*), MAX(started_at) FROM sessions WHERE project = '{code}'`
3. **Obligations**: `SELECT status, COUNT(*) FROM obligations WHERE project_code = '{code}' GROUP BY status`
4. **Memory**: `SELECT topic, content FROM memory WHERE topic LIKE 'projects-%' AND content ILIKE '%{code}%'`
5. **Diary**: `SELECT COUNT(*), MAX(created_at) FROM diary WHERE content ILIKE '%{code}%'`
6. **Contacts**: `SELECT DISTINCT sender FROM messages WHERE content ILIKE '%{code}%' AND sender IS NOT NULL AND LOWER(sender) != 'nova'` (top 10 by frequency)

Results are assembled into a structured markdown document:

```markdown
# {name}

> {description}

## Status
- **Category**: {category}
- **Status**: {status}
- **Last Activity**: {relative_time}

## Activity Summary
- Messages mentioning this project: {count}
- Sessions: {count} (last: {relative_time})
- Open obligations: {count}
- Total obligations: {count}

## Key Contacts
- {contact_1} ({message_count} mentions)
- {contact_2} ({message_count} mentions)

## Memory Context
{memory_content_excerpt}

## Recent Diary Entries
- {date}: {slug}
```

## API Design

| Method | Path | Purpose |
|--------|------|---------|
| GET | /api/projects | List all projects (with optional `?category=` filter) |
| POST | /api/projects | Create a new project (code, name, category) |
| PUT | /api/projects/:code | Update project metadata (name, category, status) |
| POST | /api/projects/extract | Run extraction pipeline, update all project docs |

### Seeding logic (GET /api/projects)

On first call when the `projects` table is empty:
1. Parse `NV_PROJECTS` env var (or use default `[{code:"nv", path:"~/dev/nv"}]`)
2. Insert each as a project row with category "work", status "active"
3. Return the newly created rows

Subsequent calls return from DB only. The env var is never re-read.

## UI Architecture

### Tree structure

```
[Work]                          <- CategoryNode (collapsible)
  [nv] Nova - active            <- ProjectCard
  [oo] Otaku Odyssey - active   <- ProjectCard
[Personal]
  [journal] Journal - active
[Open Source]
  [lib] My Library - paused
```

### Component hierarchy

```
ProjectsPage
  FilterTabs (All | Work | Personal | Open Source | Archived)
  SearchInput
  CreateProjectButton -> CreateProjectDialog
  RefreshButton (triggers POST /api/projects/extract)
  CategoryTree
    CategoryNode (per non-empty category)
      ProjectCard (per project in category)
  ProjectDetailPanel (slide-in, conditional)
```

### ProjectCard metrics (inline)

- Status dot (green=active, amber=paused, blue=completed, gray=archived)
- Last activity (relative time)
- Open obligations count (badge)
- Session count (badge)
- 1-line description (truncated)

### ProjectDetailPanel sections

1. **Header**: Name, status, category (editable dropdowns)
2. **Knowledge doc**: Rendered markdown (prose)
3. **Obligations**: Count by status, link to filtered obligations page
4. **Sessions**: Last 5 sessions with duration
5. **Contacts**: Top contacts who mention this project
6. **Memory**: Relevant memory topic excerpts

## Migration Path

1. DB batch creates the `projects` table
2. API batch adds all four endpoints
3. Seeding on first GET ensures zero-downtime migration
4. UI batch replaces the page, removes `ProjectAccordion.tsx`
5. `NV_PROJECTS` env var can be removed from deployment after confirming DB is populated
