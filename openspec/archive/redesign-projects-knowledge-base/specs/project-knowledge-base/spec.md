# Capability: Project Knowledge Base

## ADDED Requirements

### Requirement: Database-backed project entities
Projects SHALL be persisted in a `projects` Postgres table. Each project MUST have a unique `code` identifier, a display `name`, a `category` (work, personal, open_source, archived), a `status` (active, paused, completed, archived), a `content` column storing auto-generated markdown, a `description` one-liner, and standard timestamps.

#### Scenario: First load seeds from NV_PROJECTS
Given the `projects` table is empty
And the `NV_PROJECTS` env var contains `[{"code":"nv","path":"~/dev/nv"}]`
When GET /api/projects is called
Then a project row is inserted with code "nv", category "work", status "active"
And the response includes the newly created project entity

#### Scenario: Project with all fields populated
Given a project "nv" exists with category "work" and status "active"
And extraction has populated the content field with markdown
When GET /api/projects is called
Then the response includes the project with all enrichment fields

---

### Requirement: Predefined category hierarchy
Projects MUST belong to exactly one category. The four categories SHALL be: work, personal, open_source, archived. The API MUST return projects grouped by category. The UI SHALL render categories as collapsible tree nodes.

#### Scenario: Filter by category
Given projects exist in categories work, personal, and open_source
When the user selects the "Work" filter tab
Then only projects with category "work" are displayed

#### Scenario: Archived projects hidden by default
Given some projects have category "archived"
When the page loads with the "All" filter active
Then archived projects are not shown
And the "Archived" tab shows them when selected

---

### Requirement: Entity extraction pipeline
The extraction endpoint SHALL scan messages, diary, memory, sessions, and obligations tables, MUST aggregate per-project statistics, and SHALL generate a structured markdown knowledge document per project.

#### Scenario: Extract project knowledge
Given project "nv" exists in the DB
And 150 messages mention "nv", 12 sessions reference "nv", 5 open obligations have project_code "nv"
When POST /api/projects/extract is called
Then the project's content field is updated with a markdown document containing message stats, session summary, obligation breakdown, related contacts, and memory context
And the response includes extraction stats (projects_updated, sources_scanned)

#### Scenario: Extract with no data
Given project "empty-proj" exists but no data sources reference it
When POST /api/projects/extract is called
Then the project's content is set to a minimal markdown doc with "No activity found"
And the project is not deleted

---

### Requirement: Obsidian-style tree display
The page SHALL render a two-level collapsible tree: category headers as top-level nodes, project cards as children. Each category header MUST show a count badge and SHALL be collapsible. Project cards MUST show key metrics inline.

#### Scenario: Tree renders with categories
Given 3 work projects and 2 personal projects exist
When the page loads
Then two category groups render: "Work (3)" and "Personal (2)"
And each group is expanded by default
And each project card shows name, status badge, last activity, open obligations count

#### Scenario: Empty category hidden
Given all projects are category "work"
When the page loads
Then only the "Work" category group renders
And "Personal" and "Open Source" headers do not appear

---

### Requirement: Project detail panel
Clicking a project card SHALL open a slide-in panel from the right with the full knowledge document, activity summary, and MUST provide edit controls for name, category, and status.

#### Scenario: View project detail
Given project "nv" has a populated knowledge document
When the user clicks the "nv" project card
Then a detail panel slides in from the right
And displays the rendered markdown content, obligation summary, recent sessions, and related contacts

#### Scenario: Edit project metadata
Given the detail panel is open for project "nv"
When the user changes category from "work" to "personal" and clicks save
Then PUT /api/projects/nv is called with category "personal"
And the tree view updates to move "nv" under the "Personal" group

## MODIFIED Requirements

### Requirement: Project API contract replaces existing GET /api/projects
The existing endpoint SHALL return `ProjectEntity[]` from the database instead of `ApiProject[]` from env var. The endpoint MUST provide backward-compatible seeding from `NV_PROJECTS` when the table is empty.

#### Scenario: Backward compatible response
Given projects exist in the database
When GET /api/projects is called
Then the response includes code, name, category, status, description, content (truncated), obligation_count, active_obligation_count, session_count, last_activity
And the response shape is `{ projects: ProjectEntity[] }`

## REMOVED Requirements

### Requirement: NV_PROJECTS as sole data source
The `NV_PROJECTS` env var is no longer the source of truth for projects. It becomes an optional seed for initial migration only.

#### Scenario: Env var ignored after seeding
Given projects already exist in the database
When GET /api/projects is called and NV_PROJECTS contains additional entries
Then only database projects are returned
And no new projects are created from the env var
