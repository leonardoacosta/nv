# Proposal: Add Tool Documentation

## Change ID
`add-tool-documentation`

## Summary

Add a `## CLI Tools` section to `config/system-prompt.md` documenting every Bash-invokable CLI tool available to Nova (teams-cli, outlook-cli, ado-cli, discord-cli, az, jira), so the Agent SDK can invoke them via Bash without asking permission for reads.

## Context
- Extends: `config/system-prompt.md` — operational rules file consumed by the TypeScript daemon via `systemPromptPath`
- Depends on: `add-teams-cli`, `add-ado-cli` (CLIs must be built before their docs are accurate)
- Related: `add-agent-sdk-integration` (system-prompt.md is loaded as `systemPromptPath`), `add-teams-cli`, `add-ado-cli`
- Phase 2, Wave 5 (Tool Wrappers) — final piece; adds the cognitive layer so Claude can route to CLI tools autonomously

## Motivation

The Agent SDK receives `config/system-prompt.md` as its system context. Without explicit CLI documentation, Claude will ask the operator before running shell commands or miss that these tools exist at all. Adding a `## CLI Tools` section that names each binary, lists its subcommands, shows example invocations, and describes expected output format gives the agent everything it needs to use Bash autonomously for reads — matching the "don't ask permission for reads" policy already in the system prompt.

## Requirements

### Req-1: CLI Tools Section Header

Add `## CLI Tools` as a new top-level section in `config/system-prompt.md`, placed after the existing `## Tool Use` section. Include a preamble:

> Use `Bash` to invoke these CLIs directly. Parse text output. Do not ask permission for read operations. All CLIs output UTF-8 text to stdout; errors go to stderr with a non-zero exit code.

#### Scenario: Section present
Given the updated `config/system-prompt.md`, the string `## CLI Tools` appears exactly once and comes after `## Tool Use`.

### Req-2: teams-cli Documentation

Document `teams-cli` — the MS Graph Teams CLI built in `packages/tools/teams-cli/`. Entry point installed to `~/.local/bin/teams-cli`. Auth via `MS_GRAPH_CLIENT_ID`, `MS_GRAPH_CLIENT_SECRET`, `MS_GRAPH_TENANT_ID` (injected by Doppler; already present at daemon runtime).

Subcommands to document:

| Subcommand | Args | Purpose |
|-----------|------|---------|
| `chats` | `[--limit N]` | List recent DMs and group chats |
| `read-chat <id>` | `[--limit N]` | Read messages from a chat thread |
| `channels <team-id>` | | List channels in a team |
| `messages <team-id> <channel-id>` | `[--limit N]` | Read channel messages |
| `presence <user>` | | Check user availability status |
| `send <id> <message>` | | Send a message to a chat or channel |

Each entry: command syntax, one-line description, example invocation, output format (plain text table or JSON lines).

#### Scenario: Agent routes to teams-cli
Given a user message "what did the team say in #general today?", the agent calls `Bash("teams-cli messages <team-id> <channel-id> --limit 20")` without asking for permission.

### Req-3: outlook-cli Documentation

Document `outlook-cli` — MS Graph mail CLI. Auth same as teams-cli (shared Graph API credentials). No subcommand list is defined yet at spec time; document based on the spec when `add-outlook-cli` is created. Placeholder until that spec lands:

Subcommands to document:

| Subcommand | Args | Purpose |
|-----------|------|---------|
| `inbox` | `[--limit N]` | List recent inbox messages |
| `read <id>` | | Read full email body by message ID |
| `calendar` | `[--days N]` | List upcoming calendar events |

#### Scenario: Agent reads inbox
Given a user query "any emails from HR?", the agent calls `Bash("outlook-cli inbox --limit 20")` and filters output, without escalating to the operator.

### Req-4: ado-cli Documentation

Document `ado-cli` — the Azure DevOps CLI built in `crates/ado-cli/`. Auth via `ADO_ORG` and `ADO_PAT` env vars (injected by Doppler).

Subcommands to document:

| Subcommand | Args | Purpose |
|-----------|------|---------|
| `pipelines <project>` | | List pipeline definitions |
| `builds <project>` | `[--limit N]` | List recent builds with status |
| `work-items <project>` | `[--type T] [--state S] [--limit N]` | Query work items |
| `run-pipeline <project> <id>` | `[--branch B]` | Trigger a pipeline run |

Output format: tab-separated table, one record per line.

#### Scenario: Agent queries build status
Given "is the deploy pipeline passing?", the agent calls `Bash("ado-cli builds MyProject --limit 5")` and summarizes the result.

### Req-5: discord-cli Documentation

Document `discord-cli` — Discord bot CLI built in `packages/tools/discord-cli/` (or the `relays/discord/bot.py` relay, whichever is invokable). Auth via `DISCORD_BOT_TOKEN`.

Subcommands to document:

| Subcommand | Args | Purpose |
|-----------|------|---------|
| `guilds` | | List servers the bot is in |
| `channels <guild-id>` | | List channels in a server |
| `read <channel-id>` | `[--limit N]` | Read recent messages from a channel |
| `send <channel-id> <message>` | | Send a message (confirm first) |

#### Scenario: Agent reads Discord channel
Given "what's the latest in #announcements?", the agent calls `Bash("discord-cli read <channel-id> --limit 10")` autonomously.

### Req-6: az (Azure CLI) Documentation

Document `az` — the Azure CLI already installed at `~/.local/bin/az`. Auth: `az login` with service principal via `AZURE_CLIENT_ID`, `AZURE_CLIENT_SECRET`, `AZURE_TENANT_ID`, or interactive token cache.

Common patterns to document:

| Pattern | Command | Purpose |
|---------|---------|---------|
| List resource groups | `az group list --output table` | Enumerate Azure resource groups |
| List VMs | `az vm list --output table` | List virtual machines |
| VM status | `az vm show -g <rg> -n <vm> --query powerState` | Check VM power state |
| Start/stop VM | `az vm start/stop -g <rg> -n <vm>` | Power operations (confirm first) |
| List subscriptions | `az account list --output table` | Enumerate subscriptions |
| Set subscription | `az account set --subscription <id>` | Switch active subscription |
| CloudPC list | `az desktopvirtualization hostpool list --output table` | List Cloud PCs |

Note: `az` operations that modify resources (start, stop, deallocate, delete) require operator confirmation before execution.

#### Scenario: Agent checks VM status
Given "is the CloudPC running?", the agent calls `Bash("az vm list --output table")` to enumerate and then checks power state.

### Req-7: jira (if available) Documentation

Document Jira access patterns. Note: no standalone `jira` binary is installed. The daemon uses `jira_search` / `jira_get` / `jira_create` / etc. as native tools (not CLI). Document this explicitly so the agent does not attempt `Bash("jira ...")`:

> Jira is accessed via native tools (`jira_search`, `jira_get`, `jira_create`, `jira_transition`, `jira_assign`, `jira_comment`), not via Bash. Do not use `Bash` for Jira operations.

#### Scenario: Agent uses correct Jira path
Given "what Jira tickets are open in OO?", the agent calls `jira_search` (native tool), not `Bash("jira ...")`.

## Scope

- **IN**: `config/system-prompt.md` — new `## CLI Tools` section only
- **OUT**: Any changes to the CLI binaries themselves, any changes to daemon tool registration, any new CLI binary creation, changes to `config/identity.md` / `config/soul.md` / `config/user.md`

## Impact

| Area | Change |
|------|--------|
| `config/system-prompt.md` | Add `## CLI Tools` section (~60-80 lines) after `## Tool Use` |

## Risks

| Risk | Mitigation |
|------|-----------|
| CLIs not yet built when doc is applied | Docs are non-functional but harmless; agent will get `command not found` errors until CLIs land — acceptable, spec is applied last in wave |
| Subcommand flags change after doc is written | Keep examples minimal; flag names are stable contracts — update docs alongside CLI spec changes |
| `az` write operations performed without confirmation | Explicit note in docs: "operations that modify resources require operator confirmation" |
| outlook-cli subcommands not yet finalized | Placeholder documented with TBD note; update when `add-outlook-cli` spec is written |
