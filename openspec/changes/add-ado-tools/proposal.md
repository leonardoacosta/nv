# Proposal: Add Azure DevOps Tools

## Change ID
`add-ado-tools`

## Summary

Azure DevOps pipeline and build status tools via REST API (dev.azure.com). Two read-only tools
(`ado_pipelines`, `ado_builds`) that query ADO for pipeline definitions and build results,
giving Nova visibility into day-job CI/CD status.

## Context
- Extends: `crates/nv-daemon/src/tools.rs` (tool definitions + dispatch), `crates/nv-daemon/src/agent.rs` (tool execution)
- Related: Existing tool pattern (Jira, Nexus, Memory tools), `add-tool-audit-log` spec (audit logging)
- PRD ref: Phase 2, Section 6.1 — Tier 4 (Special — day job)

## Motivation

Azure DevOps hosts the CI/CD pipelines for day-job projects. Currently Leo must open the ADO
portal to check build status, which breaks focus. Wiring ADO into Nova lets Leo ask "Any failed
builds on ProjectX?" or "What pipelines are running?" from Telegram without context-switching.

## Requirements

### Req-1: HTTP Client Module

New file `crates/nv-daemon/src/ado.rs` with:
- `AdoClient` struct holding organization URL, PAT, and reqwest client
- Base URL: `https://dev.azure.com/{organization}` (configurable)
- Auth: Basic auth with empty username + PAT as password, or `Authorization: Basic base64(:$PAT)`
- API version: `api-version=7.1` query parameter on all requests
- All requests are GET (read-only)

### Req-2: ado_pipelines Tool

`ado_pipelines(project)` — List pipeline definitions for a project.

- Endpoint: `GET /{project}/_apis/pipelines?api-version=7.1`
- Input: `project` (required) — ADO project name
- Output: Formatted list of pipelines with id, name, folder, revision
- Cap: return first 50 pipelines

### Req-3: ado_builds Tool

`ado_builds(pipeline_id)` — List recent builds for a pipeline.

- Endpoint: `GET /{project}/_apis/build/builds?definitions={pipeline_id}&$top=10&api-version=7.1`
- Input: `pipeline_id` (required) — pipeline definition ID (integer)
- Output: Formatted list of recent builds with buildNumber, status, result, queueTime, finishTime, sourceBranch, requestedFor
- Default: last 10 builds

### Req-4: Tool Registration

Register both tools in `register_tools()` with Anthropic tool schema format.
Wire dispatch in `execute_tool()` to call AdoClient methods.

### Req-5: Configuration

- Env vars: `ADO_ORG` (organization name), `ADO_PAT` (Personal Access Token), `ADO_PROJECT` (default project, optional)
- Alternative: support `az` CLI auth if PAT not set (shell out to `az devops` commands)
- Fail gracefully: if neither PAT nor az CLI available, tools return "Azure DevOps not configured"

### Req-6: Audit Logging

Every tool invocation logged via tool audit log. Log: tool name, project, pipeline_id, success/failure, duration_ms.

## Scope
- **IN**: AdoClient HTTP module, ado_pipelines tool, ado_builds tool, tool registration, env config
- **OUT**: Triggering builds, managing work items, repo management, release management, test plans

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/ado.rs` | New: AdoClient with pipelines(project), builds(pipeline_id) |
| `crates/nv-daemon/src/tools.rs` | Add 2 tool definitions + dispatch cases |
| `crates/nv-daemon/src/main.rs` | Init AdoClient, pass to tool executor |
| `config/env` or `.env` | Add ADO_ORG, ADO_PAT, ADO_PROJECT |

## Risks
| Risk | Mitigation |
|------|-----------|
| PAT expires | ADO PATs have max 1-year expiry. Log auth failures clearly. |
| Corporate network required | ADO is public cloud (dev.azure.com), accessible via Tailscale. |
| PAT overprivileged | Create PAT with Build (Read) + Pipeline (Read) scopes only. |
| az CLI not installed | PAT is primary auth. az CLI is optional fallback. Document both paths. |
