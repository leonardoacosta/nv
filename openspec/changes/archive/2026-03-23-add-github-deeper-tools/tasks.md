# Implementation Tasks

<!-- beads:epic:TBD -->

## Truncation Helper

- [x] [1.1] [P-1] Add `truncate_with_suffix(text: &str, max_chars: usize, suffix: &str) -> String` helper in github.rs — returns text unchanged if within limit, otherwise truncates at char boundary and appends suffix [owner:api-engineer]
- [x] [1.2] [P-2] Add tests for truncation: under limit unchanged, at limit unchanged, over limit truncated with suffix, empty input, multi-byte UTF-8 boundary safety [owner:api-engineer]

## gh_pr_detail

- [x] [2.1] [P-1] Add `PrDetail` serde struct in github.rs — title, body, state, author, labels, assignees, milestone, reviewDecision, statusCheckRollup, createdAt, updatedAt [owner:api-engineer]
- [x] [2.2] [P-1] Add `gh_pr_detail(repo, pr_number)` handler — calls `gh pr view` with `--json` fields, calls `gh pr diff --stat` for diff stat, truncates body to 2000 chars, formats structured output [owner:api-engineer]
- [x] [2.3] [P-1] Add `ToolDefinition` for `gh_pr_detail` in `github_tool_definitions()` — params: repo (string, required), pr_number (integer, required) [owner:api-engineer]
- [x] [2.4] [P-1] Add dispatch match arm for `gh_pr_detail` in tools.rs — extract repo and pr_number from input, call handler [owner:api-engineer]
- [x] [2.5] [P-2] Add Telegram formatter for `PrDetail` — sections: header, review status, checks summary, diff stat, body excerpt [owner:api-engineer]
- [x] [2.6] [P-2] Add parse + format tests for `gh_pr_detail` — valid JSON, missing optional fields, body truncation, diff stat parsing [owner:api-engineer]

## gh_pr_diff

- [x] [3.1] [P-1] Add `gh_pr_diff(repo, pr_number, file_filter)` handler — calls `gh pr diff`, applies optional file_filter via simple string matching on diff headers, truncates to 10K chars [owner:api-engineer]
- [x] [3.2] [P-1] Add `ToolDefinition` for `gh_pr_diff` in `github_tool_definitions()` — params: repo (required), pr_number (required), file_filter (optional) [owner:api-engineer]
- [x] [3.3] [P-1] Add dispatch match arm for `gh_pr_diff` in tools.rs — extract repo, pr_number, optional file_filter [owner:api-engineer]
- [x] [3.4] [P-2] Add tests for `gh_pr_diff` — truncation at 10K boundary, file_filter matching (extension, exact name, no match), empty diff [owner:api-engineer]

## gh_releases

- [x] [4.1] [P-1] Add `ReleaseSummary` serde struct in github.rs — tagName, name, publishedAt, isDraft, isPrerelease, body [owner:api-engineer]
- [x] [4.2] [P-1] Add `gh_releases(repo, limit)` handler — calls `gh release list --json` with limit clamped to 1..=20 (default 5), truncates each release body to 500 chars, formats output [owner:api-engineer]
- [x] [4.3] [P-1] Add `ToolDefinition` for `gh_releases` in `github_tool_definitions()` — params: repo (required), limit (optional integer, default 5) [owner:api-engineer]
- [x] [4.4] [P-1] Add dispatch match arm for `gh_releases` in tools.rs — extract repo, optional limit [owner:api-engineer]
- [x] [4.5] [P-2] Add Telegram formatter for `ReleaseSummary` — tag, title, date, draft/prerelease badges, notes excerpt [owner:api-engineer]
- [x] [4.6] [P-2] Add parse + format tests for `gh_releases` — valid JSON, empty list, body truncation, limit clamping [owner:api-engineer]

## gh_compare

- [x] [5.1] [P-1] Add `CompareResult` and `CompareCommit` serde structs in github.rs — status, ahead_by, behind_by, total_commits, commits (sha, author, message, date), files summary (total, additions, deletions) [owner:api-engineer]
- [x] [5.2] [P-1] Add `gh_compare(repo, base, head)` handler — calls `gh api repos/{owner}/{repo}/compare/{base}...{head}`, parses response, truncates commit list to 30 entries, formats output [owner:api-engineer]
- [x] [5.3] [P-1] Add `ToolDefinition` for `gh_compare` in `github_tool_definitions()` — params: repo (required), base (required), head (required) [owner:api-engineer]
- [x] [5.4] [P-1] Add dispatch match arm for `gh_compare` in tools.rs — extract repo, base, head [owner:api-engineer]
- [x] [5.5] [P-2] Add Telegram formatter for `CompareResult` — summary line (status, ahead/behind), commit list, diff stat [owner:api-engineer]
- [x] [5.6] [P-2] Add parse + format tests for `gh_compare` — valid JSON, identical refs, diverged status, commit truncation at 30 [owner:api-engineer]

## Integration

- [x] [6.1] [P-1] Update `humanize_tool` match arm in orchestrator.rs — add `gh_pr_detail`, `gh_pr_diff`, `gh_releases`, `gh_compare` alongside existing GitHub tools [owner:api-engineer]
- [x] [6.2] [P-2] Update `github_tool_definitions_returns_three_tools` test to expect 7 tools and assert new tool names present [owner:api-engineer]

## Verify

- [x] [7.1] `cargo build` passes [owner:api-engineer]
- [x] [7.2] `cargo clippy -- -D warnings` passes [owner:api-engineer]
- [x] [7.3] `cargo test` — existing tests pass, new tests for all 4 tools + truncation helper [owner:api-engineer]
