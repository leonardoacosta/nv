# Implementation Tasks

<!-- beads:epic:nv-44w -->

## Code Batch

- [x] [1.1] [P-1] Add default_project fallback to both jira_create handlers in mod.rs before validate_jira_project_key [owner:api-engineer] [beads:nv-soc]
- [x] [1.2] [P-1] Add tracing::info! when falling back to default project [owner:api-engineer] [beads:nv-crm]

## E2E Batch

- [x] [2.1] Test jira_create without project field resolves to default_project [owner:test-writer] [beads:nv-rnj]
