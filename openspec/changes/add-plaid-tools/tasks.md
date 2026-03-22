# Implementation Tasks

<!-- beads:epic:TBD -->

## Rust Implementation

- [ ] [1.1] [P-1] Add tokio-postgres to nv-daemon Cargo.toml dependencies (if not present) [owner:api-engineer]
- [ ] [1.2] [P-1] Create crates/nv-daemon/src/plaid.rs — PlaidClient struct with connection string, new() constructor that validates connectivity [owner:api-engineer]
- [ ] [1.3] [P-1] Define ALLOWED_COLUMNS const array: ["account_name", "account_type", "current_balance", "available_balance", "last_updated"] [owner:api-engineer]
- [ ] [1.4] [P-1] Implement filter_pii(row: &Row) -> SafeRow — extract only allowlisted columns, scrub values matching PII patterns (9-digit numbers, routing number patterns), return SafeRow struct [owner:api-engineer]
- [ ] [1.5] [P-2] Add balances() method — hardcoded SELECT of allowed columns from plaid_accounts, apply filter_pii to each row, return Vec<SafeRow> [owner:api-engineer]
- [ ] [1.6] [P-2] Add bills() method — hardcoded SELECT of credit/loan accounts from plaid_accounts, apply filter_pii to each row, return Vec<SafeRow> [owner:api-engineer]
- [ ] [1.7] [P-2] Add format_balances(rows: &[SafeRow]) helper — formatted table with account name, type, current/available balance, last updated [owner:api-engineer]
- [ ] [1.8] [P-2] Add format_bills(rows: &[SafeRow]) helper — formatted list of credit/loan accounts with balances owed [owner:api-engineer]
- [ ] [1.9] [P-3] Add mod plaid declaration in main.rs [owner:api-engineer]

## Tool Integration

- [ ] [2.1] [P-1] Register plaid_balances tool in register_tools() — input schema: {} (no params) [owner:api-engineer]
- [ ] [2.2] [P-1] Register plaid_bills tool in register_tools() — input schema: {} (no params) [owner:api-engineer]
- [ ] [2.3] [P-2] Add dispatch cases in execute_tool() for both tools — call PlaidClient methods, return formatted output [owner:api-engineer]
- [ ] [2.4] [P-2] Init PlaidClient in main.rs from CORTEX_POSTGRES_URL env var — graceful fallback if missing [owner:api-engineer]

## Security Tests

- [ ] [3.1] [P-1] Unit test: filter_pii strips non-allowlisted columns — add extra columns to mock row, verify they are absent from SafeRow [owner:api-engineer]
- [ ] [3.2] [P-1] Unit test: filter_pii scrubs PII patterns — inject 9-digit number in account_name, verify it's redacted [owner:api-engineer]
- [ ] [3.3] [P-1] Unit test: balances() returns only SafeRow (no raw Row exposed) — verify return type [owner:api-engineer]
- [ ] [3.4] [P-2] Unit test: bills() returns only credit/loan type accounts [owner:api-engineer]

## Verify

- [ ] [4.1] cargo build passes [owner:api-engineer]
- [ ] [4.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [ ] [4.3] cargo test — all security tests + PlaidClient tests + existing tests pass [owner:api-engineer]
- [ ] [4.4] [user] Manual test: ask Nova "What are my account balances?" via Telegram, verify no PII visible in response [owner:api-engineer]
- [ ] [4.5] [user] Manual test: verify audit log does NOT contain raw balance values or PII [owner:api-engineer]
