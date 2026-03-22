# Implementation Tasks

<!-- beads:epic:TBD -->

## Rust Implementation

- [ ] [1.1] [P-1] Create crates/nv-daemon/src/upstash.rs — UpstashClient struct with rest_url + token + reqwest::Client, new() constructor [owner:api-engineer]
- [ ] [1.2] [P-1] Add execute_command(args: &[&str]) method — POST to REST URL with JSON array body, parse response [owner:api-engineer]
- [ ] [1.3] [P-2] Add info() method — sends INFO command, parses response into structured UpstashInfo (memory, clients, keyspace, uptime) [owner:api-engineer]
- [ ] [1.4] [P-2] Add keys(pattern: &str) method — sends SCAN 0 MATCH pattern COUNT 100, returns Vec<String> capped at 100 [owner:api-engineer]
- [ ] [1.5] [P-2] Add format_info(info: &UpstashInfo) helper — formats as readable summary text [owner:api-engineer]
- [ ] [1.6] [P-3] Add mod upstash declaration in main.rs [owner:api-engineer]

## Tool Integration

- [ ] [2.1] [P-1] Register upstash_info tool in register_tools() — input schema: {} (no params) [owner:api-engineer]
- [ ] [2.2] [P-1] Register upstash_keys tool in register_tools() — input schema: { pattern: string } [owner:api-engineer]
- [ ] [2.3] [P-2] Add dispatch cases in execute_tool() for both tools — call UpstashClient methods, format output [owner:api-engineer]
- [ ] [2.4] [P-2] Init UpstashClient in main.rs from UPSTASH_REDIS_REST_URL + UPSTASH_REDIS_REST_TOKEN env vars — graceful fallback if missing [owner:api-engineer]

## Verify

- [ ] [3.1] cargo build passes [owner:api-engineer]
- [ ] [3.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [ ] [3.3] cargo test — new UpstashClient tests (mock HTTP with wiremock for INFO + SCAN responses) + existing tests pass [owner:api-engineer]
- [ ] [3.4] [user] Manual test: ask Nova "How's Redis doing?" via Telegram, verify upstash_info response [owner:api-engineer]
