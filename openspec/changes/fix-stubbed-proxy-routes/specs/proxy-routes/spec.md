# Capability: Proxy Routes — Replace 501 Stubs

## MODIFIED Requirements

### Requirement: config route.ts — GET and PUT proxy to daemon
`apps/dashboard/app/api/config/route.ts` MUST replace the 501 stub with handlers that forward to the daemon using `daemonFetch` from `@/lib/daemon`. `GET` SHALL call `daemonFetch("/api/config")`, read the JSON body, and return `NextResponse.json(data, { status: res.status })`. `PUT` SHALL read the request JSON body, call `daemonFetch("/api/config", { method: "PUT", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) })`, and return the daemon response. Both handlers MUST catch network errors and return `{ error: "Daemon unreachable" }` with status 502.

#### Scenario: GET proxied to daemon config
Given the daemon is running and returns `{ "fields": [{ "key": "anthropic.api_key", "value": "***" }] }`, when `GET /api/config` is called on the dashboard, then the response is the daemon payload with status 200.

#### Scenario: PUT proxied with body forwarded
Given a PUT request with body `{ "fields": { "log_level": "debug" } }`, when `PUT /api/config` is called, then `daemonFetch` is called with method `PUT`, the body is forwarded, and the daemon response is returned.

#### Scenario: daemon unreachable returns 502
Given the daemon is not running, when `GET /api/config` or `PUT /api/config` is called, then the catch block returns `{ "error": "Daemon unreachable" }` with status 502.

### Requirement: projects route.ts — GET proxy to daemon
`apps/dashboard/app/api/projects/route.ts` MUST replace the 501 stub with a handler that calls `daemonFetch("/api/projects")`, reads the JSON body, and returns `NextResponse.json(data, { status: res.status })`. Network errors MUST return `{ error: "Daemon unreachable" }` with status 502.

#### Scenario: GET proxied to daemon projects
Given the daemon is running and returns `{ "projects": [{ "code": "nv", "path": "/home/nyaptor/nv" }] }`, when `GET /api/projects` is called on the dashboard, then the daemon response is returned with status 200.

#### Scenario: daemon unreachable returns 502
Given the daemon is not running, when `GET /api/projects` is called, then the response is `{ "error": "Daemon unreachable" }` with status 502.

## ADDED Requirements

### Requirement: daemon GET /api/memory endpoint
`crates/nv-daemon/src/http.rs` SHALL expose `GET /api/memory`. Without a `?topic` query param the handler MUST list all topic names from the memory directory and return `{ "topics": ["<name>", ...] }` with HTTP 200. With `?topic=<name>` it MUST read that topic file and return `{ "topic": "<name>", "content": "<file contents>" }` with HTTP 200. If the topic does not exist, it MUST return HTTP 404 with `{ "error": "Topic not found" }`. The handler MUST use the `Memory` instance or base path exposed via `HttpState`.

#### Scenario: list topics returns all topic names
Given the memory directory contains `projects.md` and `decisions.md`, when `GET /api/memory` is called without query params, then the response is `{ "topics": ["projects", "decisions"] }` (or with extensions, matching what `Memory` exposes) with HTTP 200.

#### Scenario: read existing topic
Given topic `projects` exists in the memory directory, when `GET /api/memory?topic=projects` is called, then the response is `{ "topic": "projects", "content": "<file contents>" }` with HTTP 200.

#### Scenario: read missing topic returns 404
Given no topic named `nonexistent` exists, when `GET /api/memory?topic=nonexistent` is called, then the response is `{ "error": "Topic not found" }` with HTTP 404.

### Requirement: daemon PUT /api/memory endpoint
`crates/nv-daemon/src/http.rs` SHALL expose `PUT /api/memory`. The handler MUST accept a JSON body `{ "topic": "<name>", "content": "<string>" }`, write the content to the memory directory using `Memory` or direct file I/O, and return `{ "topic": "<name>", "written": <byte_count> }` with HTTP 200. Write failures MUST return HTTP 500 with `{ "error": "<message>" }`.

#### Scenario: write topic succeeds
Given body `{ "topic": "projects", "content": "# Projects\n\nsome content" }`, when `PUT /api/memory` is called, then the file is written and the response is `{ "topic": "projects", "written": <N> }` with HTTP 200.

#### Scenario: write failure returns 500
Given a permissions error prevents writing, when `PUT /api/memory` is called, then the response is HTTP 500 with `{ "error": "<error message>" }`.

### Requirement: HttpState exposes memory base path
`HttpState` in `crates/nv-daemon/src/http.rs` (or `state.rs`) MUST expose the memory base path (as `memory_base_path: PathBuf` or equivalent) so that the new `GET /api/memory` and `PUT /api/memory` handlers can construct a `Memory` instance or read/write files without depending on process-global state.

#### Scenario: handler constructs Memory from state
Given `HttpState.memory_base_path` is set to `~/.nv/memory`, when a memory handler is invoked, then `Memory::from_base_path(&state.memory_base_path)` (or equivalent) is used to access memory files.

### Requirement: memory route.ts — GET and PUT proxy to daemon
`apps/dashboard/app/api/memory/route.ts` MUST replace the 501 stub with proxy handlers. `GET` SHALL forward the optional `?topic=` query param: if present, call `daemonFetch("/api/memory?topic=<value>")`, otherwise call `daemonFetch("/api/memory")`. `PUT` SHALL read the request JSON body and call `daemonFetch("/api/memory", { method: "PUT", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) })`. Both handlers MUST catch network errors and return `{ error: "Daemon unreachable" }` with status 502.

#### Scenario: GET without topic forwards to list endpoint
Given `GET /api/memory` with no query string, when the proxy handler is called, then `daemonFetch("/api/memory")` is called and the daemon response is returned.

#### Scenario: GET with topic param forwards correctly
Given `GET /api/memory?topic=projects`, when the proxy handler is called, then `daemonFetch("/api/memory?topic=projects")` is called and the daemon response is returned.

#### Scenario: PUT forwards body to daemon
Given a PUT request with body `{ "topic": "projects", "content": "# Projects" }`, when `PUT /api/memory` is called, then `daemonFetch` is called with method PUT and the body is forwarded unchanged.

### Requirement: daemon POST /api/solve endpoint
`crates/nv-daemon/src/http.rs` SHALL expose `POST /api/solve`. The handler MUST accept a JSON body `{ "project": "<code>", "error": "<message>", "context": "<optional string>" }` and return `{ "session_id": "<uuid>" }` with HTTP 200. The `context` field is optional. For this spec a minimal implementation that generates a UUID and returns it synchronously is acceptable; full session orchestration is out of scope.

#### Scenario: solve request returns session_id
Given body `{ "project": "nv", "error": "build failed", "context": "Cargo.toml:12" }`, when `POST /api/solve` is called, then the response is `{ "session_id": "<uuid>" }` with HTTP 200.

#### Scenario: solve request without context field
Given body `{ "project": "nv", "error": "type error" }` (no `context`), when `POST /api/solve` is called, then the response is still `{ "session_id": "<uuid>" }` with HTTP 200.

### Requirement: solve route.ts — POST proxy to daemon
`apps/dashboard/app/api/solve/route.ts` MUST replace the 501 stub with a handler that reads the request JSON body and calls `daemonFetch("/api/solve", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) })`. Network errors MUST return `{ error: "Daemon unreachable" }` with status 502.

#### Scenario: POST proxied with body forwarded
Given body `{ "project": "nv", "error": "some error" }`, when `POST /api/solve` is called, then `daemonFetch` is called with method POST, the body is forwarded, and the daemon response is returned.

#### Scenario: daemon unreachable returns 502
Given the daemon is not running, when `POST /api/solve` is called, then the response is `{ "error": "Daemon unreachable" }` with status 502.
