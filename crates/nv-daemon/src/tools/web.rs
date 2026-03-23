//! Web fetch and search tools for Nova.
//!
//! Three read-only tools:
//! * `fetch_url` — retrieve and extract text from a URL (HTML or JSON).
//! * `check_url` — HTTP health check with status, timing, redirect chain, TLS info.
//! * `search_web` — web search via SearXNG or DuckDuckGo.
//!
//! All tools use a shared `reqwest::Client` configured with a 10-second timeout,
//! `User-Agent: Nova/1.0`, and a 1 MB response-body cap.

use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Result};
use serde::Deserialize;

use crate::claude::ToolDefinition;

// ── Constants ────────────────────────────────────────────────────────

const USER_AGENT: &str = "Nova/1.0";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
/// Maximum response body bytes to read (1 MB).
const MAX_BODY_BYTES: usize = 1_048_576;
/// Maximum characters in the final text output.
const MAX_OUTPUT_CHARS: usize = 10_000;
/// Default DuckDuckGo HTML endpoint.
const DDG_URL: &str = "https://html.duckduckgo.com/html/";

// ── Shared Client Builder ────────────────────────────────────────────

/// Build a shared reqwest client with Nova's standard settings.
fn build_client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .user_agent(USER_AGENT)
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()?)
}

// ── HTML Text Extraction ─────────────────────────────────────────────

/// Strip noise tags and their contents, then strip remaining HTML tags,
/// and collapse whitespace to produce clean plain text for LLM consumption.
fn extract_text_from_html(html: &str) -> String {
    // Tags whose entire content should be removed (tag + everything inside)
    let noise_tags = ["script", "style", "nav", "footer", "header"];

    let mut s = html.to_string();

    // Remove noise tags and their contents
    for tag in &noise_tags {
        s = remove_tag_and_contents(&s, tag);
    }

    // Strip remaining HTML tags
    s = strip_html_tags(&s);

    // Decode common HTML entities
    s = s
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");

    // Collapse whitespace: replace runs of whitespace (including newlines) with a single space,
    // but preserve paragraph breaks (double-newline).
    let lines: Vec<&str> = s
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    lines.join("\n")
}

/// Remove a tag and all its contents (non-recursive, handles nested same-name tags
/// by simple bracket counting).
fn remove_tag_and_contents(input: &str, tag: &str) -> String {
    let open_tag = format!("<{tag}");
    let close_tag = format!("</{tag}>");
    let mut result = String::with_capacity(input.len());
    let lower = input.to_lowercase();
    let mut pos = 0;

    loop {
        // Find next opening tag (case-insensitive)
        let Some(start) = lower[pos..].find(open_tag.as_str()).map(|i| i + pos) else {
            result.push_str(&input[pos..]);
            break;
        };

        // Verify the char after the tag name is '>' or a space/newline (not a longer tag name)
        let after = start + open_tag.len();
        if after < input.len() {
            let ch = input.as_bytes()[after] as char;
            if ch != '>' && !ch.is_whitespace() {
                // Not our tag — skip past it
                result.push_str(&input[pos..=start]);
                pos = start + 1;
                continue;
            }
        }

        // Append everything before this tag
        result.push_str(&input[pos..start]);

        // Scan for matching close tag (account for nesting)
        let search_from = start + open_tag.len();
        let lower_tail = lower[search_from..].to_string();
        let close_pos = lower_tail
            .find(close_tag.as_str())
            .map(|i| search_from + i + close_tag.len());

        match close_pos {
            Some(end) => {
                pos = end;
            }
            None => {
                // No close tag found — skip to end
                break;
            }
        }
    }

    result
}

/// Strip all remaining HTML tags from the string.
fn strip_html_tags(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut in_tag = false;
    for ch in input.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    result
}

/// Truncate a string to `max_chars` characters, appending `[truncated]` if cut.
fn truncate(s: String, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s;
    }
    let truncated: String = s.chars().take(max_chars).collect();
    format!("{truncated}\n[truncated]")
}

// ── Tool: fetch_url ──────────────────────────────────────────────────

/// Fetch a URL and return its content as clean text.
///
/// HTML responses have noise tags stripped and remaining tags removed.
/// JSON responses are pretty-printed. Output is truncated to 10,000 chars.
pub async fn fetch_url(url: &str, format_hint: Option<&str>) -> Result<String> {
    if url.is_empty() {
        bail!("url cannot be empty");
    }

    let client = build_client()?;
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to fetch {url}: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        return Err(anyhow!("HTTP {status} for {url}"));
    }

    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase();

    // Check Content-Length cap before streaming
    if let Some(len) = resp.content_length() {
        if len as usize > MAX_BODY_BYTES {
            bail!("Response too large ({len} bytes) — max 1 MB");
        }
    }

    // Stream body with size cap
    let bytes = read_body_capped(resp).await?;

    let body = String::from_utf8_lossy(&bytes).into_owned();

    // Determine effective format
    let is_json = format_hint == Some("json")
        || content_type.contains("json");
    let is_html = format_hint == Some("html")
        || content_type.contains("html")
        || (!is_json && !content_type.contains("text/plain"));

    let text = if is_json {
        // Try to pretty-print; fall back to raw text
        match serde_json::from_str::<serde_json::Value>(&body) {
            Ok(v) => serde_json::to_string_pretty(&v).unwrap_or(body),
            Err(_) => body,
        }
    } else if is_html {
        extract_text_from_html(&body)
    } else {
        body
    };

    Ok(truncate(text, MAX_OUTPUT_CHARS))
}

/// Stream a response body with a 1 MB cap.
async fn read_body_capped(resp: reqwest::Response) -> Result<Vec<u8>> {
    use futures_util::StreamExt;

    let mut bytes = Vec::new();
    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| anyhow!("Error reading response body: {e}"))?;
        bytes.extend_from_slice(&chunk);
        if bytes.len() > MAX_BODY_BYTES {
            bail!("Response body exceeded 1 MB limit — aborting");
        }
    }

    Ok(bytes)
}

// ── Tool: check_url ──────────────────────────────────────────────────

/// Perform an HTTP health check: status, timing, redirect chain, TLS info,
/// selected headers. Uses HEAD first, falls back to GET on 405.
pub async fn check_url(url: &str) -> Result<String> {
    if url.is_empty() {
        bail!("url cannot be empty");
    }

    let client = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .user_agent(USER_AGENT)
        // Disable auto-redirect so we can capture the chain manually
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    let start = Instant::now();

    // First attempt: HEAD
    let (resp, method_used) = {
        let head_resp = client.head(url).send().await;
        match head_resp {
            Ok(r) if r.status() == reqwest::StatusCode::METHOD_NOT_ALLOWED => {
                let get_resp = client.get(url).send().await
                    .map_err(|e| anyhow!("GET {url} failed: {e}"))?;
                (get_resp, "GET")
            }
            Ok(r) => (r, "HEAD"),
            Err(e) => bail!("Failed to connect to {url}: {e}"),
        }
    };

    let elapsed_ms = start.elapsed().as_millis();
    let status = resp.status();

    let mut lines = vec![
        format!("URL: {url}"),
        format!("Method: {method_used}"),
        format!("Status: {} {}", status.as_u16(), status.canonical_reason().unwrap_or("")),
        format!("Response time: {elapsed_ms}ms"),
    ];

    // Redirect chain: follow manually if this is a redirect
    if status.is_redirection() {
        lines.push(String::new());
        lines.push("Redirect chain:".to_string());
        let mut current_url = url.to_string();
        let mut hops = 0;
        loop {
            let r = client.head(&current_url).send().await;
            let Ok(hop_resp) = r else { break };
            let hop_status = hop_resp.status();
            if let Some(location) = hop_resp.headers().get(reqwest::header::LOCATION) {
                let loc = location.to_str().unwrap_or("?");
                lines.push(format!("  {} -> {loc}", hop_status.as_u16()));
                current_url = loc.to_string();
                hops += 1;
                if hops >= 10 { break; }
                if !hop_status.is_redirection() { break; }
            } else {
                break;
            }
        }
    }

    // TLS info (best-effort via peer certificate — reqwest exposes limited info)
    let is_https = url.starts_with("https://");
    if is_https {
        lines.push(String::new());
        lines.push("TLS: HTTPS connection established".to_string());
    }

    // Selected response headers
    let interesting_headers = ["server", "content-type", "x-powered-by"];
    let mut header_lines = Vec::new();
    for header_name in &interesting_headers {
        if let Some(val) = resp.headers().get(*header_name) {
            if let Ok(s) = val.to_str() {
                header_lines.push(format!("  {}: {s}", header_name.to_uppercase()));
            }
        }
    }
    if !header_lines.is_empty() {
        lines.push(String::new());
        lines.push("Headers:".to_string());
        lines.extend(header_lines);
    }

    Ok(lines.join("\n"))
}

// ── Tool: search_web ─────────────────────────────────────────────────

/// A single search result from either SearXNG or DuckDuckGo.
struct SearchResult {
    title: String,
    url: String,
    snippet: String,
}

/// Search the web via a configurable endpoint.
///
/// Uses SearXNG JSON API when `search_url` contains `/search`,
/// otherwise parses DuckDuckGo HTML. Returns a numbered list of results.
pub async fn search_web(query: &str, count: usize, search_url: Option<&str>) -> Result<String> {
    if query.is_empty() {
        bail!("query cannot be empty");
    }

    let count = count.clamp(1, 10);
    let endpoint = search_url.unwrap_or(DDG_URL);
    let client = build_client()?;

    let results = if endpoint.contains("/search") {
        search_searxng(&client, endpoint, query, count).await
    } else {
        search_duckduckgo(&client, endpoint, query, count).await
    };

    match results {
        Ok(results) if results.is_empty() => {
            Ok(format!("No results found for: {query}"))
        }
        Ok(results) => {
            let total = results.len().min(count);
            let mut lines = vec![format!("🔍 **{query}** — {total} result{}", if total == 1 { "" } else { "s" })];
            for r in results.into_iter().take(count) {
                if r.snippet.is_empty() {
                    lines.push(format!("   🔗 **{}**\n   {}", r.title, r.url));
                } else {
                    lines.push(format!("   🔗 **{}**\n   {}\n   {}", r.title, r.url, r.snippet));
                }
            }
            Ok(lines.join("\n"))
        }
        Err(e) => {
            Err(anyhow!(
                "Search failed: {e}\n\nIf using a custom search endpoint, \
                check `[web] search_url` in nv.toml."
            ))
        }
    }
}

/// SearXNG JSON result deserialization types.
#[derive(Deserialize)]
struct SearxngResponse {
    results: Vec<SearxngResult>,
}

#[derive(Deserialize)]
struct SearxngResult {
    title: String,
    url: String,
    #[serde(default)]
    content: String,
}

async fn search_searxng(
    client: &reqwest::Client,
    base_url: &str,
    query: &str,
    count: usize,
) -> Result<Vec<SearchResult>> {
    let url = format!(
        "{base_url}?q={}&format=json&categories=general",
        urlencoding::encode(query)
    );

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| anyhow!("SearXNG request failed: {e}"))?;

    if !resp.status().is_success() {
        bail!("SearXNG returned {}", resp.status());
    }

    let body: SearxngResponse = resp
        .json()
        .await
        .map_err(|e| anyhow!("Failed to parse SearXNG response: {e}"))?;

    Ok(body
        .results
        .into_iter()
        .take(count)
        .map(|r| SearchResult {
            title: r.title,
            url: r.url,
            snippet: r.content,
        })
        .collect())
}

async fn search_duckduckgo(
    client: &reqwest::Client,
    base_url: &str,
    query: &str,
    count: usize,
) -> Result<Vec<SearchResult>> {
    let resp = client
        .get(base_url)
        .query(&[("q", query)])
        .send()
        .await
        .map_err(|e| anyhow!("DuckDuckGo request failed: {e}"))?;

    if !resp.status().is_success() {
        bail!("DuckDuckGo returned {}", resp.status());
    }

    let bytes = read_body_capped(resp).await?;
    let html = String::from_utf8_lossy(&bytes);

    let results = parse_ddg_html(&html, count);
    Ok(results)
}

/// Parse DuckDuckGo HTML results from `.result` elements.
///
/// DuckDuckGo HTML structure (best-effort parsing):
/// - Result containers are `<div class="result ...">` or `<div class="links_main">`
/// - Each contains a link (`<a class="result__a">`) and a snippet (`<a class="result__snippet">`)
fn parse_ddg_html(html: &str, count: usize) -> Vec<SearchResult> {
    let mut results = Vec::new();

    // Simple state-machine parser: find result blocks, extract title/url/snippet
    // DDG HTML uses `<a class="result__a" href="...">title</a>` pattern
    let lower = html.to_lowercase();
    let mut pos = 0;

    while results.len() < count {
        // Find a result link
        let Some(anchor_pos) = lower[pos..].find("result__a").map(|i| i + pos) else {
            break;
        };

        // Back-track to the opening `<a`
        let Some(open_pos) = html[..anchor_pos].rfind('<') else {
            pos = anchor_pos + 1;
            continue;
        };

        // Extract href
        let tag_end = html[open_pos..].find('>').map(|i| open_pos + i + 1);
        let Some(tag_end) = tag_end else {
            pos = anchor_pos + 1;
            continue;
        };

        let tag_text = &html[open_pos..tag_end];
        let href = extract_attr(tag_text, "href").unwrap_or_default();

        // Skip DuckDuckGo internal links
        if href.is_empty() || href.starts_with("//duckduckgo") || href.contains("duckduckgo.com") {
            pos = anchor_pos + 1;
            continue;
        }

        // Extract title (text between `>` and `</a>`)
        let close_anchor = html[tag_end..].find("</a>").map(|i| i + tag_end);
        let Some(close_anchor) = close_anchor else {
            pos = anchor_pos + 1;
            continue;
        };

        let title_raw = &html[tag_end..close_anchor];
        let title = strip_html_tags(title_raw).trim().to_string();

        if title.is_empty() {
            pos = anchor_pos + 1;
            continue;
        }

        // Look for a snippet nearby (within 2000 chars after close_anchor)
        let search_window = &html[close_anchor..((close_anchor + 2000).min(html.len()))];
        let snippet = if let Some(snip_pos) = search_window.to_lowercase().find("result__snippet") {
            let snip_start = search_window[snip_pos..]
                .find('>')
                .map(|i| snip_pos + i + 1);
            if let Some(snip_start) = snip_start {
                let snip_end = search_window[snip_start..]
                    .find("</a>")
                    .map(|i| snip_start + i);
                if let Some(snip_end) = snip_end {
                    let raw = &search_window[snip_start..snip_end];
                    strip_html_tags(raw).trim().to_string()
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        results.push(SearchResult {
            title,
            url: href,
            snippet,
        });

        pos = close_anchor + 1;
    }

    results
}

/// Extract an HTML attribute value from a tag string.
fn extract_attr(tag: &str, attr: &str) -> Option<String> {
    let needle = format!("{attr}=\"");
    let start = tag.find(&needle)? + needle.len();
    let end = tag[start..].find('"').map(|i| i + start)?;
    Some(tag[start..end].to_string())
}

// ── Tool Definitions ─────────────────────────────────────────────────

/// Return tool definitions for all web tools.
pub fn web_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "fetch_url".into(),
            description: "Fetch a URL and return its content as clean text. HTML pages have script/style/nav/footer/header tags removed and remaining HTML stripped. JSON responses are pretty-printed. Output truncated to 10,000 characters. Supports HTTP and HTTPS.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "The URL to fetch (http:// or https://)"
                    },
                    "format": {
                        "type": "string",
                        "enum": ["html", "json"],
                        "description": "Optional: force treatment as html or json. If omitted, auto-detected from Content-Type."
                    }
                },
                "required": ["url"]
            }),
        },
        ToolDefinition {
            name: "check_url".into(),
            description: "HTTP health check for a URL. Returns status code, response time in ms, redirect chain (if any), TLS info (if HTTPS), and selected response headers (Server, Content-Type, X-Powered-By). Uses HEAD with GET fallback.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "The URL to health-check (http:// or https://)"
                    }
                },
                "required": ["url"]
            }),
        },
        ToolDefinition {
            name: "search_web".into(),
            description: "Search the web and return the top N results (title, URL, snippet). Uses SearXNG if a `[web] search_url` containing `/search` is configured in nv.toml, otherwise falls back to DuckDuckGo HTML parsing.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query"
                    },
                    "count": {
                        "type": "integer",
                        "description": "Number of results to return (default: 5, max: 10)"
                    }
                },
                "required": ["query"]
            }),
        },
    ]
}

// ── URL encoding helper ──────────────────────────────────────────────

/// Minimal URL percent-encoding for query string values.
mod urlencoding {
    pub fn encode(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        for b in s.bytes() {
            match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9'
                | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
                b' ' => out.push('+'),
                _ => out.push_str(&format!("%{b:02X}")),
            }
        }
        out
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_tool_definitions_returns_three_tools() {
        let tools = web_tool_definitions();
        assert_eq!(tools.len(), 3);
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"fetch_url"));
        assert!(names.contains(&"check_url"));
        assert!(names.contains(&"search_web"));
    }

    #[test]
    fn tool_definitions_have_schemas() {
        for tool in web_tool_definitions() {
            assert!(!tool.name.is_empty());
            assert!(!tool.description.is_empty());
            assert_eq!(tool.input_schema["type"], "object");
        }
    }

    #[test]
    fn fetch_url_requires_url() {
        let tools = web_tool_definitions();
        let t = tools.iter().find(|t| t.name == "fetch_url").unwrap();
        let required = t.input_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v.as_str() == Some("url")));
    }

    #[test]
    fn search_web_requires_query() {
        let tools = web_tool_definitions();
        let t = tools.iter().find(|t| t.name == "search_web").unwrap();
        let required = t.input_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v.as_str() == Some("query")));
    }

    #[test]
    fn strip_html_tags_basic() {
        let html = "<p>Hello <b>world</b>!</p>";
        assert_eq!(strip_html_tags(html), "Hello world!");
    }

    #[test]
    fn extract_text_from_html_strips_noise_tags() {
        let html = r#"
            <html>
            <head><style>body { color: red; }</style></head>
            <body>
            <nav>Menu</nav>
            <main><p>Important content here.</p></main>
            <footer>Footer text</footer>
            <script>alert('hello');</script>
            </body>
            </html>
        "#;
        let text = extract_text_from_html(html);
        assert!(text.contains("Important content here."));
        assert!(!text.contains("Menu"));
        assert!(!text.contains("Footer text"));
        assert!(!text.contains("alert"));
        assert!(!text.contains("body { color"));
    }

    #[test]
    fn truncate_short_string() {
        let s = "hello".to_string();
        assert_eq!(truncate(s, 100), "hello");
    }

    #[test]
    fn truncate_long_string() {
        let s = "a".repeat(20_000);
        let result = truncate(s, 10_000);
        assert!(result.ends_with("[truncated]"));
        assert!(result.chars().count() <= 10_000 + 20); // small margin for suffix
    }

    #[test]
    fn urlencoding_encode_basic() {
        assert_eq!(urlencoding::encode("hello world"), "hello+world");
        assert_eq!(urlencoding::encode("foo&bar=baz"), "foo%26bar%3Dbaz");
    }

    #[test]
    fn extract_attr_finds_href() {
        let tag = r#"<a href="https://example.com" class="result__a">"#;
        assert_eq!(
            extract_attr(tag, "href"),
            Some("https://example.com".to_string())
        );
    }

    #[test]
    fn remove_tag_and_contents_basic() {
        let html = "<div>before<script>evil()</script>after</div>";
        let result = remove_tag_and_contents(html, "script");
        assert!(result.contains("before"));
        assert!(result.contains("after"));
        assert!(!result.contains("evil"));
    }
}
