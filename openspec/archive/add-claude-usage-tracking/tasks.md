# Implementation Tasks

<!-- beads:epic:TBD -->

## Tier 1 — Parse & Store Cost Data

- [x] [1.1] [P-1] Add `total_cost_usd: Option<f64>` (with `#[serde(default)]`) to `CliUsage` and `StreamJsonUsage` in claude.rs [owner:api-engineer]
- [x] [1.2] [P-1] Add `total_cost_usd: Option<f64>` to `Usage` struct and `ApiResponse` — propagate from both cold-start and stream-json parse paths [owner:api-engineer]
- [x] [1.3] [P-1] Add `api_usage` table creation to `MessageStore::init()` in messages.rs — CREATE TABLE IF NOT EXISTS with id, timestamp, worker_id, cost_usd, tokens_in, tokens_out, model, session_id + indexes [owner:api-engineer]
- [x] [1.4] [P-2] Add `log_api_usage(worker_id, cost_usd, tokens_in, tokens_out, model, session_id)` method to MessageStore [owner:api-engineer]
- [x] [1.5] [P-2] Call `log_api_usage()` in worker.rs after each `send_messages` response — extract cost/tokens/model from ApiResponse, pass worker UUID [owner:api-engineer]
- [x] [1.6] [P-2] Call `log_api_usage()` in agent.rs after each `send_messages` response (legacy single-session path) [owner:api-engineer]

## Tier 1 — Stats & Display

- [x] [2.1] [P-1] Add `UsageStatsReport` struct to messages.rs — today_cost, today_calls, today_tokens_in, today_tokens_out, week_cost, month_cost, daily_breakdown: Vec<(date, cost, calls, tokens_in, tokens_out)> [owner:api-engineer]
- [x] [2.2] [P-1] Add `usage_stats()` method to MessageStore — queries api_usage table for today/7-day/30-day aggregates [owner:api-engineer]
- [x] [2.3] [P-2] Extend `stats_handler` in http.rs — call `usage_stats()`, add `claude_usage` section to JSON response [owner:api-engineer]
- [x] [2.4] [P-2] Add `UsageStatsSection` to `StatsResponse` in nv-cli/src/main.rs — deserialize `claude_usage` from stats JSON [owner:api-engineer]
- [x] [2.5] [P-2] Extend `display_stats()` in nv-cli/src/main.rs — print "Claude Usage" section with today/week/month costs, call counts, budget percentage [owner:api-engineer]

## Tier 2 — Account Info

- [x] [3.1] [P-2] Create `crates/nv-daemon/src/account.rs` — `AccountInfo` struct (plan, username, auth_method), `query_account_info()` fn that runs `claude account` CLI and parses output [owner:api-engineer]
- [x] [3.2] [P-2] Add JSON cache at `~/.nv/account-info.json` — write on successful query, read as fallback, refresh if older than 6 hours [owner:api-engineer]
- [x] [3.3] [P-2] Add `mod account` to main.rs — call `query_account_info()` on daemon startup (non-blocking, background) [owner:api-engineer]
- [x] [3.4] [P-2] Extend stats_handler and `nv stats` display — add "Account: {plan} / {username} / {auth}" line [owner:api-engineer]

## Tier 3 — Budget Alerts

- [x] [4.1] [P-2] Add `weekly_budget_usd: Option<f64>` (default 50.0) and `alert_threshold_pct: Option<u8>` (default 90) to `AgentConfig` in config.rs [owner:api-engineer]
- [x] [4.2] [P-2] Add config keys to `config/nv.toml` [agent] section (commented defaults) [owner:api-engineer]
- [x] [4.3] [P-2] Add `BudgetStatus` struct and `usage_budget_status(weekly_budget)` method to MessageStore — rolling 7-day cost vs budget, pct used [owner:api-engineer]
- [x] [4.4] [P-2] Add `budget_alert_sent` table to MessageStore (id, timestamp) — tracks last alert send time for 6-hour debounce [owner:api-engineer]
- [x] [4.5] [P-2] Inject budget warning into digest output when >80% of weekly budget — append line to DigestResult.content in digest synthesis [owner:api-engineer]
- [x] [4.6] [P-2] Send immediate Telegram alert when rolling 7-day cost crosses alert_threshold_pct — check after each `log_api_usage()`, debounce via budget_alert_sent table [owner:api-engineer]

## Verify

- [x] [5.1] cargo build passes [owner:api-engineer]
- [x] [5.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [x] [5.3] cargo test — new tests: api_usage CRUD, usage_stats aggregation, budget_status calculation, account info parse, CLI display formatting + all existing tests pass [owner:api-engineer]
