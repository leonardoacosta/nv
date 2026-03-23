# Proposal: Jira Default Project Fallback

## Change ID
`jira-default-project-fallback`

## Summary
When Claude omits the `project` field in `jira_create`, fall back to the `default_project`
from the Jira registry config instead of returning an error.

## Context
- Extends: `crates/nv-daemon/src/tools/mod.rs` (both execute_tool functions)
- Related: `crates/nv-core/src/config.rs` JiraInstanceConfig has `default_project` field

## Motivation
The `jira_create` tool requires `project`, `issue_type`, and `title`. When Claude omits
`project`, the code falls back to an empty string which fails validation. The Jira config
already has a `default_project` per instance, but the tool handler never reads it as a
fallback. This causes unnecessary failures when Claude's inner session forgets to specify
the project — a common occurrence visible in production logs.

## Requirements

### Req-1: Default project fallback
When the `project` field is empty or missing in `jira_create`, the handler must fall back
to `default_project` from the Jira registry's default client config before validation.

### Req-2: Fallback logging
When falling back to the default project, emit a `tracing::info!` so the fallback is
visible in logs for debugging.

## Scope
- **IN**: jira_create handler fallback logic in both execute_tool functions, tracing
- **OUT**: Changes to tool schema required fields, other Jira tools, config changes

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/tools/mod.rs` (~line 1523, ~line 2327) | Add default_project fallback before `validate_jira_project_key` |

## Risks
| Risk | Mitigation |
|------|-----------|
| Default project may not be the intended target | Confirmation gate still shows project before execution |
| Registry may have no default client | Fall back to error (current behavior) if no default available |
