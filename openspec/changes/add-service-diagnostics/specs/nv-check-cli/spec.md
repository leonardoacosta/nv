# nv check CLI & check_services Tool

## ADDED Requirements

### Requirement: nv check Subcommand

The binary MUST provide a `check` clap subcommand that loads config and credentials, instantiates all configured service clients, runs their `Checkable` probes concurrently, and outputs categorized results with status icons, latencies, and a summary line. It MUST support `--json`, `--read-only`, and `--service <name>` flags.

#### Scenario: Check all services with colored terminal output

**Given** the user runs `nv check`
**When** all probes complete
**Then** output is grouped by category (Channels, Tools read, Tools write)
**And** each line shows: status icon (✓/✗/○), service name (with instance suffix if multi), detail, latency
**And** a summary line shows total healthy/unhealthy/missing/disabled counts

#### Scenario: JSON output for scripting

**Given** the user runs `nv check --json`
**Then** output is a JSON object with `channels`, `tools_read`, `tools_write`, and `summary` keys
**And** each entry has `name`, `status`, `detail`, `latency_ms` fields

#### Scenario: Single service check

**Given** the user runs `nv check --service stripe`
**Then** only Stripe instances are checked (all instances if multi-instance)
**And** output format matches the full check but filtered

#### Scenario: Read-only mode

**Given** the user runs `nv check --read-only`
**Then** write probes are skipped entirely
**And** output only shows channel and read check sections

#### Scenario: Missing env vars reported without probing

**Given** a service has required env vars not set
**When** `nv check` runs
**Then** that service reports `Missing` immediately without attempting a network call
**And** the missing env var name is shown in the detail column

### Requirement: check_services Nova Tool

The daemon MUST register a `check_services` tool definition that Nova can invoke to run health probes and receive structured JSON results. This MUST allow Nova to self-diagnose auth failures without human intervention.

#### Scenario: Nova can self-diagnose after tool failure

**Given** a tool call returns an auth error
**When** Nova calls `check_services` tool
**Then** it receives structured JSON with all service statuses
**And** can report to the user which specific credential is invalid or missing

#### Scenario: Tool definition registered in tools registry

**Given** the daemon starts
**Then** `check_services` is registered with schema:
```json
{
  "type": "object",
  "properties": {
    "service": {
      "type": "string",
      "description": "Optional: check a specific service only (e.g., 'stripe', 'jira')"
    },
    "read_only": {
      "type": "boolean",
      "description": "Skip write probes if true"
    }
  }
}
```

### Requirement: Health Endpoint Integration

The `/health` HTTP endpoint MUST support a `?deep=true` query parameter that triggers full service probes and includes per-service `CheckResult` data in the response. Without the parameter, the endpoint SHALL return the existing fast response with no probes.

#### Scenario: Deep health includes tool status

**Given** the HTTP health endpoint at `GET /health`
**When** called with query param `?deep=true`
**Then** response includes `tools` field with per-service `CheckResult` data
**And** overall status is `degraded` if any tool is `Unhealthy`, `ok` if all healthy
**When** called without `?deep=true`
**Then** response is unchanged (fast, no probes)
