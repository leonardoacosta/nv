# Capability: Missing Daemon API Routes

## ADDED Requirements

### Requirement: GET /api/obligations Endpoint
The Rust HTTP router in `crates/nv-daemon/src/http.rs` SHALL expose `GET /api/obligations`. It MUST accept optional query params `status` (one of "open", "in_progress", "done", "dismissed") and `owner` (one of "nova", "leo"). When both are provided, both filters apply (AND semantics). Response MUST be `{ "obligations": [...] }` using the serialized `nv_core::types::Obligation` struct. The handler reuses the existing `ObligationStore::list_by_status`, `list_by_owner`, and `list_all` methods (currently `#[allow(dead_code)]`).

#### Scenario: Filter by owner and status
Given obligations exist with owners "nova" and "leo" in various statuses, when `GET /api/obligations?owner=leo&status=open` is called, then the response is `{ "obligations": [...] }` containing only obligations matching both filters, HTTP 200.

#### Scenario: No filter returns all obligations
Given three obligations in various states and owners, when `GET /api/obligations` is called with no query params, then all three obligations appear in the response array.

#### Scenario: Unknown owner param returns 400
Given a request `GET /api/obligations?owner=robot`, then the response is HTTP 400 with `{ "error": "unknown owner: robot" }`.

### Requirement: PATCH /api/obligations/:id Endpoint
The router SHALL expose `PATCH /api/obligations/:id`. It MUST accept request body `{ "status": "open" | "in_progress" | "done" | "dismissed" }`, call `ObligationStore::update_status`, and return `{ "id": "...", "status": "..." }`. On missing obligation ID the handler MUST return HTTP 404. On success it MUST broadcast `DaemonEvent::ApprovalUpdated` (matching the existing approve handler pattern).

#### Scenario: Dismiss an open obligation
Given obligation `abc123` with status "open", when `PATCH /api/obligations/abc123` is sent with body `{ "status": "dismissed" }`, then the response is `{ "id": "abc123", "status": "dismissed" }` and the record is updated in the store.

#### Scenario: Patch unknown ID returns 404
Given no obligation exists with id "missing", when `PATCH /api/obligations/missing` is sent, then HTTP 404 is returned with `{ "error": "obligation missing not found" }`.

### Requirement: GET /api/projects Endpoint
The router SHALL expose `GET /api/projects`. It MUST return `{ "projects": [{ "code": "...", "path": "..." }, ...] }` from the daemon's project registry. The project registry is a `HashMap<String, PathBuf>` added as `project_registry: HashMap<String, PathBuf>` to `HttpState` and populated in `main.rs`. If the registry is empty or not configured, the response MUST be `{ "projects": [] }` with HTTP 200.

#### Scenario: Registry returns configured projects
Given the daemon was started with registry `{ "nv": "/home/nyaptor/nv" }`, when `GET /api/projects` is called, then `{ "projects": [{ "code": "nv", "path": "/home/nyaptor/nv" }] }` is returned.

#### Scenario: Empty registry returns empty list
Given no project registry was configured, when `GET /api/projects` is called, then `{ "projects": [] }` is returned with HTTP 200.

### Requirement: GET /api/config Endpoint
The router SHALL expose `GET /api/config`. It MUST return the daemon's loaded configuration as a JSON object (`Record<string, unknown>`). Secret fields — any key whose leaf name contains "token", "secret", "password", "key", "api_key", or "auth" — MUST be replaced with the string `"***"` before serializing. If no config file is loaded, the response MUST be `{}` with HTTP 200. The router SHALL also expose `PUT /api/config` accepting `{ "fields": { ... } }` to write config fields.

#### Scenario: Config masks secret fields
Given daemon config contains `{ "anthropic": { "api_key": "sk-ant-real" } }`, when `GET /api/config` is called, then the response is `{ "anthropic": { "api_key": "***" } }` with the real value absent.

#### Scenario: Missing config file returns empty object
Given the daemon was started without a config file, when `GET /api/config` is called, then `{}` is returned with HTTP 200, not a 500 or 502 error.
