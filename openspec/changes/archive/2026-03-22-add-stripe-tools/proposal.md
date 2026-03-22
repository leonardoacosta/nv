# Proposal: Add Stripe Tools

## Change ID
`add-stripe-tools`

## Summary

Stripe payment data via REST API (api.stripe.com). Two read-only tools exposing customer
search and invoice listing — authenticated via Stripe secret key, formatted for Telegram
delivery. No write operations, no PII beyond email/name.

## Context
- Extends: `crates/nv-daemon/src/tools.rs` (tool definitions), `crates/nv-daemon/src/agent.rs` (tool dispatch)
- Related: PRD Phase 2 "Data Sources — OAuth/DB" (Tier 3), `add-tool-audit-log` spec (audit logging dependency)
- Auth: Stripe secret key via `STRIPE_SECRET_KEY` env var (restricted key with read-only scope preferred)

## Motivation

Stripe processes payments for multiple projects. Currently checking customer status or
invoice state requires opening the Stripe dashboard. Wiring Stripe into Nova enables:

1. **Customer lookups** — "Find the Stripe customer for john@example.com" returns customer data
2. **Invoice status** — "Any unpaid invoices?" returns overdue/open invoices
3. **Aggregation layer input** — `financial_summary()` needs Stripe revenue data
4. **Proactive digest** — "3 invoices overdue ($450 total) | 12 new customers this week"

## Requirements

### Req-1: stripe_customers Tool

```
stripe_customers(query: String) -> Vec<CustomerSummary>
```

REST call: `GET https://api.stripe.com/v1/customers/search?query={query}&limit=10`

Stripe Search API supports: `email:"john@example.com"`, `name:"John"`, `metadata["key"]:"value"`.

Returns matching customers with: id, email, name, created date, currency, total invoices count.
Format for Telegram as condensed customer list. Omit sensitive payment method details.

### Req-2: stripe_invoices Tool

```
stripe_invoices(status: String) -> Vec<InvoiceSummary>
```

REST call: `GET https://api.stripe.com/v1/invoices?status={status}&limit=20`

Valid status values: `draft`, `open`, `paid`, `uncollectible`, `void`. Default: `open`.

Returns invoices with: id, customer email, amount due (formatted with currency),
status, due date, description. Format for Telegram with status emoji and amount
highlighting. Total amount at bottom.

### Req-3: HTTP Client

Use `reqwest` with:
- `Authorization: Bearer {STRIPE_SECRET_KEY}` header on all requests
- `Stripe-Version: 2024-12-18.acacia` header (pin API version)
- 15s request timeout
- Form-encoded query parameters (Stripe uses form encoding, not JSON bodies)
- Error mapping: 401 -> "Stripe key invalid", 403 -> "Key lacks permission", 429 -> "Rate limited"

### Req-4: Tool Registration

Both tools registered in `tools.rs` with:
- Tool name and description for Claude's tool-use schema
- Input validation (query non-empty for customers, status is valid enum for invoices)
- Error handling for missing STRIPE_SECRET_KEY env var
- Audit logging via tool_usage table (log query/status, NOT the API key)

### Req-5: Data Safety

- Use restricted API key with read-only scope when possible
- Never log the Stripe secret key
- Omit full card numbers, bank accounts from customer responses
- Only surface: email, name, invoice amounts, dates, statuses

## Scope
- **IN**: Two read-only tools (customers search, invoices list), REST API client, form-encoded requests, error handling, audit logging
- **OUT**: Payment creation, refunds, subscription management, webhook handling, checkout sessions, payment methods, disputes

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/tools.rs` | Add stripe_customers, stripe_invoices tool definitions + dispatch handlers |
| `crates/nv-daemon/src/agent.rs` | Register new tools in available_tools list |
| `crates/nv-daemon/src/stripe.rs` | New: Stripe module with HTTP client, typed structs, currency formatter |
| `crates/nv-daemon/src/main.rs` | Add `mod stripe;` declaration |

## Risks
| Risk | Mitigation |
|------|-----------|
| STRIPE_SECRET_KEY not set | Return clear error: "STRIPE_SECRET_KEY env var not set" |
| Key has write permissions | Document: use restricted key with read-only scope |
| PII in customer data | Only surface email + name; omit payment methods, addresses |
| Stripe API version drift | Pin version header; update annually or when features need it |
| Rate limiting (100 read/s) | Single-user scale; well within limits |
