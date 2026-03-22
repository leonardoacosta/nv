# Proposal: Add Claude Usage Tracking

## Change ID
`add-claude-usage-tracking`

## Summary

Three-tier Claude usage tracking system: parse cost/token data already present in CLI responses
(Tier 1), query account metadata from the Claude CLI (Tier 2), and enforce configurable budget
thresholds with Telegram alerts (Tier 3). All tiers feed into `nv stats` and the proactive digest.

## Context
- Extends: `crates/nv-daemon/src/claude.rs` (CliJsonResponse, CliUsage, StreamJsonUsage, StreamJsonEvent — cost fields exist in CLI output but are not captured)
- Extends: `crates/nv-daemon/src/messages.rs` (MessageStore — new `api_usage` table alongside existing `messages` and `tool_usage` tables)
- Extends: `crates/nv-daemon/src/http.rs` (stats_handler — add usage cost section to `/stats` JSON)
- Extends: `crates/nv-cli/src/main.rs` (display_stats — add cost section to `nv stats` output)
- Extends: `crates/nv-daemon/src/worker.rs` (worker loop — log usage after each Claude turn)
- Extends: `crates/nv-core/src/config.rs` (AgentConfig — budget/threshold fields)
- Extends: `config/nv.toml` ([agent] section — new budget config keys)
- Related: `crates/nv-daemon/src/digest/` (inject budget warnings into digest output)

## Motivation

The Claude CLI already returns `total_cost_usd` in its JSON response, but `CliJsonResponse` and
`StreamJsonUsage` do not capture it. Token counts are logged per-message in the `messages` table
but never aggregated into cost. There is no visibility into:

1. **How much Claude costs per day/week/month** — the operator has to check the Anthropic dashboard manually
2. **Whether spending is on track** — no budget awareness, no alerts until the bill arrives
3. **Account status** — plan type, rate limits, and session caps are invisible to NV

This spec adds all three in a layered approach where each tier is independently useful.

## Requirements

### Req-1: Parse Cost from CLI Response (Tier 1)

Add `total_cost_usd: Option<f64>` to `CliJsonResponse`, `CliUsage`, `StreamJsonUsage`, and
propagate through `ApiResponse` / `Usage`. The Claude CLI already emits this field — we just
need to deserialize it.

Both paths must capture cost:
- **Cold-start** (`--output-format json`): `CliJsonResponse.usage.total_cost_usd`
- **Stream-json** (`--output-format stream-json`): `StreamJsonUsage.total_cost_usd` in the `result` event

### Req-2: API Usage Table

New SQLite table in `~/.nv/messages.db` (same database as messages/tool_usage):

```sql
CREATE TABLE api_usage (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL,
    worker_id TEXT NOT NULL,
    cost_usd REAL,              -- from total_cost_usd (NULL if CLI doesn't report it)
    tokens_in INTEGER NOT NULL,
    tokens_out INTEGER NOT NULL,
    model TEXT NOT NULL,
    session_id TEXT NOT NULL
);

CREATE INDEX idx_api_usage_timestamp ON api_usage(timestamp);
CREATE INDEX idx_api_usage_worker ON api_usage(worker_id);
```

### Req-3: Log Usage After Each Claude Turn

After every `send_messages` call (both worker.rs and agent.rs paths), insert a row into
`api_usage` with the response's cost, tokens, model, session_id, and the worker's UUID.

### Req-4: Usage Stats Queries

Add methods to `MessageStore`:

- `usage_stats() -> UsageStatsReport` — today's cost, today's calls, today's tokens, 7-day daily
  breakdown (date, cost, calls, tokens_in, tokens_out), rolling 7-day total cost, rolling 30-day
  total cost
- `usage_budget_status(weekly_budget: f64) -> BudgetStatus` — rolling 7-day cost vs budget,
  percentage used

### Req-5: Extend `nv stats` Output

Add a "Claude Usage" section to the stats HTTP endpoint and CLI display:

```
Claude Usage
----------
Today:             $4.82 / 142 calls / 2.1M tokens
This week:         $34.20 / 891 calls
This month:        $127.50 / 3,420 calls
Budget:            $34.20 / $50.00 (68%)
```

### Req-6: Account Info Cache (Tier 2)

Query `claude account` (or equivalent CLI subcommand) for account metadata:
- Plan name (e.g., "Pro", "Max")
- Organization/username
- Auth method (OAuth, API key)

Cache result in `~/.nv/account-info.json`. Refresh every 6 hours (or on daemon startup if stale).

Display in `nv stats`:
```
Account: Pro Plan / leonardoacosta / OAuth
```

### Req-7: Budget Threshold Alerts (Tier 3)

New config fields in `[agent]` section of `nv.toml`:

```toml
[agent]
weekly_budget_usd = 50.0
alert_threshold_pct = 90
```

Defaults: `weekly_budget_usd = 50.0`, `alert_threshold_pct = 90`.

**Digest injection**: When rolling 7-day cost exceeds 80% of budget, append a warning line to
the proactive digest output. Example at 83%:

```
Budget: $41.50 / $50.00 (83%)
```

**Immediate Telegram alert**: When rolling 7-day cost crosses `alert_threshold_pct`, send a
standalone Telegram message (not part of digest). Debounce: at most one alert per 6 hours.

```
Budget alert: 92% used ($46.00 / $50.00)
```

Track last alert timestamp in SQLite to enforce debounce.

## Scope
- **IN**: Cost parsing from CLI JSON, api_usage SQLite table, per-turn logging, usage stats queries, `nv stats` extension, account info cache, budget config, digest budget warning, immediate Telegram alert at threshold
- **OUT**: Anthropic API direct billing integration, per-tool cost attribution, historical cost visualization UI, multi-account tracking, cost prediction/forecasting

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/claude.rs` | Add `total_cost_usd` to CliUsage, StreamJsonUsage, Usage, ApiResponse |
| `crates/nv-daemon/src/messages.rs` | New `api_usage` table, `log_api_usage()`, `usage_stats()`, `usage_budget_status()` methods |
| `crates/nv-daemon/src/worker.rs` | Log api_usage after each Claude turn |
| `crates/nv-daemon/src/agent.rs` | Log api_usage after each Claude turn (legacy path) |
| `crates/nv-daemon/src/http.rs` | Extend stats_handler with usage cost section |
| `crates/nv-cli/src/main.rs` | Add Claude Usage section to `nv stats` display |
| `crates/nv-core/src/config.rs` | Add `weekly_budget_usd`, `alert_threshold_pct` to AgentConfig |
| `config/nv.toml` | Add budget config keys to [agent] section |
| `crates/nv-daemon/src/digest/` | Inject budget warning line when >80% |
| `crates/nv-daemon/src/account.rs` | New: account info query + JSON cache |

## Risks
| Risk | Mitigation |
|------|-----------|
| `total_cost_usd` absent from some CLI versions | Field is `Option<f64>` with `#[serde(default)]` — graceful None |
| `claude account` CLI subcommand changes/unavailable | Cache file provides fallback; display "unknown" if query fails |
| Budget alerts spam operator | 6-hour debounce, single alert per threshold crossing |
| api_usage table grows large | ~100 bytes/row, 1000 calls/day = ~36KB/day, ~13MB/year. Not a concern. |
| Cost field precision (f64) | Sufficient for USD amounts; no financial accounting precision needed |
