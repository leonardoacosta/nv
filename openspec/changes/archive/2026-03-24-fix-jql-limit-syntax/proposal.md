# Proposal: Fix JQL Limit Syntax

## Change ID
`fix-jql-limit-syntax`

## Summary

Strip invalid `LIMIT N` clauses from JQL queries before sending them to the Jira API, and update
the `jira_search` tool description to tell Claude that JQL does not support `LIMIT` and that
result count is controlled by `maxResults`.

## Context
- Extends: `crates/nv-daemon/src/tools/jira/client.rs` (search method)
- Extends: `crates/nv-daemon/src/tools/jira/tools.rs` (tool definition description)
- Related: beads nv-9vt (P1)

## Problem

Claude frequently appends SQL-style `LIMIT N` to JQL queries (e.g.
`project = OO ORDER BY created DESC LIMIT 10`). JQL does not have a `LIMIT` keyword -- pagination
is controlled by the `maxResults` query parameter on the REST API. The Jira API returns HTTP 400
for any JQL containing `LIMIT`, causing the search to fail with a parse error.

The root cause is twofold:

1. **Tool description gap** -- the `jira_search` tool definition says only "JQL query string" for
   the `jql` parameter, giving Claude no indication that `LIMIT` is invalid in JQL.
2. **No input sanitization** -- `client.rs::search()` passes the raw JQL string through to the API
   without stripping known-invalid syntax.

## Solution

### Fix 1: Sanitize JQL input (defensive)

Add a `sanitize_jql()` function in `client.rs` that strips `LIMIT \d+` (case-insensitive) from the
end of the JQL string before sending it to the API. If a numeric value is found, use it to override
the default `maxResults` query parameter (capped at 100 to avoid API abuse).

```
Input:  "project = OO ORDER BY created DESC LIMIT 10"
Output: jql = "project = OO ORDER BY created DESC", maxResults = 10
```

### Fix 2: Improve tool description (preventive)

Update the `jira_search` tool definition's `jql` parameter description to explicitly state:

> "JQL query string. Do NOT use LIMIT -- result count is controlled automatically (max 50).
> Example: project = OO AND status != Done ORDER BY created DESC"

This tells Claude at tool-definition time that `LIMIT` is not valid JQL.

## Scope
- **IN**: JQL sanitization function, tool description update, unit tests
- **OUT**: Pagination support (startAt), custom maxResults parameter on the tool schema

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/tools/jira/client.rs` | Add `sanitize_jql()`, call it in `search()`, pass extracted limit as `maxResults` |
| `crates/nv-daemon/src/tools/jira/tools.rs` | Update `jira_search` tool description for the `jql` field |

## Risks
| Risk | Mitigation |
|------|-----------|
| Regex strips a legitimate JQL field named "limit" | The regex anchors to end-of-string with `\s+LIMIT\s+\d+\s*$` -- JQL field names appear before operators, never at the trailing position |
| Claude still ignores the description | The sanitizer is the real fix; the description is belt-and-suspenders |
