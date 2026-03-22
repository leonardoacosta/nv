# Implementation Tasks

<!-- beads:epic:TBD -->

## Rust Module

- [ ] [1.1] [P-1] Create crates/nv-daemon/src/stripe.rs — module with typed structs: CustomerSummary, InvoiceSummary [owner:api-engineer]
- [ ] [1.2] [P-1] Add StripeClient struct — holds reqwest::Client + secret key + API version, constructed from STRIPE_SECRET_KEY env var [owner:api-engineer]
- [ ] [1.3] [P-2] Add search_customers(query: &str) async method — GET /v1/customers/search, form-encoded query, parse customer list [owner:api-engineer]
- [ ] [1.4] [P-2] Add list_invoices(status: &str) async method — GET /v1/invoices?status={status}, parse invoice list with amounts [owner:api-engineer]
- [ ] [1.5] [P-2] Add format_currency(amount: i64, currency: &str) helper — convert cents to display string (e.g., 4500 USD -> "$45.00") [owner:api-engineer]
- [ ] [1.6] [P-2] Add format_for_telegram() methods — condensed customer list, invoice list with status emoji + amounts + total [owner:api-engineer]

## Tool Integration

- [ ] [2.1] [P-1] Add `mod stripe;` to main.rs [owner:api-engineer]
- [ ] [2.2] [P-1] Register stripe_customers, stripe_invoices in tools.rs tool definitions (name, description, input schema) [owner:api-engineer]
- [ ] [2.3] [P-2] Add dispatch handlers in tools.rs — validate inputs (query non-empty, status in allowed set), call stripe module, return formatted result [owner:api-engineer]
- [ ] [2.4] [P-2] Add error handling — missing env var, 401/403/429 HTTP errors, timeout, malformed response [owner:api-engineer]
- [ ] [2.5] [P-3] Init StripeClient in main.rs on startup, pass to tool dispatch context [owner:api-engineer]
- [ ] [2.6] [P-3] Log each tool invocation to tool_usage audit table (query/status text, NOT API key) [owner:api-engineer]

## Verify

- [ ] [3.1] cargo build passes [owner:api-engineer]
- [ ] [3.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [ ] [3.3] cargo test — parse customer search JSON fixture, parse invoices fixture, format_currency conversion [owner:api-engineer]
- [ ] [3.4] [user] Manual test: send "Any unpaid invoices on Stripe?" via Telegram, verify formatted response [owner:api-engineer]
