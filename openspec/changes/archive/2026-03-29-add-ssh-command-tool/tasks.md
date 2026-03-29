# Implementation Tasks

<!-- beads:epic:nv-61iue -->

## API Batch

- [x] [1.1] [P-1] Create `packages/tools/azure-svc/src/tools/ssh-command.ts` — new tool that accepts any command string, calls `sshCloudPC()` directly (no sanitization, no prefix check), returns stdout, logs command preview + metrics [owner:api-engineer] [beads:nv-lg2bq]
- [x] [1.2] [P-1] Register `ssh_command` in `packages/tools/azure-svc/src/mcp.ts` — add `server.registerTool("ssh_command", ...)` with Zod schema for `command` string param, matching `azure_cli` pattern [owner:api-engineer] [beads:nv-c8z9t]
- [x] [1.3] [P-1] Add `POST /ssh` route in `packages/tools/azure-svc/src/http.ts` — extract `command` from body, call handler, return `{result, error}` matching `/az` response shape [owner:api-engineer] [beads:nv-hearw]
- [x] [1.4] [P-2] Update `config/system-prompt.md` — add `ssh_command` to the tool examples section alongside `azure_cli` and `teams_list_chats`, describing it as running any command on CloudPC [owner:api-engineer] [beads:nv-8tx1x]

## E2E Batch

- [ ] [2.1] [deferred] Manual test — message Nova "what processes are running on CloudPC" and verify it calls `ssh_command` with a Get-Process command [owner:user] [beads:nv-dl1a4]
