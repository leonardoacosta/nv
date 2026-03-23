# Proposal: Web Fetch Tools

## Change ID
`add-web-fetch-tools`

## Summary

Add three read-only web tools to the Nova daemon: `fetch_url` (retrieve and extract text from web
pages), `check_url` (HTTP health check with status/timing/TLS info), and `search_web` (web search
via configurable SearXNG or DuckDuckGo endpoint). All tools use reqwest (already a dependency),
require no confirmation (read-only), and live in a new `web_tools.rs` module.

## Context
- Extends: `crates/nv-daemon/src/tools.rs` (tool registration + dispatch), `crates/nv-core/src/config.rs` (config structs), `crates/nv-daemon/src/main.rs` (mod declaration)
- New file: `crates/nv-daemon/src/web_tools.rs` — HTTP client, HTML text extraction, search
- Depends on: `reqwest` (already in Cargo.toml workspace deps)
- Related: existing tool modules follow the pattern of `*_tool_definitions()` + public functions dispatched from `execute_tool`/`execute_tool_send`

## Motivation

Nova currently has no ability to fetch web content, check if a URL is alive, or search the web.
Users frequently ask Nova to look things up, verify links, or check service status. Without these
tools, Nova must refuse or hallucinate. Adding lightweight HTTP tools gives Nova factual grounding
from the live web.

## Requirements

### Req-1: `fetch_url` — Fetch URL and Extract Text

Fetch a URL and return its content as clean text. Supports HTML and JSON responses.

- **Input**: `url` (string, required), `format` (string, optional: `"html"` | `"json"`, default: auto-detect from Content-Type)
- **HTML handling**: Strip all `<script>`, `<style>`, `<nav>`, `<footer>`, `<header>` tags and their contents, then strip remaining HTML tags. Collapse whitespace. Return plain text.
- **JSON handling**: Pretty-print the JSON response body.
- **Truncation**: Truncate output to 10,000 characters with a `[truncated]` suffix.
- **Timeout**: 10 second request timeout.
- **Max response size**: 1MB — abort and return an error if Content-Length exceeds this or if the streamed body exceeds 1MB.
- **User-Agent**: `"Nova/1.0"`
- **Redirects**: Follow up to 10 redirects (reqwest default).
- **Error handling**: Return human-readable errors for DNS failures, timeouts, TLS errors, non-2xx status codes.

### Req-2: `check_url` — HTTP Health Check

Perform an HTTP health check on a URL and return diagnostic information.

- **Input**: `url` (string, required)
- **Method**: Send HEAD first. If HEAD returns 405 Method Not Allowed, fall back to GET.
- **Output** (formatted text):
  - Status code and reason phrase
  - Response time in milliseconds
  - Redirect chain (each hop: status + location)
  - TLS certificate info: subject CN, issuer, expiry date (if HTTPS)
  - Selected response headers: `Server`, `Content-Type`, `X-Powered-By`
- **Timeout**: 10 second request timeout.
- **User-Agent**: `"Nova/1.0"`
- **No body processing** — this is a lightweight connectivity/certificate check.

### Req-3: `search_web` — Web Search

Search the web via a configurable search endpoint and return structured results.

- **Input**: `query` (string, required), `count` (integer, optional, default: 5, max: 10)
- **Backend**: Configurable via `[web]` section in `nv.toml`:
  - `search_url` — base URL of SearXNG instance or DuckDuckGo HTML endpoint
  - Default: DuckDuckGo HTML (`https://html.duckduckgo.com/html/`)
- **SearXNG mode** (when `search_url` contains `/search`): GET `{search_url}?q={query}&format=json&categories=general`. Parse JSON response `results[]` array — extract `title`, `url`, `content` (snippet).
- **DuckDuckGo mode** (fallback): GET with `q={query}` parameter. Parse HTML response to extract result links, titles, and snippets from `.result` elements.
- **Output**: Numbered list of results, each with title, URL, and snippet. Top N results where N = `count`.
- **Timeout**: 10 second request timeout.
- **User-Agent**: `"Nova/1.0"`
- **Error handling**: If the search endpoint is unreachable or returns an error, return a clear message suggesting the user check `[web] search_url` in `nv.toml`.

### Req-4: Configuration — `WebConfig`

Add a `[web]` section to the config:

```toml
[web]
search_url = "https://html.duckduckgo.com/html/"
```

- `search_url` (string, optional) — defaults to DuckDuckGo HTML endpoint
- The `[web]` section itself is optional — all web tools work with defaults when omitted
- Add `pub web: Option<WebConfig>` to the root `Config` struct in `nv-core`

### Req-5: Tool Registration and Dispatch

- Add `web_tool_definitions()` in `web_tools.rs` returning `Vec<ToolDefinition>` for all 3 tools
- Call `tools.extend(web_tools::web_tool_definitions())` in `register_tools()`
- Add dispatch arms for `"fetch_url"`, `"check_url"`, `"search_web"` in both `execute_tool` and `execute_tool_send`
- All three return `ToolResult::Immediate` (read-only, no confirmation needed)
- Add `mod web_tools;` to `main.rs`

## Scope
- **IN**: `fetch_url` with HTML text extraction and JSON support, `check_url` with HEAD/GET fallback and TLS info, `search_web` with SearXNG/DuckDuckGo backends, `WebConfig` in nv-core, tool registration + dispatch
- **OUT**: JavaScript rendering (no headless browser), caching/rate-limiting (future), POST/PUT/DELETE requests, authentication headers, cookie handling, screenshot capture

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/web_tools.rs` | New module: HTTP client, HTML text extraction, search parsing, `web_tool_definitions()` |
| `crates/nv-daemon/src/tools.rs` | Register 3 tool definitions, add dispatch arms in both `execute_tool` and `execute_tool_send` |
| `crates/nv-core/src/config.rs` | Add `WebConfig` struct, add `pub web: Option<WebConfig>` to `Config` |
| `crates/nv-daemon/src/main.rs` | Add `mod web_tools;` |

## Risks
| Risk | Mitigation |
|------|-----------|
| Large HTML pages exceed memory limits | 1MB hard cap on response body, streamed read with early abort |
| HTML text extraction misses content or includes junk | Strip known noise tags (script, style, nav, footer, header) before tag removal. Good enough for LLM consumption — not a browser. |
| DuckDuckGo HTML parsing breaks on layout changes | SearXNG is the primary recommended backend with stable JSON API. DDG is a fallback with best-effort parsing. |
| Search endpoint unreachable in air-gapped environments | Tools return clear error messages. `search_web` suggests checking config. `fetch_url` and `check_url` work independently. |
| Timeout on slow servers blocks the worker | 10s timeout per request. Worker thread is async — other workers unaffected. |
| TLS cert extraction requires native-tls or rustls introspection | Use reqwest's built-in redirect policy tracking for redirect chain. For TLS, extract from the connection info if available, or parse the certificate via `native-tls` — degrade gracefully if unavailable. |
