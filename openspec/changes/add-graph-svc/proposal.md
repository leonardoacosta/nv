# Proposal: Add Graph Service

## Change ID
`add-graph-svc`

## Summary

Build the Graph service (`packages/tools/graph-svc/`) -- a Hono+MCP microservice on port 4007 that
exposes Outlook Calendar and Azure DevOps tools via SSH to the CloudPC. Six tools total: three
calendar tools (`calendar_today`, `calendar_upcoming`, `calendar_next`) and three ADO tools
(`ado_projects`, `ado_pipelines`, `ado_builds`). Thin wrapper pattern: HTTP/MCP request comes in,
SSH exec runs PowerShell on the CloudPC, output is parsed and returned as JSON.

## Context

- Phase: 3 -- Communication Tools | Wave: 4
- Feature area: tools
- Depends on: `scaffold-tool-service` (Wave 1) -- copies the service template
- Roadmap: `docs/plan/nova-v10/wave-plan.json`
- Existing Rust implementations:
  - `crates/nv-daemon/src/tools/outlook.rs` -- calendar via `graph-outlook.ps1` on CloudPC
  - `crates/nv-daemon/src/tools/cloudpc.rs` -- SSH helper (`ssh_cloudpc_script`)
- PowerShell scripts on CloudPC:
  - `graph-outlook.ps1` -- calendar operations (CalendarToday, CalendarUpcoming, CalendarNext)
  - `graph-ado.ps1` -- ADO operations (Projects, Pipelines, Builds)
- SSH host: `cloudpc` (defined in SSH config)
- Architecture decision: SSH to CloudPC is primary for all MS Graph operations (user decision,
  documented in `docs/plan/nova-v10/wave-plan.json`)

## Motivation

Nova needs access to the user's Outlook calendar and Azure DevOps project status. The CloudPC is
the only machine with authenticated Microsoft Graph access (PowerShell scripts manage their own
device-code + token refresh flow). The Rust daemon already routes calendar and ADO operations
through SSH to the CloudPC -- this service ports that proven pattern to the TypeScript tool fleet.

Calendar tools enable Nova to answer "What's on my calendar today?", "What's my next meeting?", and
"What do I have this week?" -- critical for daily briefings and scheduling awareness.

ADO tools enable Nova to answer "What pipelines ran recently?", "Did the build pass?", and "What
projects exist?" -- important for development workflow awareness.

## Requirements

### Req-1: Package Scaffold

Copy `packages/tools/service-template/` to `packages/tools/graph-svc/`. Update `package.json`:
- `name: "@nova/graph-svc"`
- No additional dependencies beyond the template (SSH is via `child_process`)

### Req-2: SSH Client Module

Create `src/ssh.ts` -- a reusable SSH helper for executing PowerShell scripts on the CloudPC:

- `sshCloudPC(script: string, args: string): Promise<string>` -- spawns `ssh cloudpc "powershell -ExecutionPolicy Bypass -Command \"& { . C:\\Users\\leo.346-CPC-QJXVZ\\<script> <args> }\""` via `child_process.execFile`
- Connection timeout: 10 seconds (`-o ConnectTimeout=10`)
- Filter noise lines from stdout (lines containing `WARNING:`, `vulnerable`, `upgraded`, `security fix`)
- On connection failure (stderr contains `Connection refused`, `timed out`, `No route to host`): throw `"CloudPC unreachable -- cannot connect to 'cloudpc' via SSH"`
- On non-zero exit: throw with stderr content

This mirrors the Rust `cloudpc::ssh_cloudpc_script` function exactly.

### Req-3: Calendar Tools

Create `src/tools/calendar.ts` with three tools:

**calendar_today()**
- No required parameters
- Runs: `graph-outlook.ps1 -Action CalendarToday`
- Returns: today's calendar events as formatted text

**calendar_upcoming(days?)**
- Optional parameter: `days` (integer, 1-14, default 7)
- Runs: `graph-outlook.ps1 -Action CalendarUpcoming -Days <days>`
- Returns: upcoming events for the specified number of days

**calendar_next()**
- No required parameters
- Runs: `graph-outlook.ps1 -Action CalendarNext`
- Returns: the next upcoming event

All three tools return the raw PowerShell output as-is (the scripts format their own output).
If SSH fails, return the error message.

### Req-4: ADO Tools

Create `src/tools/ado.ts` with three tools:

**ado_projects()**
- No required parameters
- Runs: `graph-ado.ps1 -Action Projects`
- Returns: list of Azure DevOps projects

**ado_pipelines(project?)**
- Optional parameter: `project` (string, default: all projects)
- Runs: `graph-ado.ps1 -Action Pipelines [-Project <project>]`
- Returns: list of pipelines, optionally filtered by project

**ado_builds(project?, pipeline?, limit?)**
- Optional parameters: `project` (string), `pipeline` (string), `limit` (integer, 1-50, default 10)
- Runs: `graph-ado.ps1 -Action Builds [-Project <project>] [-Pipeline <pipeline>] [-Limit <limit>]`
- Returns: recent builds, optionally filtered

All three tools return the raw PowerShell output as-is. If SSH fails, return the error message.

### Req-5: Tool Registration

Create `src/tools/index.ts` -- registers all 6 tools in the service's `ToolRegistry`:

| Tool | Name | Description | Required Params | Optional Params |
|------|------|-------------|-----------------|-----------------|
| calendar_today | `calendar_today` | Get today's calendar events from Outlook | none | none |
| calendar_upcoming | `calendar_upcoming` | Get upcoming calendar events | none | `days` (1-14, default 7) |
| calendar_next | `calendar_next` | Get the next upcoming calendar event | none | none |
| ado_projects | `ado_projects` | List Azure DevOps projects | none | none |
| ado_pipelines | `ado_pipelines` | List Azure DevOps pipelines | none | `project` (string) |
| ado_builds | `ado_builds` | Get recent Azure DevOps builds | none | `project`, `pipeline`, `limit` (1-50) |

Each tool's `inputSchema` follows JSON Schema format matching the Rust tool definitions pattern.

### Req-6: HTTP Routes

Wire Hono routes in `src/http.ts` (extending the template):

- `GET /health` -- inherited from template
- `GET /calendar/today` -- calls `calendar_today()`
- `GET /calendar/upcoming?days=N` -- calls `calendar_upcoming(days)`
- `GET /calendar/next` -- calls `calendar_next()`
- `GET /ado/projects` -- calls `ado_projects()`
- `GET /ado/pipelines?project=X` -- calls `ado_pipelines(project)`
- `GET /ado/builds?project=X&pipeline=Y&limit=N` -- calls `ado_builds(project, pipeline, limit)`

All routes return `{ result: string }` on success, `{ error: string, status: number }` on failure.
SSH timeout/connection errors return HTTP 503 (Service Unavailable).

### Req-7: Service Config

Update `src/config.ts`:
- `SERVICE_NAME` defaults to `"graph-svc"`
- `SERVICE_PORT` defaults to `4007`
- `CLOUDPC_HOST` defaults to `"cloudpc"` (SSH config name)
- `CLOUDPC_USER_PATH` defaults to `"C:\\Users\\leo.346-CPC-QJXVZ"` (path to scripts on CloudPC)

### Req-8: Error Handling

All tool handlers must handle SSH failures gracefully:
- CloudPC unreachable: return `"CloudPC unreachable -- calendar/ADO tools require the CloudPC to be online. SSH connection to 'cloudpc' failed."` with HTTP 503
- Script error (non-zero exit): return the stderr content as the error message with HTTP 502
- Timeout (SSH hangs beyond 30 seconds): kill the child process and return `"CloudPC SSH timed out after 30 seconds"` with HTTP 504

## Scope

- **IN**: Package scaffold, SSH client, 6 tool implementations (3 calendar + 3 ADO), HTTP routes, MCP tool registration, config, error handling
- **OUT**: PowerShell scripts themselves (already exist on CloudPC), Traefik routing (handled by add-fleet-deploy), systemd service file (handled by add-fleet-deploy), MCP registration in mcp.json (handled by register-mcp-servers), health aggregation in tool-router

## Impact

| Area | Change |
|------|--------|
| `packages/tools/graph-svc/` | New directory -- complete service from template |
| `packages/tools/graph-svc/package.json` | New -- `@nova/graph-svc` package manifest |
| `packages/tools/graph-svc/tsconfig.json` | New -- TypeScript config (from template) |
| `packages/tools/graph-svc/src/index.ts` | New -- entry point with dual HTTP/MCP transport |
| `packages/tools/graph-svc/src/ssh.ts` | New -- SSH helper for CloudPC script execution |
| `packages/tools/graph-svc/src/tools/calendar.ts` | New -- 3 calendar tool handlers |
| `packages/tools/graph-svc/src/tools/ado.ts` | New -- 3 ADO tool handlers |
| `packages/tools/graph-svc/src/tools/index.ts` | New -- tool registration |
| `packages/tools/graph-svc/src/http.ts` | New -- Hono routes for all 6 tools |
| `packages/tools/graph-svc/src/config.ts` | New -- service-specific config |

No changes to existing code. No changes to the daemon, dashboard, or other tool services.

## Risks

| Risk | Mitigation |
|------|-----------|
| CloudPC offline -- all 6 tools fail | Expected and documented. Tools return clear error messages. Health endpoint stays healthy (service itself is fine, just SSH target is down). Future: health check could probe CloudPC SSH connectivity. |
| PowerShell script argument injection via user input | `ado_pipelines(project)` and `ado_builds(project, pipeline)` accept user strings passed to SSH. Sanitize by stripping single quotes and semicolons from all parameters before interpolation into the SSH command. |
| SSH key not configured on deploy host | Prerequisite: SSH config must have `cloudpc` host entry with key-based auth. Documented in service README. Existing Rust daemon already depends on this. |
| Long-running SSH commands block the event loop | `child_process.execFile` is async (callback-based, wrapped in Promise). 30-second timeout kills the process. No event loop blocking. |
| PowerShell script output format changes | Service passes raw output through. No parsing means no breakage from format changes. Future: structured JSON output from scripts would be better but is out of scope. |

## Dependencies

- `scaffold-tool-service` -- service template must exist before this spec runs
