# Proposal: Add Plaid Tools

## Change ID
`add-plaid-tools`

## Summary

Plaid financial data tools via cortex-postgres read-only SQL queries. Two tools (`plaid_balances`,
`plaid_bills`) that query the Plaid data already synced to cortex-postgres, with strict column
allowlisting and PII filtering enforced in Rust BEFORE any tool result reaches Claude.

## Context
- Extends: `crates/nv-daemon/src/tools.rs` (tool definitions + dispatch), `crates/nv-daemon/src/agent.rs` (tool execution)
- Related: Existing tool pattern, `add-tool-audit-log` spec, cortex-postgres database (Plaid data synced via external process)
- PRD ref: Phase 2, Section 6.1 — Tier 4 (Special — PII sensitive)

## Motivation

Plaid data is already synced to cortex-postgres by an external pipeline. Currently Leo must
query the database directly or check the Plaid dashboard for account balances and upcoming bills.
Wiring Plaid into Nova lets Leo ask "What are my account balances?" or "Any bills due this week?"
from Telegram. **Critical constraint**: financial data contains PII (account numbers, routing
numbers, SSN fragments) that MUST be filtered in Rust before reaching Claude or Telegram.

## Requirements

### Req-1: Database Client Module

New file `crates/nv-daemon/src/plaid.rs` with:
- `PlaidClient` struct holding a PostgreSQL connection string for cortex-postgres
- Connection: `tokio-postgres` or `sqlx` with read-only connection (SET default_transaction_read_only = on)
- All queries are SELECT only — no writes, no DDL

### Req-2: Column Allowlist (Security Critical)

**Hardcoded allowlist of columns that may be returned.** Any column NOT on this list is stripped
from results in Rust before the tool returns.

Allowed columns:
- `account_name` — display name of the account (e.g., "Checking", "Savings")
- `account_type` — account type (e.g., "depository", "credit", "loan")
- `current_balance` — current balance amount
- `available_balance` — available balance amount
- `last_updated` — timestamp of last sync

**Blocked columns** (never returned, never in query results):
- `account_number`, `routing_number`, `wire_routing` — PII
- `account_id`, `item_id`, `access_token` — Plaid internals
- Any column not in the allowlist — blocked by default

### Req-3: PII Filter (Security Critical)

`filter_pii(row: &Row) -> SafeRow` function in Rust that:
1. Extracts ONLY allowlisted columns from query results
2. Scrubs any remaining values matching PII patterns (9-digit numbers, routing patterns)
3. Returns `SafeRow` struct containing only safe fields
4. This filter runs BEFORE the tool result is returned to Claude

This is the primary security boundary. Claude never sees raw query results.

### Req-4: plaid_balances Tool

`plaid_balances()` — List account balances.

- Query: `SELECT account_name, account_type, current_balance, available_balance, last_updated FROM plaid_accounts ORDER BY account_type, account_name`
- Output: Formatted table of accounts with name, type, current balance, available balance, last sync
- All results pass through PII filter before returning

### Req-5: plaid_bills Tool

`plaid_bills()` — List upcoming bills and recurring transactions.

- Query: `SELECT account_name, account_type, current_balance, last_updated FROM plaid_accounts WHERE account_type IN ('credit', 'loan') ORDER BY current_balance DESC`
- Or if a dedicated bills/recurring table exists: query it with same allowlist enforcement
- Output: Formatted list of credit/loan accounts with balances (which represent amounts owed)
- All results pass through PII filter before returning

### Req-6: Tool Registration

Register both tools in `register_tools()`. No input parameters (both are parameterless).
Wire dispatch in `execute_tool()` to call PlaidClient methods.

### Req-7: Configuration

- Env var: `CORTEX_POSTGRES_URL` — connection string for cortex-postgres (read-only user)
- The database user should have SELECT-only grants on Plaid tables
- Fail gracefully: if connection string missing or DB unreachable, return "Plaid not configured"

### Req-8: Audit Logging

Every tool invocation logged via tool audit log. Log: tool name, row count returned, success/failure, duration_ms.
**Never log raw query results or column values in audit log.**

## Scope
- **IN**: PlaidClient DB module, column allowlist, PII filter, plaid_balances tool, plaid_bills tool, tool registration, env config
- **OUT**: Writing to Plaid tables, Plaid Link flow, transaction history, investment accounts, identity verification

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/plaid.rs` | New: PlaidClient with balances(), bills(), filter_pii() |
| `crates/nv-daemon/src/tools.rs` | Add 2 tool definitions + dispatch cases |
| `crates/nv-daemon/src/main.rs` | Init PlaidClient, pass to tool executor |
| `crates/nv-daemon/Cargo.toml` | Add tokio-postgres (or sqlx with postgres feature) if not already present |
| `config/env` or `.env` | Add CORTEX_POSTGRES_URL |

## Risks
| Risk | Mitigation |
|------|-----------|
| PII leak to Claude/Telegram | Column allowlist + PII regex filter in Rust, BEFORE tool result. Defense in depth. |
| Database schema changes | Hardcoded queries. If columns renamed, query fails gracefully (no silent PII leak). |
| Connection pool exhaustion | Single read-only connection (not pooled). Plaid queries are infrequent. |
| Stale balance data | Show last_updated timestamp so Leo knows data freshness. |
| SQL injection | No user input in queries. Both tools are parameterless with hardcoded SQL. |
