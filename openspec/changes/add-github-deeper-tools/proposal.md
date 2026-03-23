# Proposal: Deeper GitHub Tools

## Change ID
`add-github-deeper-tools`

## Summary

Add four new read-only GitHub tools to the NV daemon that extend the existing `gh_pr_list`,
`gh_run_status`, and `gh_issues` surface: **`gh_pr_detail`** (PR metadata + review status + checks
+ diff stat), **`gh_pr_diff`** (actual diff content with optional file filter and truncation),
**`gh_releases`** (recent releases with notes), and **`gh_compare`** (compare two refs — commits +
diff stat). All tools shell out to the `gh` CLI, reuse the existing `exec_gh` / `validate_repo`
infrastructure in `github.rs`, and truncate large outputs to avoid blowing up Telegram messages or
Claude context.

## Context
- Extends: `crates/nv-daemon/src/github.rs` (exec_gh, validate_repo, types, formatters, tool definitions), `crates/nv-daemon/src/tools.rs` (dispatch match arms)
- Related: `gh_pr_list` already fetches summary-level PR data; `gh_pr_detail` and `gh_pr_diff` go deeper into a single PR. `gh_run_status` shows CI runs; `gh_compare` shows what changed between two refs.
- Depends on: nothing — standalone addition to existing GitHub module

## Motivation

The existing GitHub tools answer "what PRs are open?" and "what's the CI status?" but cannot answer
follow-up questions: "what does PR #42 actually change?", "did the reviewers approve?", "what was
in the last release?", or "what commits landed between v1.2 and v1.3?". These are the most common
second-order questions when Nova is used for project monitoring. Adding these four tools closes the
gap without leaving read-only territory.

## Requirements

### Req-1: `gh_pr_detail` — PR Metadata + Reviews + Checks + Diff Stat

Return comprehensive detail for a single PR.

**Parameters:**
| Param | Type | Required | Description |
|-------|------|----------|-------------|
| `repo` | string | yes | `owner/repo` format |
| `pr_number` | integer | yes | PR number |

**Data to fetch (via `gh pr view`):**
- Title, body (truncated to 2000 chars), state, author, created/updated dates
- Labels, assignees, milestone
- Review decision (APPROVED, CHANGES_REQUESTED, REVIEW_REQUIRED)
- Status checks summary (total, passing, failing, pending)
- Diff stat: files changed, additions, deletions (via `gh pr diff --stat`)

**Output format:** Structured text for Telegram, with sections separated by blank lines. Body
truncated with `[...truncated]` suffix. Diff stat as `N files changed, +A -D`.

### Req-2: `gh_pr_diff` — Actual Diff Content

Return the unified diff for a PR, optionally filtered to specific files.

**Parameters:**
| Param | Type | Required | Description |
|-------|------|----------|-------------|
| `repo` | string | yes | `owner/repo` format |
| `pr_number` | integer | yes | PR number |
| `file_filter` | string | no | Glob pattern to filter files (e.g. `*.rs`, `src/tools.rs`) |

**Behavior:**
- Fetch full diff via `gh pr diff`
- If `file_filter` is provided, keep only hunks whose file path matches the glob (simple `contains` or `ends_with` matching — no regex crate needed)
- Truncate final output to **10,000 characters** with `\n[...diff truncated at 10K chars]` suffix
- If the diff is empty after filtering, return `"No changes matching '{file_filter}' in PR #{pr_number}."`

### Req-3: `gh_releases` — Recent Releases

Return recent releases with tag, title, date, and notes summary.

**Parameters:**
| Param | Type | Required | Description |
|-------|------|----------|-------------|
| `repo` | string | yes | `owner/repo` format |
| `limit` | integer | no | Number of releases (default 5, max 20) |

**Data to fetch (via `gh release list` + `gh release view`):**
- Tag name, title, published date, isDraft, isPrerelease
- Release notes body (truncated to 500 chars per release)

**Output format:** One block per release. Notes truncated with `[...truncated]` suffix.

### Req-4: `gh_compare` — Compare Two Refs

Compare two branches, tags, or commits and return the commit list + diff stat.

**Parameters:**
| Param | Type | Required | Description |
|-------|------|----------|-------------|
| `repo` | string | yes | `owner/repo` format |
| `base` | string | yes | Base ref (branch, tag, or SHA) |
| `head` | string | yes | Head ref (branch, tag, or SHA) |

**Data to fetch (via `gh api repos/{owner}/{repo}/compare/{base}...{head}`):**
- Status (ahead, behind, diverged, identical)
- Total commits, ahead_by, behind_by
- Commit list: SHA (short), author, message (first line only), date
- Diff stat: files changed, additions, deletions

**Output format:** Summary line + commit list (truncated to 30 commits) + diff stat. Commit list
as `abc1234 author — message (date)` per line.

### Req-5: Truncation Helper

Add a shared `truncate_with_suffix(text, max_chars, suffix)` helper in `github.rs` to avoid
duplicating truncation logic across tools. Used by `gh_pr_detail` (body), `gh_pr_diff` (diff),
`gh_releases` (notes), and `gh_compare` (commit messages).

### Req-6: humanize_tool Mapping

The new tools should map to the existing `"Checking GitHub..."` description in `humanize_tool()`.
Add `gh_pr_detail`, `gh_pr_diff`, `gh_releases`, and `gh_compare` to the match arm alongside
`gh_pr_list | gh_run_status | gh_issues`.

## Scope
- **IN**: four new tool handlers in `github.rs`, four new tool definitions in `github_tool_definitions()`, four new dispatch arms in `tools.rs`, truncation helper, humanize_tool update, unit tests for parsing/formatting/truncation
- **OUT**: write operations (comment, merge, create release), webhook/event listeners, GitHub Apps authentication (stays with `gh` CLI auth), new module file (all additions fit in `github.rs`)

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/github.rs` | Add 4 handler functions, 4 tool definitions, serde types for new responses, `truncate_with_suffix` helper, Telegram formatters, unit tests |
| `crates/nv-daemon/src/tools.rs` | Add 4 dispatch match arms in both `execute_tool` paths |
| `crates/nv-daemon/src/orchestrator.rs` | Extend `humanize_tool` match arm to include new tool names |

## Risks
| Risk | Mitigation |
|------|-----------|
| Large diffs blow up Claude context or Telegram message limits | Hard truncation at 10K chars for `gh_pr_diff`, 2K for PR body, 500 per release notes. `truncate_with_suffix` ensures consistent behavior. |
| `gh api` for compare endpoint may return large JSON | Parse only needed fields (commits, files summary). Limit commit list to 30 entries. |
| `gh pr diff` can be slow on massive PRs | Existing `GH_TIMEOUT` (15s) applies. If it times out, the error is already actionable ("gh command timed out"). |
| `file_filter` glob matching without regex crate | Use simple string `contains` / `ends_with` matching — covers 95% of use cases (exact filename, extension). Document limitation. |
| `gh release view` requires one call per release (N+1) | Fetch release list first (JSON), then only call `gh release view` for the top N releases. Cap at 20. |
