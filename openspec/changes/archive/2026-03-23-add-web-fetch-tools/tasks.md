# Implementation Tasks

<!-- beads:epic:TBD -->

## Configuration

- [x] [1.1] [P-1] Add `WebConfig` struct to `crates/nv-core/src/config.rs` — `search_url: Option<String>` with default `https://html.duckduckgo.com/html/`, add default value function [owner:api-engineer]
- [x] [1.2] [P-1] Add `pub web: Option<WebConfig>` field to root `Config` struct in `crates/nv-core/src/config.rs` [owner:api-engineer]

## Web Tools Module

- [x] [2.1] [P-1] Create `crates/nv-daemon/src/web_tools.rs` with shared HTTP client builder — 10s timeout, `User-Agent: Nova/1.0`, 1MB max response size constant, redirect policy (max 10) [owner:api-engineer]
- [x] [2.2] [P-1] Implement `fetch_url()` — accept url + optional format, auto-detect HTML vs JSON from Content-Type, stream body with 1MB cap, return text or pretty-printed JSON, truncate to 10K chars with `[truncated]` suffix [owner:api-engineer]
- [x] [2.3] [P-1] Implement HTML text extraction helper — strip `<script>`, `<style>`, `<nav>`, `<footer>`, `<header>` tags and contents, strip remaining HTML tags, collapse whitespace [owner:api-engineer]
- [x] [2.4] [P-1] Implement `check_url()` — HEAD request with GET fallback on 405, collect status code, response time (ms), redirect chain (status + location per hop), TLS cert info (subject, issuer, expiry), selected headers (Server, Content-Type, X-Powered-By), format as text [owner:api-engineer]
- [x] [2.5] [P-1] Implement `search_web()` — accept query + count (default 5, max 10), resolve search_url from `WebConfig`, SearXNG JSON mode when URL contains `/search`, DuckDuckGo HTML fallback, return numbered list of title/url/snippet [owner:api-engineer]
- [x] [2.6] [P-1] Implement `web_tool_definitions()` returning `Vec<ToolDefinition>` for `fetch_url`, `check_url`, `search_web` with full input schemas [owner:api-engineer]

## Registration and Dispatch

- [x] [3.1] [P-1] Add `mod web_tools;` to `crates/nv-daemon/src/main.rs` [owner:api-engineer]
- [x] [3.2] [P-1] Add `tools.extend(web_tools::web_tool_definitions())` in `register_tools()` in `tools.rs` [owner:api-engineer]
- [x] [3.3] [P-1] Add dispatch arms for `"fetch_url"`, `"check_url"`, `"search_web"` in `execute_tool_send()` — all return `ToolResult::Immediate`, pass `WebConfig` (or search_url) where needed [owner:api-engineer]
- [x] [3.4] [P-1] Add dispatch arms for `"fetch_url"`, `"check_url"`, `"search_web"` in `execute_tool()` — mirror `execute_tool_send` [owner:api-engineer]

## Verify

- [x] [4.1] `cargo build` passes [owner:api-engineer]
- [x] [4.2] `cargo clippy -- -D warnings` passes [owner:api-engineer]
- [x] [4.3] `cargo test` — existing tests pass [owner:api-engineer]
