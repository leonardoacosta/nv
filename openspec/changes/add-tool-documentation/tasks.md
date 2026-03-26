# Implementation Tasks

<!-- beads:epic:nv-5nt9 -->

## Documentation Batch

- [ ] [1.1] [P-1] Read current `config/system-prompt.md` and identify insertion point after `## Tool Use` section [owner:api-engineer] [beads:nv-cp8u]
- [ ] [1.2] [P-1] Add `## CLI Tools` preamble paragraph to `config/system-prompt.md` explaining Bash invocation, read-permission policy, and stdout/stderr conventions [owner:api-engineer] [beads:nv-cp8u]
- [ ] [1.3] [P-1] Add `### teams-cli` subsection: table of six subcommands (chats, read-chat, channels, messages, presence, send), one example invocation per subcommand, note on auth env vars (`MS_GRAPH_CLIENT_ID`, `MS_GRAPH_CLIENT_SECRET`, `MS_GRAPH_TENANT_ID`) [owner:api-engineer] [beads:nv-biw8]
- [ ] [1.4] [P-1] Add `### outlook-cli` subsection: table of three subcommands (inbox, read, calendar), one example per subcommand, same Graph API auth note [owner:api-engineer] [beads:nv-biw8]
- [ ] [1.5] [P-1] Add `### ado-cli` subsection: table of four subcommands (pipelines, builds, work-items, run-pipeline), one example per subcommand, note on `ADO_ORG` and `ADO_PAT` auth, tab-separated output format [owner:api-engineer] [beads:nv-biw8]
- [ ] [1.6] [P-1] Add `### discord-cli` subsection: table of four subcommands (guilds, channels, read, send), one example per subcommand, note on `DISCORD_BOT_TOKEN` auth, note that `send` requires confirmation [owner:api-engineer] [beads:nv-biw8]
- [ ] [1.7] [P-1] Add `### az (Azure CLI)` subsection: table of seven common patterns (group list, vm list, vm show, vm start/stop, account list, account set, CloudPC list), explicit note that resource-modifying operations require operator confirmation [owner:api-engineer] [beads:nv-biw8]
- [ ] [1.8] [P-1] Add `### jira` subsection: explicit note that Jira uses native tools (jira_search, jira_get, etc.) and NOT Bash — agent must not attempt `Bash("jira ...")` [owner:api-engineer] [beads:nv-biw8]

## Validation Batch

- [ ] [2.1] Verify `## CLI Tools` section appears exactly once and after `## Tool Use` in `config/system-prompt.md` using `grep -n "## CLI Tools\|## Tool Use" config/system-prompt.md` [owner:api-engineer] [beads:nv-tzxr]
- [ ] [2.2] Verify all seven CLI tool entries are present: `grep -c "### " config/system-prompt.md` returns >= 7 new headers [owner:api-engineer] [beads:nv-tzxr]
- [ ] [2.3] [user] Manual smoke test: restart TypeScript daemon with updated system prompt; send "list my Teams chats" — Nova invokes `teams-cli chats` via Bash without asking permission [beads:nv-tzxr]
