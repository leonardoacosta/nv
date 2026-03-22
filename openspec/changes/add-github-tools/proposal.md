# Proposal: Add GitHub Tools

## Change ID
`add-github-tools`

## Summary

GitHub data source via the `gh` CLI (already authenticated on the homelab). Three tools
exposing PR status, CI run status, and issue lists — all read-only, parsed from JSON output,
formatted for Telegram delivery.

## Context
- Extends: `crates/nv-daemon/src/tools.rs` (tool definitions), `crates/nv-daemon/src/agent.rs` (tool dispatch)
- Related: PRD Phase 2 "Data Sources — Zero Auth" (Tier 1), `add-tool-audit-log` spec (audit logging dependency)
- Auth: None — `gh` CLI uses existing `~/.config/gh/hosts.yml` token

## Motivation

GitHub is the single source of truth for code status across all 20+ projects. Currently,
checking PR status, CI results, or open issues requires opening a browser or running CLI
commands manually. Wiring `gh` into Nova enables:

1. **Instant project health checks** — "Are there any failing CI runs on OO?"
2. **Aggregation layer input** — `project_health(code)` needs GitHub CI + PR data
3. **Proactive alerts** — digest can include "3 PRs waiting for review across projects"
4. **Zero auth overhead** — `gh` is already authenticated, no API keys needed

## Requirements

### Req-1: gh_pr_list Tool

```
gh_pr_list(repo: String) -> Vec<PrSummary>
```

Shells out to: `gh pr list --repo {repo} --json number,title,state,author,updatedAt,mergeable --limit 20`

Returns structured list of open PRs with number, title, state, author login, last update,
and mergeable status. Formatted for Telegram as a condensed list.

### Req-2: gh_run_status Tool

```
gh_run_status(repo: String) -> Vec<RunSummary>
```

Shells out to: `gh run list --repo {repo} --json databaseId,displayTitle,status,conclusion,event,headBranch,updatedAt --limit 10`

Returns recent CI/CD workflow runs with status (completed/in_progress/queued),
conclusion (success/failure/cancelled), branch, and trigger event. Failed runs
highlighted with status emoji for Telegram.

### Req-3: gh_issues Tool

```
gh_issues(repo: String) -> Vec<IssueSummary>
```

Shells out to: `gh issue list --repo {repo} --json number,title,state,labels,assignees,updatedAt --limit 20`

Returns open issues with number, title, label names, assignee logins, and last update.

### Req-4: Tool Registration

All three tools registered in `tools.rs` with:
- Tool name and description for Claude's tool-use schema
- Input validation (repo must match `owner/repo` format)
- JSON parse error handling (if `gh` output is malformed or binary not found)
- Audit logging via tool_usage table (requires `add-tool-audit-log`)

### Req-5: Shell Execution Pattern

Use `tokio::process::Command` for async subprocess execution:
- Set `--json` flag on all `gh` commands for structured output
- Capture stdout, parse as `serde_json::Value`, map to typed structs
- Timeout: 15s per `gh` invocation (network latency for API calls)
- If `gh` binary not found: return tool error "gh CLI not installed"
- If `gh` auth expired: detect "auth login" in stderr, return actionable error

## Scope
- **IN**: Three read-only tools (pr_list, run_status, issues), JSON parsing, error handling, audit logging, Telegram formatting
- **OUT**: Write operations (create PR, merge, close issue), webhook listeners, caching layer, repo creation

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/tools.rs` | Add gh_pr_list, gh_run_status, gh_issues tool definitions + dispatch handlers |
| `crates/nv-daemon/src/agent.rs` | Register new tools in available_tools list |
| `crates/nv-daemon/src/github.rs` | New: GitHub module with exec_gh(), parse helpers, typed structs |
| `crates/nv-daemon/src/main.rs` | Add `mod github;` declaration |

## Risks
| Risk | Mitigation |
|------|-----------|
| `gh` CLI not installed on target | Graceful error: "gh CLI not found — install with `pacman -S github-cli`" |
| Rate limiting (5000 req/hr) | Unlikely at single-user scale; add header check if needed later |
| `gh` auth token expired | Detect "auth login" in stderr, return clear error to user |
| Slow network to GitHub API | 15s timeout, return partial data if available |
