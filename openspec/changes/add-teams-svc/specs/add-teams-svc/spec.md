# Capability: Teams Service

## ADDED Requirements

### Requirement: SSH-to-CloudPC Execution Helper
`src/ssh.ts` SHALL export a `sshCloudPc(script: string, args: string): Promise<string>` function
that spawns `ssh -o ConnectTimeout=10 cloudpc "powershell -ExecutionPolicy Bypass -Command ..."`,
captures stdout, filters noise lines (WARNING, vulnerable, upgraded, security fix), and returns
the cleaned output. The function MUST throw on SSH connection failures with a "CloudPC unreachable"
message and throw on non-zero exit with the stderr content.

#### Scenario: successful script execution
Given the CloudPC is reachable via SSH
When `sshCloudPc("graph-teams.ps1", "-Action list")` is called
Then the function returns the stdout of the PowerShell script with noise lines removed.

#### Scenario: CloudPC unreachable
Given SSH to cloudpc fails with "Connection refused" or "timed out"
When `sshCloudPc(...)` is called
Then the function throws an error containing "CloudPC unreachable".

#### Scenario: script error
Given the PowerShell script exits with a non-zero status
When `sshCloudPc(...)` is called
Then the function throws an error containing the script's stderr output.

#### Scenario: noise filtering
Given the CloudPC script output contains lines with "WARNING:" and "security fix"
When `sshCloudPc(...)` is called
Then those noise lines are stripped from the returned string.

---

### Requirement: teams_list_chats handler
`src/tools/list-chats.ts` SHALL export a handler that calls
`sshCloudPc("graph-teams.ps1", "-Action list")` and returns the script output. It accepts an
optional `limit` parameter (clamped to 1-50, default 20).

#### Scenario: list chats
Given the CloudPC is reachable
When `teams_list_chats()` is called
Then the handler returns the CloudPC script's chat listing output.

#### Scenario: limit parameter
Given `limit: 5`
When `teams_list_chats(5)` is called
Then the SSH command includes the limit parameter.

---

### Requirement: teams_read_chat handler
`src/tools/read-chat.ts` SHALL export a handler that calls
`sshCloudPc("graph-teams.ps1", "-Action messages -ChatId '{chat_id}' -Count {limit}")`.
`chat_id` is required; `limit` defaults to 20 (clamped 1-50).

#### Scenario: read chat messages
Given a valid chat_id "19:abc@thread.v2"
When `teams_read_chat("19:abc@thread.v2", 10)` is called
Then the SSH command includes `-ChatId '19:abc@thread.v2' -Count 10`.

#### Scenario: missing chat_id
Given chat_id is empty or undefined
When the handler is called
Then it throws a validation error before invoking SSH.

---

### Requirement: teams_messages handler
`src/tools/messages.ts` SHALL export a handler that calls
`sshCloudPc("graph-teams.ps1", "-Action messages -TeamName '{team_name}' [-ChannelName '{channel_name}'] [-Count {count}]")`.
`team_name` is required; `channel_name` and `count` are optional.

#### Scenario: channel messages with all params
Given team_name "WholesaleIT", channel_name "Dev", count 10
When `teams_messages("WholesaleIT", "Dev", 10)` is called
Then the SSH command includes `-TeamName 'WholesaleIT' -ChannelName 'Dev' -Count 10`.

#### Scenario: channel messages defaults
Given only team_name "WholesaleIT"
When `teams_messages("WholesaleIT")` is called
Then the SSH command includes only `-TeamName 'WholesaleIT'` (General channel, default count).

---

### Requirement: teams_channels handler
`src/tools/channels.ts` SHALL export a handler that calls
`sshCloudPc("graph-teams.ps1", "-Action channels -TeamName '{team_name}'")`.
`team_name` is required.

#### Scenario: list channels
Given team_name "WholesaleIT"
When `teams_channels("WholesaleIT")` is called
Then the SSH command includes `-Action channels -TeamName 'WholesaleIT'`.

---

### Requirement: teams_presence handler
`src/tools/presence.ts` SHALL export a handler that calls
`sshCloudPc("graph-teams.ps1", "-Action presence -User '{user}'")`.
`user` (email/UPN or Azure AD object ID) is required.

#### Scenario: check presence
Given user "sarah@civalent.com"
When `teams_presence("sarah@civalent.com")` is called
Then the SSH command includes `-Action presence -User 'sarah@civalent.com'`.

---

### Requirement: teams_send handler
`src/tools/send.ts` SHALL export a handler that calls
`sshCloudPc("graph-teams.ps1", "-Action send -ChatId '{chat_id}' -Message '{message}'")`.
Both `chat_id` and `message` are required.

#### Scenario: send message
Given chat_id "19:abc@thread.v2" and message "Hello from Nova"
When `teams_send("19:abc@thread.v2", "Hello from Nova")` is called
Then the SSH command includes `-Action send -ChatId '19:abc@thread.v2' -Message 'Hello from Nova'`.

#### Scenario: missing message
Given chat_id is present but message is empty
When `teams_send("19:abc@thread.v2", "")` is called
Then the handler throws a validation error before invoking SSH.

---

### Requirement: Hono HTTP Server
`src/index.ts` SHALL create a Hono app listening on `PORT` (default 4005) with routes:
`GET /chats` (teams_list_chats), `GET /chats/:id` (teams_read_chat),
`GET /channels?team_name=` (teams_channels), `POST /search` (teams_messages),
`GET /presence?user=` (teams_presence), `POST /send` (teams_send),
`GET /health` (returns `{status: "ok", service: "teams-svc"}`).

Success responses SHALL be `{ok: true, data: <string>}`. Error responses SHALL be
`{ok: false, error: <string>}` with appropriate HTTP status codes (400, 502, 503, 500).

#### Scenario: health check
Given the service is running
When `GET /health` is called
Then it returns 200 with `{status: "ok", service: "teams-svc"}`.

#### Scenario: list chats via HTTP
Given the CloudPC is reachable
When `GET /chats?limit=5` is called
Then it returns 200 with `{ok: true, data: "..."}` containing the chat listing.

#### Scenario: missing required param
Given `GET /channels` is called without `team_name` query param
Then it returns 400 with `{ok: false, error: "team_name is required"}`.

#### Scenario: CloudPC unreachable
Given SSH to cloudpc fails
When any tool route is called
Then it returns 503 with `{ok: false, error: "CloudPC unreachable -- cannot connect via SSH"}`.

---

### Requirement: MCP Tool Definitions
`src/mcp.ts` SHALL define an MCP server with stdio transport exposing 6 tools:
`teams_list_chats`, `teams_read_chat`, `teams_messages`, `teams_channels`,
`teams_presence`, `teams_send`. Each tool definition includes name, description, and
JSON Schema for parameters matching the Rust tool definitions in
`crates/nv-daemon/src/tools/teams.rs`. Each MCP tool invokes the same handler as the
corresponding HTTP route.

#### Scenario: MCP tool count
The MCP server SHALL register exactly 6 tools.

#### Scenario: MCP tool execution
Given the MCP server receives a `teams_list_chats` tool call
When the handler executes
Then it calls `sshCloudPc("graph-teams.ps1", "-Action list")` and returns the result.

---

### Requirement: Package Configuration
`packages/tools/teams-svc/package.json` SHALL define `@nova/teams-svc` with scripts `build`,
`dev`, and `start`. `tsconfig.json` SHALL target Node 20 with strict mode and ESM module
resolution. Dependencies: `hono`, `pino`. Dev dependencies: `@types/node`, `esbuild`, `tsx`,
`typescript`.

#### Scenario: build produces dist
Given all source files are present
When `pnpm build` is run from `packages/tools/teams-svc/`
Then `dist/index.js` is produced.

#### Scenario: typecheck passes
Given all source files are present
When `pnpm typecheck` is run
Then it exits with code 0.
