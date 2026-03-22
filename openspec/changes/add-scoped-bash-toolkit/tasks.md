# Implementation Tasks

<!-- beads:epic:TBD -->

## API Batch

- [x] [2.1] [P-1] Add `projects: HashMap<String, PathBuf>` to config struct in nv-core/config.rs — maps project codes ("oo", "tc", etc.) to absolute paths [owner:api-engineer]
- [x] [2.2] [P-1] Add project path validation on config load — check each path exists, warn if missing [owner:api-engineer]
- [x] [2.3] [P-1] Create `crates/nv-daemon/src/bash.rs` module — define `AllowedCommand` enum with variants: GitStatus, GitLog, GitBranch, GitDiff, LsDir, CatConfig, BdReady, BdStats [owner:api-engineer]
- [x] [2.4] [P-1] Add `validate_project()` function in bash.rs — look up project code in registry, verify path exists, return PathBuf or error [owner:api-engineer]
- [x] [2.5] [P-1] Add `validate_subdir()` function in bash.rs — reject `..` components, canonicalize, verify stays within project root [owner:api-engineer]
- [x] [2.6] [P-1] Add `validate_config_file()` function in bash.rs — check extension is in allowlist (.json, .toml, .yaml, .yml, .md), reject `..` [owner:api-engineer]
- [x] [2.7] [P-1] Add `execute_command()` async function in bash.rs — match on AllowedCommand, construct `tokio::process::Command::new()` with args, capture stdout, 5s timeout, check exit code [owner:api-engineer]
- [x] [2.8] [P-1] Register 8 tool definitions in tools.rs register_tools() — git_status, git_log, git_branch, git_diff_stat, ls_project, cat_config, bd_ready, bd_stats with input schemas [owner:api-engineer]
- [x] [2.9] [P-1] Add execution handlers for all 8 tools in execute_tool() and execute_tool_send() — parse input JSON, validate project, call bash::execute_command(), return ToolResult::Immediate [owner:api-engineer]
- [x] [2.10] [P-2] Add `mod bash;` declaration to main.rs or lib.rs [owner:api-engineer]

## Verify

- [x] [3.1] cargo build passes [owner:api-engineer]
- [x] [3.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [x] [3.3] Unit test: validate_project() accepts known project, rejects unknown [owner:api-engineer]
- [x] [3.4] Unit test: validate_subdir() rejects `..` path traversal attempts [owner:api-engineer]
- [x] [3.5] Unit test: validate_config_file() accepts .json/.toml/.yaml/.yml/.md, rejects .rs/.sh [owner:api-engineer]
- [x] [3.6] Unit test: execute_command(GitStatus) returns expected format for a git repo [owner:api-engineer]
- [x] [3.7] Unit test: execute_command() respects 5s timeout [owner:api-engineer]
- [x] [3.8] Unit test: tool definitions registered correctly (8 new tools in register_tools()) [owner:api-engineer]
- [x] [3.9] Existing tests pass [owner:api-engineer]
