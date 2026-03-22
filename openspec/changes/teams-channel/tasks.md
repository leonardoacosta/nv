# Implementation Tasks

<!-- beads:epic:TBD -->

## API Batch

- [x] [2.1] [P-1] Add TeamsConfig to nv-core/config.rs: tenant_id (String), team_ids (Vec<String>), channel_ids (Vec<String>); add `teams: Option<TeamsConfig>` to Config struct [owner:api-engineer]
- [x] [2.2] [P-1] Add ms_graph_client_id (Option<String>) and ms_graph_client_secret (Option<String>) to Secrets struct, read from MS_GRAPH_CLIENT_ID and MS_GRAPH_CLIENT_SECRET env vars [owner:api-engineer]
- [x] [2.3] [P-1] Create crates/nv-daemon/src/teams/oauth.rs — MsGraphAuth struct: token request via POST to login.microsoftonline.com/{tenant}/oauth2/v2.0/token with client_credentials grant, cache access_token + expires_at, auto-refresh 5 min before expiry, Mutex-guarded token state [owner:api-engineer]
- [x] [2.4] [P-1] Create crates/nv-daemon/src/teams/webhook.rs — register_subscription() via POST /subscriptions (resource, changeType, notificationUrl, expirationDateTime); handle validation handshake (validationToken); auto-renew timer before 60-min expiry; delete_subscription() on disconnect [owner:api-engineer]
- [x] [2.5] [P-1] Create crates/nv-daemon/src/teams/client.rs — TeamsClient REST client (reqwest): send_channel_message(team_id, channel_id, content) via POST /teams/{id}/channels/{id}/messages; send_chat_message(chat_id, content); Authorization Bearer header; handle 429 rate limits [owner:api-engineer]
- [x] [2.6] [P-1] Create crates/nv-daemon/src/teams/mod.rs — TeamsChannel struct implementing Channel trait: name() -> "teams", connect() acquires token + registers subscription, poll_messages() drains buffered webhook notifications, send_message() calls TeamsClient, disconnect() deletes subscription [owner:api-engineer]
- [x] [2.7] [P-2] Add /webhooks/teams route to http.rs — axum handler: validate incoming requests (validation handshake returns validationToken as text/plain), parse change notification JSON, extract message content + metadata, buffer as InboundMessage in Arc<Mutex<VecDeque>> shared with TeamsChannel [owner:api-engineer]
- [x] [2.8] [P-2] Wire TeamsChannel into main.rs — conditionally spawn when teams config + secrets present, register in channel map, connect to mpsc trigger channel, pass webhook buffer to HTTP router [owner:api-engineer]
- [x] [2.9] [P-2] Add mod teams declaration in main.rs or lib.rs [owner:api-engineer]

## Verify

- [x] [3.1] cargo build passes [owner:api-engineer]
- [x] [3.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [x] [3.3] Unit tests: OAuth token request serialization, token refresh timing (expired, near-expiry, valid) [owner:api-engineer]
- [x] [3.4] Unit tests: webhook validation handshake (return validationToken), notification payload parsing [owner:api-engineer]
- [x] [3.5] Unit tests: subscription registration request format, auto-renew scheduling [owner:api-engineer]
- [x] [3.6] Unit tests: InboundMessage conversion from Graph notification payload [owner:api-engineer]
- [x] [3.7] Existing tests pass [owner:api-engineer]
