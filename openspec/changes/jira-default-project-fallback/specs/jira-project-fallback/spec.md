# Capability: Jira Project Fallback

## MODIFIED Requirements

### Requirement: jira_create project resolution
The `jira_create` handler MUST fall back to `default_project` from the Jira registry's default
client config when the `project` field is empty or missing, before running validation.

#### Scenario: Claude omits project field
**Given** Claude calls `jira_create` with `{"issue_type": "Bug", "title": "Fix login"}`
**When** the handler detects `project` is empty
**Then** it reads `default_project` from the registry's default client config
**And** uses that value for the rest of the handler (validation, confirmation, execution)

#### Scenario: Claude provides project field
**Given** Claude calls `jira_create` with `{"project": "TC", "issue_type": "Task", "title": "Add feature"}`
**When** the handler processes the input
**Then** it uses `"TC"` as-is without fallback

#### Scenario: No default client available
**Given** the Jira registry has no default client configured
**When** Claude omits the `project` field
**Then** the handler returns the same validation error as before (no regression)

### Requirement: Fallback tracing
The handler MUST emit a `tracing::info!` log when falling back to the default project,
including the resolved project key.

#### Scenario: Fallback logged
**Given** Claude omits `project` and fallback resolves to `"OO"`
**When** the fallback is applied
**Then** a log line like `jira_create: project not provided, falling back to default "OO"` is emitted
