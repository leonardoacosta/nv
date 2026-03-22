# Proposal: Scoped Bash Toolkit

## Change ID
`add-scoped-bash-toolkit`

## Summary

Allowlisted read-only shell commands per project, executed in Rust via `Command::new()`.
Supports: git status/log/branch/diff, ls, cat (config files), bd ready/stats. ~10ms execution.
No write operations without PendingAction confirmation.

## Context
- Extends: `crates/nv-daemon/src/tools.rs` (register_tools, execute_tool)
- Extends: `crates/nv-core/src/config.rs` (project paths configuration)
- Related: PRD §5.3 (Scoped Bash Toolkit)

## Motivation

Nova currently has no way to answer project-level questions without calling Claude. Simple queries
like "what branch is OO on?" or "what's ready on TC?" require a full Claude API round-trip. With
scoped bash tools, Nova can answer these in ~10ms by running allowlisted commands directly:

1. **Speed** — `Command::new("git").arg("status")` returns in milliseconds, no AI needed
2. **Cost** — zero tokens consumed for routine status checks
3. **Safety** — strict allowlist prevents any write operations; project paths scoped to `~/dev/`
4. **Foundation** — enables Nexus integration (spec-20) to answer project queries remotely

## Requirements

### Req-1: Project Path Registry

Add `projects` field to config (or use existing project registry from agent config). Map project
codes to paths: `{ "oo": "~/dev/oo", "tc": "~/dev/tc", ... }`. Validate paths exist on startup.

### Req-2: Command Allowlist

Define an enum of allowed commands with their argument patterns:

| Command | Allowlist Pattern | Args |
|---------|------------------|------|
| `git_status` | `git -C {path} status --short` | project |
| `git_log` | `git -C {path} log --oneline -N` | project, count (max 20) |
| `git_branch` | `git -C {path} branch --show-current` | project |
| `git_diff` | `git -C {path} diff --stat` | project |
| `ls_dir` | `ls {path}/{subdir}` | project, subdir (validated) |
| `cat_config` | `cat {path}/{file}` | project, file (allowlisted extensions: .json, .toml, .yaml, .yml, .md) |
| `bd_ready` | `bd -C {path} ready --json` | project |
| `bd_stats` | `bd -C {path} stats --json` | project |

### Req-3: Rust Execution

Execute via `tokio::process::Command::new()` with:
- Working directory set to project path
- Timeout of 5 seconds (commands should return in <100ms)
- stdout captured, stderr logged
- Exit code checked — non-zero returns error message to Claude

No shell invocation (`sh -c`). Direct binary execution only.

### Req-4: Tool Definitions

Register 8 tools with Claude via `register_tools()`:
- `git_status(project)` — short status
- `git_log(project, count?)` — recent commits
- `git_branch(project)` — current branch name
- `git_diff_stat(project)` — diff summary
- `ls_project(project, subdir?)` — list directory contents
- `cat_config(project, file)` — read config file
- `bd_ready(project)` — beads ready queue
- `bd_stats(project)` — beads statistics

Each tool validates the `project` argument against the registry before execution.

### Req-5: Security Constraints

- Project paths must be under `~/dev/` — reject any path outside
- Subdir arguments cannot contain `..` — reject path traversal
- Cat only allows specific extensions (.json, .toml, .yaml, .yml, .md)
- No write commands without PendingAction confirmation (not in this spec)
- All executions logged to tool_usage table (when audit log spec is applied)

## Scope
- **IN**: 8 read-only tools, project path registry, allowlist enforcement, Command::new() execution
- **OUT**: Write commands, interactive commands, piped commands, sudo

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/tools.rs` | Add 8 tool definitions to register_tools(), add execution handlers to execute_tool/execute_tool_send |
| `crates/nv-core/src/config.rs` | Add project path registry (HashMap<String, PathBuf>) to config |
| `crates/nv-daemon/src/bash.rs` | New module: allowlist validation, Command::new() execution, output capture |

## Risks
| Risk | Mitigation |
|------|-----------|
| Path traversal via subdir argument | Validate no `..` components; canonicalize and check prefix |
| Command injection via argument | No shell invocation; args passed directly to Command::new() |
| bd CLI not installed on system | Check `which bd` on startup; disable bd tools if missing |
| Stale project paths | Validate on startup; return clear error if path doesn't exist at call time |
