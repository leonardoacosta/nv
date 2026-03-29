# Spec: SSH Command Tool

## ADDED Requirements

### Requirement: Generic SSH command execution
The `ssh_command` tool MUST accept a `command` string parameter and execute it on the CloudPC via the existing `sshCloudPC()` function. No prefix validation, no metacharacter stripping. The raw command string is passed directly to the remote shell.

#### Scenario: Run PowerShell command
Given a call to `ssh_command` with `{"command": "Get-Process | Sort CPU -Desc | Select -First 10"}`,
When executed on CloudPC via SSH,
Then the tool returns the PowerShell output as text.

#### Scenario: Run diagnostic command
Given a call to `ssh_command` with `{"command": "ipconfig /all"}`,
When executed on CloudPC via SSH,
Then the tool returns the full network configuration output.

#### Scenario: SSH failure
Given the CloudPC is unreachable,
When `ssh_command` is called,
Then the tool returns the actual error message (unreachable, timeout, or command stderr).

### Requirement: Dual transport registration
The `ssh_command` tool MUST be registered in both MCP (stdio) and HTTP transports, following the same pattern as `azure_cli`.

#### Scenario: MCP tool discovery
Given the agent lists available tools,
When the tool list includes `nova-azure` server tools,
Then `ssh_command` appears alongside `azure_cli`.

#### Scenario: HTTP endpoint
Given a `POST /ssh` request with `{"command": "hostname"}`,
When the request is processed,
Then the response contains the CloudPC hostname.

### Requirement: System prompt awareness
The daemon system prompt MUST mention `ssh_command` so Nova knows it can run arbitrary commands on CloudPC.

#### Scenario: Nova uses ssh_command
Given a user asks "what processes are running on CloudPC",
When Nova processes the request,
Then Nova calls `ssh_command` with a `Get-Process` command rather than saying it lacks access.
