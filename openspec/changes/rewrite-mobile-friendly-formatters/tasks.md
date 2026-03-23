# Implementation Tasks

<!-- beads:epic:TBD -->

## Shared Helpers

- [x] [0.1] [P-1] Add shared `relative_time(timestamp: &str) -> String` helper to `tools/mod.rs` (or new `tools/fmt.rs`) — parses ISO 8601 / RFC 3339, returns "5m ago" / "3h ago" / "Mar 15" etc. [owner:api-engineer]
- [x] [0.2] [P-2] Add unit tests for relative_time: <1min, minutes, hours, days, >7d, invalid input [owner:api-engineer]

## Batch 1: ADO + GitHub (CI/CD domain)

- [x] [1.1] [P-1] Rewrite `ado.rs` — format_projects, format_pipelines, format_builds to list-based format with 📁/🔄/🏗️ emoji [owner:api-engineer]
- [x] [1.2] [P-1] Rewrite `github.rs` — PrSummary, RunSummary, IssueSummary format_for_telegram methods to list-based format [owner:api-engineer]
- [x] [1.3] [P-2] Rewrite `github.rs` — PrDetail, ReleaseSummary, CompareResult format_for_telegram methods (longer multi-line formats) [owner:api-engineer]
- [x] [1.4] [P-2] Update all tests in ado.rs and github.rs to match new format strings [owner:api-engineer]

## Batch 2: Infrastructure (Vercel + Cloudflare + Docker + Neon)

- [x] [2.1] [P-1] Rewrite `vercel.rs` — Deployment::format_for_telegram, format_deployments_for_telegram, format_build_logs_for_telegram [owner:api-engineer]
- [x] [2.2] [P-1] Rewrite `cloudflare.rs` — inline formatters in cf_zones, cf_dns_records, cf_domain_status [owner:api-engineer]
- [x] [2.3] [P-1] Rewrite `docker.rs` — inline table formatter in docker_ps to list-based format [owner:api-engineer]
- [x] [2.4] [P-1] Rewrite `neon.rs` — format_results (multi-row only), format_projects, format_branches, format_endpoints; remove format_table helper [owner:api-engineer]
- [x] [2.5] [P-2] Update all tests in vercel.rs, cloudflare.rs, docker.rs, neon.rs [owner:api-engineer]

## Batch 3: Secrets + Monitoring (Doppler + Sentry + PostHog + Check)

- [x] [3.1] [P-1] Rewrite `doppler.rs` — inline formatters for secrets list, compare, activity [owner:api-engineer]
- [x] [3.2] [P-1] Rewrite `sentry.rs` — IssueSummary and IssueDetail format_for_telegram methods (leave format_stack_trace unchanged) [owner:api-engineer]
- [x] [3.3] [P-1] Rewrite `posthog.rs` — format_trends, format_flags [owner:api-engineer]
- [x] [3.4] [P-1] Add `format_telegram()` to `check.rs` — new function (does not replace format_terminal or format_json) [owner:api-engineer]
- [x] [3.5] [P-2] Update all tests in doppler.rs, sentry.rs, posthog.rs, check.rs [owner:api-engineer]

## Batch 4: Comms + Financial (Stripe + Resend + Plaid + Teams + Jira)

- [x] [4.1] [P-1] Rewrite `stripe.rs` — Customer and Invoice format_for_telegram methods (refine, not full rewrite — already close) [owner:api-engineer]
- [x] [4.2] [P-1] Rewrite `resend.rs` — format_emails, format_bounces [owner:api-engineer]
- [x] [4.3] [P-1] Rewrite `plaid.rs` — format_balances, format_bills [owner:api-engineer]
- [x] [4.4] [P-1] Rewrite `teams.rs` — inline channel list and message formatters [owner:api-engineer]
- [x] [4.5] [P-1] Rewrite `jira/tools.rs` — format_issues_for_claude, format_issue_for_claude [owner:api-engineer]
- [x] [4.6] [P-2] Update `jira/webhooks.rs` — 3 inline alert format strings to use emoji prefix [owner:api-engineer]
- [x] [4.7] [P-2] Update all tests in stripe.rs, resend.rs, plaid.rs, teams.rs, jira/ [owner:api-engineer]

## Batch 5: Remaining (HA + Upstash + Calendar + Web)

- [x] [5.1] [P-1] Rewrite `ha.rs` — format_states, format_entity [owner:api-engineer]
- [x] [5.2] [P-1] Rewrite `upstash.rs` — format_info, format_keys [owner:api-engineer]
- [x] [5.3] [P-1] Rewrite `calendar.rs` — format_event (leave format_event_time and format_attendees helpers as-is) [owner:api-engineer]
- [x] [5.4] [P-1] Rewrite `web.rs` — inline search result formatter [owner:api-engineer]
- [x] [5.5] [P-2] Update all tests in ha.rs, upstash.rs, calendar.rs, web.rs [owner:api-engineer]

## Verify

- [ ] [6.1] `cargo build` passes [owner:api-engineer]
- [ ] [6.2] `cargo clippy -- -D warnings` passes [owner:api-engineer]
- [ ] [6.3] `cargo test` — all existing tests pass with updated assertions, no test count decrease [owner:api-engineer]
- [ ] [6.4] [user] Manual test: send tool queries via Telegram on mobile, verify formatting is compact and readable
