# Implementation Tasks

<!-- beads:epic:TBD -->

## Rust Implementation

- [x] [1.1] [P-1] Create crates/nv-daemon/src/ha_tools.rs — HAClient struct with base_url + token + reqwest::Client, from_env() constructor [owner:api-engineer]
- [x] [1.2] [P-1] Add states() method — GET /api/states, deserialize into Vec<HAEntity>, group by domain, format summary [owner:api-engineer]
- [x] [1.3] [P-2] Add entity(id: &str) method — GET /api/states/<id>, deserialize into HAEntity with full attributes [owner:api-engineer]
- [x] [1.4] [P-2] Add service_call(domain: &str, service: &str, data: serde_json::Value) method — POST /api/services/<domain>/<service> with data body [owner:api-engineer]
- [x] [1.5] [P-2] Add format_states(entities: &[HAEntity]) helper — grouped by domain with counts, top 20 recently changed [owner:api-engineer]
- [x] [1.6] [P-2] Add format_entity(entity: &HAEntity) helper — state + all attributes + timestamps [owner:api-engineer]
- [x] [1.7] [P-3] Add mod ha_tools declaration in main.rs [owner:api-engineer]

## Tool Integration

- [x] [2.1] [P-1] Register ha_states tool in register_tools() — input schema: {} (no params) [owner:api-engineer]
- [x] [2.2] [P-1] Register ha_entity tool in register_tools() — input schema: { id: string } [owner:api-engineer]
- [x] [2.3] [P-1] Register ha_service_call tool in register_tools() — input schema: { domain: string, service: string, data: object } [owner:api-engineer]
- [x] [2.4] [P-2] Add dispatch for ha_states and ha_entity in execute_tool() — direct call, format output [owner:api-engineer]
- [x] [2.5] [P-2] Add dispatch for ha_service_call with PendingAction — generate confirmation message, require user confirmation before executing [owner:api-engineer]
- [x] [2.6] [P-2] HAClient constructed from HA_URL (default localhost:8123) + HA_TOKEN env vars — graceful fallback if missing [owner:api-engineer]

## Verify

- [x] [3.1] cargo build passes [owner:api-engineer]
- [x] [3.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [x] [3.3] cargo test — new HAClient tests (format_states, format_entity, describe_service_call, missing env) + existing tests pass [owner:api-engineer]
- [ ] [3.4] [user] Manual test: ask Nova "What's the living room temperature?" via Telegram, verify ha_entity response [owner:api-engineer]
- [ ] [3.5] [user] Manual test: ask Nova "Turn off office lights" via Telegram, verify PendingAction confirmation appears before execution [owner:api-engineer]
