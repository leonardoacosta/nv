# Implementation Tasks

<!-- beads:epic:TBD -->

## Rust Implementation

- [x] [1.1] [P-1] tokio-postgres already in nv-daemon Cargo.toml dependencies [owner:api-engineer]
- [x] [1.2] [P-1] Create crates/nv-daemon/src/plaid_tools.rs — PlaidClient via connect() with PLAID_DB_URL, read-only mode [owner:api-engineer]
- [x] [1.3] [P-1] Define ALLOWED_COLUMNS const array: ["account_name", "account_type", "current_balance", "available_balance", "last_updated"] [owner:api-engineer]
- [x] [1.4] [P-1] Implement filter_pii(row, columns) -> SafeRow — extract only allowlisted columns, scrub values matching PII patterns (9-digit numbers, routing number patterns), return SafeRow struct [owner:api-engineer]
- [x] [1.5] [P-2] Add balances() function — hardcoded SELECT of allowed columns from plaid_accounts, apply filter_pii to each row, return formatted string [owner:api-engineer]
- [x] [1.6] [P-2] Add bills() function — hardcoded SELECT of credit/loan accounts from plaid_accounts, apply filter_pii to each row, return formatted string [owner:api-engineer]
- [x] [1.7] [P-2] Add format_balances(rows: &[SafeRow]) helper — formatted table with account name, type, current/available balance, last updated [owner:api-engineer]
- [x] [1.8] [P-2] Add format_bills(rows: &[SafeRow]) helper — formatted list of credit/loan accounts with balances owed [owner:api-engineer]
- [x] [1.9] [P-3] Add mod plaid_tools declaration in main.rs [owner:api-engineer]

## Tool Integration

- [x] [2.1] [P-1] Register plaid_balances tool in register_tools() — input schema: {} (no params) [owner:api-engineer]
- [x] [2.2] [P-1] Register plaid_bills tool in register_tools() — input schema: {} (no params) [owner:api-engineer]
- [x] [2.3] [P-2] Add dispatch cases in execute_tool() for both tools — call plaid_balances/plaid_bills, return formatted output [owner:api-engineer]
- [x] [2.4] [P-2] Connection from PLAID_DB_URL env var — graceful fallback if missing [owner:api-engineer]

## Security Tests

- [x] [3.1] [P-1] Unit test: filter scrub_pii strips PII patterns — inject 9-digit number, verify it's redacted [owner:api-engineer]
- [x] [3.2] [P-1] Unit test: scrub_pii allows clean values — account names, short numbers, decimals pass through [owner:api-engineer]
- [x] [3.3] [P-1] Unit test: matches_pii_pattern respects word boundaries [owner:api-engineer]
- [x] [3.4] [P-2] Unit test: format_balances and format_bills produce expected output [owner:api-engineer]

## Verify

- [x] [4.1] cargo build passes [owner:api-engineer]
- [x] [4.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [x] [4.3] cargo test — all security tests + PlaidClient tests + existing tests pass [owner:api-engineer]
- [ ] [4.4] [user] Manual test: ask Nova "What are my account balances?" via Telegram, verify no PII visible in response [owner:api-engineer]
- [ ] [4.5] [user] Manual test: verify audit log does NOT contain raw balance values or PII [owner:api-engineer]
