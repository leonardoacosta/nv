# Proposal: Rewrite Mobile-Friendly Formatters

## Change ID
`rewrite-mobile-friendly-formatters`

## Summary

Rewrite all `format_*()` and `format_for_telegram()` functions across every tool module to use a
compact, list-based format instead of ASCII-aligned tables. Current column-aligned output with
dashes and fixed-width padding wraps badly on Telegram mobile, making tool results unreadable.

## Context
- Extends: All `crates/nv-daemon/src/tools/*.rs` modules (17 files + jira subdir)
- Related: Every tool module was independently written with aligned-table or pipe-delimited formatting
- No upstream dependency — this is a cosmetic rewrite of string output only

## Motivation

Nova sends all tool results to Telegram. Every format function today produces ASCII tables with
fixed-width columns, dash separators, and padded spacing:

```
Name        Image      State    Uptime  Ports
-----------+---------+--------+-------+-------
redis       redis:7    running  3 days  6379
```

On Telegram mobile (where 90% of messages are read), columns break and wrap at arbitrary points,
dashes create visual noise, and the information hierarchy is lost. The target is a compact
list-based format that works on any screen width:

```
🐳 redis (redis:7) — running
   Uptime: 3 days | Ports: 6379
```

This format:
1. **Never breaks on narrow screens** — each item is self-contained, no column alignment
2. **Uses emoji for scanability** — domain-specific icons let you parse at a glance
3. **Preserves information density** — same data, fewer characters, better hierarchy
4. **Standardizes empty states** — "No {items} found." instead of "(no X found)" variations

## Requirements

### Req-1: List-Based Format Pattern

Every format function must produce output following this pattern per item:

```
{emoji} **{primary_field}** — {secondary_field}
   {detail line 1}
   {detail line 2 if needed}
```

For key-value detail sections: `**{key}:** {value}` on a single line.
For status indicators: `✅` healthy, `⚠️` degraded, `❌` error, `⏸` disabled.
For timestamps: relative when <7 days ("2h ago"), date when older ("Mar 15").
For empty lists: `"No {items} found."` (not `"(no X found)"`).
For counts: inline `"(3 items)"`, not a separate row.

### Req-2: Domain Emoji Assignments

| Domain | Emoji | Modules |
|--------|-------|---------|
| Projects/repos | 📁 | ado, github, neon |
| Pipelines/CI | 🔄 | ado, github |
| Builds/deploys | 🏗️ | ado, vercel |
| Search results | 🔍 | web |
| Financial | 💰 | stripe, plaid |
| Email | 📧 | resend |
| Secrets | 🔐 | doppler |
| DNS/domains | 🌐 | cloudflare |
| Analytics | 📊 | posthog |
| Errors | 🐛 | sentry |
| Home | 🏠 | ha |
| Calendar | 📅 | calendar |
| Messages | 💬 | teams, jira webhooks |
| Redis keys | 🔑 | upstash |
| Database | 🗃️ | neon |
| Containers | 🐳 | docker |
| Health check | ✅/⚠️/❌ | check |

### Req-3: Module-Level Rewrites

Each module's format functions must be rewritten. The complete inventory:

**ado.rs** (3 functions):
- `format_projects()` — `📁 {name} ({state})\n   Last updated: {date}`
- `format_pipelines()` — `🔄 [{id}] {name}\n   Folder: {folder}`
- `format_builds()` — `🏗️ #{number} {status_icon} {result} — {branch}\n   By {requester} | Queued: {queued} | Finished: {finished}`

**github.rs** (6 methods):
- `PrSummary::format_for_telegram()` — `📁 #{number} {mergeable_icon} **{title}**\n   By {author} | {state}`
- `RunSummary::format_for_telegram()` — `🔄 {status_icon} **{title}** — {conclusion}\n   Branch: {branch} | Trigger: {event}`
- `IssueSummary::format_for_telegram()` — `📁 #{number} **{title}**\n   {state} | {labels} | {assignees}`
- `PrDetail::format_for_telegram()` — keep multi-line detail, replace aligned sections
- `ReleaseSummary::format_for_telegram()` — keep multi-line detail, replace aligned sections
- `CompareResult::format_for_telegram()` — keep commit list format, remove table alignment

**stripe.rs** (2 methods):
- `Customer::format_for_telegram()` — already close to target, refine with `💰` prefix
- `Invoice::format_for_telegram()` — already close, standardize detail indentation

**neon.rs** (4 functions):
- `format_results()` — keep single-row key:value, replace multi-row table with list format
- `format_projects()` — `🗃️ **{name}**\n   ID: {id} | Region: {region} | Created: {date}`
- `format_branches()` — `🗃️ **{name}** ({state})\n   ID: {id} | Parent: {parent}`
- `format_endpoints()` — `🗃️ **{id}** ({type}) — {status}\n   Size: {range} | Last active: {date}`

**cloudflare.rs** (3 inline formatters in tool functions):
- `cf_zones` output — `🌐 **{name}** — {status}\n   Plan: {plan} | NS: {nameservers}`
- `cf_dns_records` output — `🌐 {type} **{name}** → {content}\n   Proxied: {yn} | TTL: {ttl}`
- `cf_domain_status` output — key-value format with `🌐` header

**doppler.rs** (3 inline formatters):
- secrets list output — `🔐 **{project}/{env}** ({count} secrets)\n   {name1}, {name2}, ...`
- compare output — `🔐 **{env_a}** vs **{env_b}**\n   Only in {a}: ...\n   Only in {b}: ...`
- activity output — `🔐 [{timestamp}] {actor}: {text}`

**vercel.rs** (2 functions + 1 method):
- `Deployment::format_for_telegram()` — `🏗️ {status_icon} **{project}** — {state}\n   {commit_msg} | {age}`
- `format_deployments_for_telegram()` — list of Deployment::format_for_telegram items
- `format_build_logs_for_telegram()` — keep log line format, remove table structure

**sentry.rs** (2 methods + 1 function):
- `IssueSummary::format_for_telegram()` — `🐛 **{title}**\n   {culprit} | Events: {count} | {short_id}`
- `IssueDetail::format_for_telegram()` — `🐛 **{title}**\n   {metadata}\n   {stack_trace preview}`
- `format_stack_trace()` — keep as-is (already line-based, not tabular)

**posthog.rs** (2 functions):
- `format_trends()` — `📊 **{event}** ({project})\n   {period}: {value} ({trend_icon} {delta}%)`
- `format_flags()` — `📊 {active_icon} **{key}**\n   {filter summary}`

**resend.rs** (2 functions):
- `format_emails()` — `📧 **{subject}** — {to}\n   Status: {status} | {created}`
- `format_bounces()` — `📧 ❌ **{subject}** — {to}\n   Bounced: {reason}`

**upstash.rs** (2 functions):
- `format_info()` — key-value pairs with `🔑` header, one line per stat
- `format_keys()` — `🔑 {pattern} ({count} keys)\n   {key1}, {key2}, ...`

**ha.rs** (2 functions):
- `format_states()` — `🏠 **{friendly_name}** — {state}\n   {attributes summary}`
- `format_entity()` — `🏠 **{friendly_name}**\n   State: {state}\n   {key}: {value} per attribute`

**plaid.rs** (2 functions):
- `format_balances()` — `💰 **{account_name}** — {balance}\n   {detail}`
- `format_bills()` — `💰 **{payee}** — {amount}\n   Due: {date} | Status: {status}`

**calendar.rs** (3 functions):
- `format_event()` — `📅 **{title}**\n   {start} – {end} | {location}\n   {attendees}`
- `format_event_time()` — helper, likely stays the same
- `format_attendees()` — helper, likely stays the same

**check.rs** (new `format_telegram()` function):
- `format_telegram()` — `✅ {name} — {latency}ms\n   {detail}` per healthy service; `⚠️`/`❌` for degraded/unhealthy; summary line at bottom
- `format_terminal()` stays as-is (designed for terminal with ANSI codes)
- `format_entry_terminal()` stays as-is

**docker.rs** (inline formatter in `docker_ps`):
- Replace table with `🐳 **{name}** ({image}) — {state}\n   Uptime: {uptime} | Ports: {ports}`

**web.rs** (inline formatter in search function):
- `🔍 **{title}**\n   {url}\n   {snippet}`

**teams.rs** (inline formatters):
- channel list — `💬 **{display_name}**\n   ID: {id} | {description}`
- messages — `💬 [{timestamp}] **{sender}**\n   {preview}`

**jira/tools.rs** (2 functions):
- `format_issues_for_claude()` — `💬 **{key}** — {summary}\n   Status: {status} | Assignee: {assignee} | Priority: {priority}`
- `format_issue_for_claude()` — key-value detail with `💬` header

**jira/webhooks.rs** (3 inline alert formats in webhook handler):
- Already `[Jira webhook] ...` format, update to use emoji prefix

### Req-4: Preserve Existing Signatures

No changes to function signatures, return types, or public API. Only the string content changes.
Helper functions like `format_table()` in `neon.rs`, `format_age()` in `vercel.rs`,
`format_currency()` in `stripe.rs`, and `format_unix_date()` in `stripe.rs` remain unchanged
(they produce value fragments, not table structure).

### Req-5: Relative Timestamp Helper

Add a shared `fn relative_time(timestamp: &str) -> String` helper (or adapt existing
`format_age()` from vercel.rs) that returns:
- `"just now"` for <1min
- `"5m ago"` for <1h
- `"3h ago"` for <24h
- `"2d ago"` for <7d
- `"Mar 15"` for older dates

This helper should live in a common location (either `tools/mod.rs` or a new `tools/fmt.rs`)
and be used by all modules that display timestamps.

### Req-6: Update Tests

All tests that assert specific format strings must be updated to match the new format. Test
function names stay the same. The test count should not decrease.

## Scope
- **IN**: All format_*() and format_for_telegram() functions across 17+ modules, inline formatters in tool functions, new format_telegram() for check.rs, shared relative_time helper, test updates
- **OUT**: Changes to tool signatures or return types, changes to API clients or data fetching, new tools or features, changes to format_json() in check.rs, changes to format_terminal() / format_entry_terminal() in check.rs

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/tools/ado.rs` | Rewrite 3 format functions |
| `crates/nv-daemon/src/tools/github.rs` | Rewrite 6 format_for_telegram methods |
| `crates/nv-daemon/src/tools/stripe.rs` | Refine 2 format_for_telegram methods |
| `crates/nv-daemon/src/tools/neon.rs` | Rewrite 4 format functions, remove format_table helper |
| `crates/nv-daemon/src/tools/cloudflare.rs` | Rewrite 3 inline formatters |
| `crates/nv-daemon/src/tools/doppler.rs` | Rewrite 3 inline formatters |
| `crates/nv-daemon/src/tools/vercel.rs` | Rewrite 2 functions + 1 method |
| `crates/nv-daemon/src/tools/sentry.rs` | Rewrite 2 format_for_telegram methods |
| `crates/nv-daemon/src/tools/posthog.rs` | Rewrite 2 format functions |
| `crates/nv-daemon/src/tools/resend.rs` | Rewrite 2 format functions |
| `crates/nv-daemon/src/tools/upstash.rs` | Rewrite 2 format functions |
| `crates/nv-daemon/src/tools/ha.rs` | Rewrite 2 format functions |
| `crates/nv-daemon/src/tools/plaid.rs` | Rewrite 2 format functions |
| `crates/nv-daemon/src/tools/calendar.rs` | Rewrite format_event function |
| `crates/nv-daemon/src/tools/check.rs` | Add new format_telegram() function |
| `crates/nv-daemon/src/tools/docker.rs` | Rewrite inline docker_ps formatter |
| `crates/nv-daemon/src/tools/web.rs` | Rewrite inline search result formatter |
| `crates/nv-daemon/src/tools/teams.rs` | Rewrite inline channel/message formatters |
| `crates/nv-daemon/src/tools/jira/tools.rs` | Rewrite 2 format functions |
| `crates/nv-daemon/src/tools/jira/webhooks.rs` | Update 3 inline alert formats |
| `crates/nv-daemon/src/tools/mod.rs` or new `fmt.rs` | Add shared relative_time helper |

## Risks
| Risk | Mitigation |
|------|-----------|
| 40+ functions across 20 files — large blast radius | Group by module, one task per module, gate after each batch |
| Test assertions on exact strings break | Tests updated in same task as each module rewrite |
| Emoji rendering inconsistency across Telegram clients | Stick to well-supported Unicode emoji (no skin tones, no ZWJ sequences) |
| Claude tool result interpretation may change | Format is still plain text — Claude reads the content, not the layout |
| format_results() in neon.rs is used for SQL query output (dynamic columns) | Keep single-row key:value, use numbered list for multi-row instead of table |
