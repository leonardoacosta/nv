# Implementation Tasks

<!-- beads:epic:TBD -->

## Dependencies

- [ ] [0.1] [P-1] Add tokio-postgres + tokio-postgres-rustls (or postgres-native-tls) to workspace Cargo.toml [owner:api-engineer]
- [ ] [0.2] [P-1] Add tokio-postgres dependency to crates/nv-daemon/Cargo.toml [owner:api-engineer]

## Rust Module

- [ ] [1.1] [P-1] Create crates/nv-daemon/src/neon.rs — module with typed structs: QueryResult, QueryRow [owner:api-engineer]
- [ ] [1.2] [P-1] Add connect(project: &str) async function — resolve NEON_{CODE}_URL env var, connect with TLS, 10s timeout [owner:api-engineer]
- [ ] [1.3] [P-2] Add validate_sql(sql: &str) function — reject INSERT/UPDATE/DELETE/DROP/ALTER/TRUNCATE/CREATE via case-insensitive regex [owner:api-engineer]
- [ ] [1.4] [P-2] Add ensure_limit(sql: &str) function — append LIMIT 50 if query lacks LIMIT clause [owner:api-engineer]
- [ ] [1.5] [P-2] Add execute_readonly(client, sql: &str) async method — SET TRANSACTION READ ONLY, execute query, 30s timeout, collect rows [owner:api-engineer]
- [ ] [1.6] [P-2] Add format_results(result: &QueryResult) method — aligned table for small results, key-value pairs for single-row, truncate cells >200 chars [owner:api-engineer]

## Tool Integration

- [ ] [2.1] [P-1] Add `mod neon;` to main.rs [owner:api-engineer]
- [ ] [2.2] [P-1] Register neon_query in tools.rs tool definition (name, description, input schema with project + sql params) [owner:api-engineer]
- [ ] [2.3] [P-2] Add dispatch handler in tools.rs — validate project code, validate SQL, connect, execute, format, return [owner:api-engineer]
- [ ] [2.4] [P-2] Add error handling — missing env var, connection failure, query syntax error, timeout, read-only violation [owner:api-engineer]
- [ ] [2.5] [P-3] Log each tool invocation to tool_usage audit table (query text, NOT connection string) [owner:api-engineer]

## Verify

- [ ] [3.1] cargo build passes [owner:api-engineer]
- [ ] [3.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [ ] [3.3] cargo test — SQL validation (block mutations, allow SELECT), ensure_limit insertion, result formatting [owner:api-engineer]
- [ ] [3.4] [user] Manual test: send "How many users on OO?" via Telegram, verify query execution and formatted response [owner:api-engineer]
