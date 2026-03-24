# Proposal: Add Neon Tools

## Change ID
`add-neon-tools`

## Summary

Direct read-only SQL queries to Neon PostgreSQL databases via per-project POSTGRES_URL
connection strings (sourced from Doppler). Single tool executing parameterized queries
with result formatting for Telegram delivery. Uses `tokio-postgres` for async connectivity.

## Context
- Extends: `crates/nv-daemon/src/tools.rs` (tool definitions), `crates/nv-daemon/src/agent.rs` (tool dispatch)
- Related: PRD Phase 2 "Data Sources — OAuth/DB" (Tier 3), `add-tool-audit-log` spec (audit logging dependency)
- Auth: POSTGRES_URL per project via env vars (`NEON_OO_URL`, `NEON_TC_URL`, etc.)

## Motivation

All T3 projects use Neon PostgreSQL. Currently querying production data requires SSHing
to the homelab and running psql manually. Wiring Neon into Nova enables:

1. **Data queries** — "How many users signed up on OO today?" runs a COUNT query
2. **Aggregation layer input** — `project_health(code)` can include DB health metrics
3. **Debugging** — "Show me the last 5 failed payments on SS" returns actual rows
4. **Read-only safety** — connection uses read-only transaction mode, no mutations possible

## Requirements

### Req-1: neon_query Tool

```
neon_query(project: String, sql: String) -> QueryResult
```

Executes parameterized SQL against the project's Neon database:
1. Resolve project code (oo, tc, tl, mv, ss) to env var name (`NEON_OO_URL`)
2. Connect via `tokio-postgres` with TLS (Neon requires SSL)
3. Execute in read-only transaction: `SET TRANSACTION READ ONLY; {sql}`
4. Return rows as Vec<Vec<String>> with column headers
5. Format as aligned table or key-value pairs for Telegram

### Req-2: Connection Management

- Per-project connection strings stored as env vars: `NEON_{CODE}_URL`
  (e.g., `NEON_OO_URL=postgres://user:pass@ep-xxx.us-east-2.aws.neon.tech/neondb`)
- Connections created on-demand, not pooled (low query frequency)
- TLS required — use `tokio-postgres-rustls` or `native-tls` feature
- Connection timeout: 10s
- Query timeout: 30s

### Req-3: Safety Guards

- **Read-only enforcement**: Every query wrapped in `SET TRANSACTION READ ONLY`
- **SQL validation**: Reject queries containing `INSERT`, `UPDATE`, `DELETE`, `DROP`,
  `ALTER`, `TRUNCATE`, `CREATE` (case-insensitive regex check before execution)
- **Row limit**: Append `LIMIT 50` if query doesn't already contain LIMIT clause
- **Column limit**: Truncate cell values >200 chars with "..."
- **No parameterized user input**: The SQL comes from Claude (trusted), but
  connection strings are secret — never log them

### Req-4: Tool Registration

Tool registered in `tools.rs` with:
- Tool name and description for Claude's tool-use schema
- Input validation (project code resolves to known env var, SQL non-empty)
- Error handling for missing connection string, connection failure, query error
- Audit logging via tool_usage table (log query text, NOT connection string)

## Scope
- **IN**: Single read-only query tool, per-project connection strings, SQL validation, row/column limits, TLS, audit logging
- **OUT**: Write queries, schema migrations, connection pooling, query caching, cross-database joins, Neon API (branch management)

## Impact
| Area | Change |
|------|--------|
| `Cargo.toml` | Add `tokio-postgres` + TLS dependency to workspace |
| `crates/nv-daemon/Cargo.toml` | Add tokio-postgres dependency |
| `crates/nv-daemon/src/tools.rs` | Add neon_query tool definition + dispatch handler |
| `crates/nv-daemon/src/agent.rs` | Register new tool in available_tools list |
| `crates/nv-daemon/src/neon.rs` | New: Neon module with connect, execute_readonly, format_results |
| `crates/nv-daemon/src/main.rs` | Add `mod neon;` declaration |

## Risks
| Risk | Mitigation |
|------|-----------|
| SQL injection via Claude | Read-only transaction + keyword blocklist; Claude is trusted but defense-in-depth |
| Connection string leaked in logs | Never log connection URL; log only query text and project code |
| Neon cold start latency | First query may take 2-5s for serverless wake; 10s connection timeout accommodates this |
| Large result sets | LIMIT 50 enforced, cell truncation at 200 chars |
| TLS handshake failure | Neon requires SSL; use rustls for reliable TLS without system OpenSSL dependency |
