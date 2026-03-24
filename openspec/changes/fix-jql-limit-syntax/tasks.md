# Implementation Tasks

<!-- beads:epic:nv-9vt -->

## API Batch

- [ ] [2.1] [P-1] Add `sanitize_jql(jql: &str) -> (String, Option<u32>)` function in `client.rs` -- strip trailing `LIMIT \d+` (case-insensitive regex), return cleaned JQL and optional extracted limit value capped at 100 [owner:api-engineer]
- [ ] [2.2] [P-1] Update `search()` in `client.rs` to call `sanitize_jql()` before building the request -- use extracted limit as `maxResults` if present, otherwise keep default 50 [owner:api-engineer]
- [ ] [2.3] [P-2] Update `jira_search` tool definition description in `tools.rs` -- change `jql` field description to: "JQL query string. Do NOT use LIMIT -- result count is controlled automatically (max 50). Example: project = OO AND status != Done ORDER BY created DESC" [owner:api-engineer]

## Verify

- [ ] [3.1] Unit test: `sanitize_jql("project = OO LIMIT 10")` returns `("project = OO", Some(10))` [owner:api-engineer]
- [ ] [3.2] Unit test: `sanitize_jql("project = OO ORDER BY created DESC LIMIT 25")` returns `("project = OO ORDER BY created DESC", Some(25))` [owner:api-engineer]
- [ ] [3.3] Unit test: `sanitize_jql("project = OO ORDER BY created DESC limit 5")` handles case-insensitive match [owner:api-engineer]
- [ ] [3.4] Unit test: `sanitize_jql("project = OO")` returns `("project = OO", None)` -- no LIMIT present, passes through unchanged [owner:api-engineer]
- [ ] [3.5] Unit test: `sanitize_jql("project = OO LIMIT 999")` caps extracted value at 100 [owner:api-engineer]
- [ ] [3.6] `cargo build` passes [owner:api-engineer]
- [ ] [3.7] `cargo clippy -- -D warnings` passes [owner:api-engineer]
- [ ] [3.8] Existing tests pass (`cargo test -p nv-daemon`) [owner:api-engineer]
