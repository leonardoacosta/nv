# Proposal: Add SSH Command Tool

## Change ID
`add-ssh-command-tool`

## Summary
Add a generic `ssh_command` MCP tool to azure-svc that executes arbitrary commands on the CloudPC via SSH, removing the `az ` prefix restriction and enabling PowerShell, diagnostics, file operations, and any CLI tool.

## Context
- Extends: `packages/tools/azure-svc/src/tools/`, `packages/tools/azure-svc/src/mcp.ts`, `packages/tools/azure-svc/src/http.ts`
- Related: `azure_cli` tool (existing — restricted to `az ` prefix)

## Motivation
Nova currently can only run `az` commands on CloudPC. Any other operation — PowerShell diagnostics, file reads, network checks, non-Azure CLIs (git, dotnet, winget), service control, or ad-hoc scripts — is impossible. The SSH infrastructure (ControlMaster, 5-minute timeout, metrics logging) already exists in azure-svc. Adding a second tool that drops the `az ` restriction unlocks the full CloudPC surface.

## Requirements

### Req-1: Generic SSH command execution
A new `ssh_command` tool that accepts any command string, executes it on CloudPC via the existing `sshCloudPC()` function, and returns stdout. No prefix restriction, no shell metacharacter stripping (the command runs as-is on the remote shell).

### Req-2: MCP and HTTP registration
Register the tool in both MCP (`mcp.ts`) and HTTP (`http.ts`) transports, matching the pattern used by `azure_cli`.

### Req-3: System prompt awareness
Add `ssh_command` to the daemon system prompt so Nova knows it can run arbitrary commands on CloudPC.

## Scope
- **IN**: New `ssh-command.ts` tool file, MCP registration, HTTP route, system prompt update
- **OUT**: Modifying `azure_cli` behavior, changing SSH infrastructure, adding command restrictions

## Impact
| Area | Change |
|------|--------|
| `packages/tools/azure-svc/src/tools/ssh-command.ts` | New tool — execute arbitrary commands via SSH |
| `packages/tools/azure-svc/src/mcp.ts` | Register `ssh_command` tool |
| `packages/tools/azure-svc/src/http.ts` | Add `POST /ssh` route |
| `config/system-prompt.md` | Add `ssh_command` to tool examples |

## Risks
| Risk | Mitigation |
|------|-----------|
| Destructive commands (rm, format, Restart-Computer) | Nova's system prompt requires operator confirmation for writes affecting others; fully open by design |
| Large stdout responses consuming tokens | Existing response size is bounded by SSH timeout; agent naturally truncates |
