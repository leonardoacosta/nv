//! In-memory TTL-based cache for tool results.
//!
//! [`ToolResultCache`] sits in front of `execute_tool_send_with_backend` in
//! `worker.rs`. Read-only tools return cached results for repeated identical
//! calls within their TTL window. Write tools bypass the cache and invalidate
//! related read-cache entries on success.
//!
//! Cache key: `(tool_name, hash_of_serde_json_input)`.
//! No background eviction — expired entries are evicted lazily on [`get`].

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ── Cache internals ──────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    tool_name: String,
    input_hash: u64,
}

struct CacheEntry {
    value: String,
    expires_at: Instant,
}

// ── Public API ───────────────────────────────────────────────────────

/// Session-scoped in-memory cache for tool results.
///
/// Cheap to clone — wraps an `Arc<Mutex<...>>` internally.
#[derive(Clone)]
pub struct ToolResultCache {
    entries: Arc<Mutex<HashMap<CacheKey, CacheEntry>>>,
}

impl ToolResultCache {
    /// Construct an empty cache.
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Look up a cached result.
    ///
    /// Returns `Some(value)` if an unexpired entry exists for `(tool_name,
    /// input)`. Expired entries are evicted lazily and `None` is returned.
    pub fn get(&self, tool_name: &str, input: &serde_json::Value) -> Option<String> {
        let key = make_key(tool_name, input);
        let mut map = self.entries.lock().unwrap();
        match map.get(&key) {
            Some(entry) if Instant::now() < entry.expires_at => {
                Some(entry.value.clone())
            }
            Some(_expired) => {
                // Lazy eviction of the expired entry.
                map.remove(&key);
                None
            }
            None => None,
        }
    }

    /// Store a result with a TTL.
    pub fn insert(
        &self,
        tool_name: &str,
        input: &serde_json::Value,
        value: String,
        ttl: Duration,
    ) {
        let key = make_key(tool_name, input);
        let entry = CacheEntry {
            value,
            expires_at: Instant::now() + ttl,
        };
        self.entries.lock().unwrap().insert(key, entry);
    }

    /// Remove all entries whose `tool_name` starts with `prefix`.
    ///
    /// Used for write-invalidation: e.g., after `jira_create_issue`, remove
    /// all `jira_*` cached entries.
    pub fn invalidate_prefix(&self, prefix: &str) {
        self.entries
            .lock()
            .unwrap()
            .retain(|k, _| !k.tool_name.starts_with(prefix));
    }

    /// Clear all entries (session reset).
    #[allow(dead_code)]
    pub fn clear(&self) {
        self.entries.lock().unwrap().clear();
    }
}

impl Default for ToolResultCache {
    fn default() -> Self {
        Self::new()
    }
}

// ── Key construction ─────────────────────────────────────────────────

fn make_key(tool_name: &str, input: &serde_json::Value) -> CacheKey {
    let mut hasher = DefaultHasher::new();
    // Serialize to a canonical string so key is independent of JSON field order.
    serde_json::to_string(input)
        .unwrap_or_default()
        .hash(&mut hasher);
    CacheKey {
        tool_name: tool_name.to_string(),
        input_hash: hasher.finish(),
    }
}

// ── TTL policy ───────────────────────────────────────────────────────

/// Return the cache TTL for a given tool name, or `None` to skip caching.
///
/// Conservative defaults: only well-understood read-only tools are cached.
/// All write/mutation tools return `None`. Any tool not explicitly listed
/// returns `None` (opt-in only).
pub fn cache_ttl_for_tool(tool_name: &str) -> Option<Duration> {
    // 5 minutes
    const MIN5: Duration = Duration::from_secs(5 * 60);
    // 3 minutes
    const MIN3: Duration = Duration::from_secs(3 * 60);
    // 10 minutes
    const MIN10: Duration = Duration::from_secs(10 * 60);
    // 1 minute
    const MIN1: Duration = Duration::from_secs(60);
    // 2 minutes
    const MIN2: Duration = Duration::from_secs(2 * 60);

    match tool_name {
        // Obligation store — read-only
        "query_obligations" | "list_obligations" => Some(MIN5),

        // Jira — read tools only (write tools explicitly return None below)
        t if t.starts_with("jira_")
            && !matches!(
                t,
                "jira_create_issue" | "jira_update_issue" | "jira_transition_issue"
                    | "jira_create" | "jira_transition" | "jira_assign" | "jira_comment"
            ) =>
        {
            Some(MIN5)
        }

        // GitHub — CI state changes frequently
        "gh_pr_list" | "gh_run_status" | "gh_issues" | "gh_releases" => Some(MIN3),

        // GitHub — PR content is stable
        "gh_pr_detail" | "gh_pr_diff" | "gh_compare" => Some(MIN10),

        // Sentry
        "sentry_issues" | "sentry_issue" => Some(MIN5),

        // Vercel — deploy state changes frequently
        "vercel_deployments" | "vercel_logs" => Some(MIN3),

        // Neon — infra metadata is stable
        "neon_projects" | "neon_branches" | "neon_compute" => Some(MIN10),

        // Docker — container state can change quickly
        "docker_status" | "docker_logs" => Some(MIN1),

        // Doppler — secrets rarely change
        "doppler_secrets" | "doppler_compare" | "doppler_activity" => Some(MIN10),

        // Cloudflare — DNS rarely changes
        t if t.starts_with("cloudflare_") => Some(MIN10),

        // PostHog — feature flags are stable within session
        "posthog_flags" => Some(MIN5),

        // Google Calendar — read-only
        "calendar_today" | "calendar_upcoming" | "calendar_next" => Some(MIN5),

        // Home Assistant — home state changes frequently
        "ha_states" | "ha_entity" => Some(MIN1),

        // Plaid — financial data read-only
        "plaid_balances" | "plaid_bills" => Some(MIN5),

        // Stripe — read-only queries
        "stripe_customers" | "stripe_invoices" => Some(MIN5),

        // Upstash — cache/queue state changes
        "upstash_info" | "upstash_keys" => Some(MIN2),

        // Resend — email log read-only
        "resend_emails" | "resend_bounces" => Some(MIN5),

        // neon_query may have side effects — no cache
        "neon_query" => None,

        // Web content is not stable
        "fetch_url" | "check_url" | "search_web" => None,

        // All other tools (including write/mutation tools) — conservative: no cache
        _ => None,
    }
}

// ── Write-invalidation map ───────────────────────────────────────────

/// Return the cache key prefix to invalidate after a successful write tool call,
/// or `None` if the tool has no corresponding read-cache entries.
pub fn invalidation_prefix_for_tool(tool_name: &str) -> Option<&'static str> {
    match tool_name {
        "patch_obligation" | "create_obligation" | "close_obligation" => {
            // Invalidate both "query_obligation*" and "list_obligation*"
            // We use the common prefix "obligation" — but since these are
            // stored separately, we emit two prefixes. The function returns
            // a single prefix, so we use the shared stem.
            // query_obligations and list_obligations both start with a unique
            // prefix; we invalidate at the shortest shared prefix.
            // Both start with either "query_obligation" or "list_obligation".
            // We invalidate "query_obligation" and "list_obligation" separately
            // by returning "obligation" — but that would also wipe unrelated
            // "obligation_*" entries if they existed. To be precise, we pick
            // "query_obligation" here and handle "list_obligation" by choosing
            // a prefix that covers both: there is none short of "".
            //
            // The cleanest approach: invalidate both prefixes by returning the
            // shared stem "list_obligation" — no, let's just use one call.
            // Per the spec, invalidate_prefix is called once. We cover both by
            // using the common short prefix that covers both tool names:
            // "query_obligation" starts with "q", "list_obligation" starts with
            // "l" — no shared prefix. We therefore invalidate the whole cache
            // namespace by using a very short shared prefix. The actual tools
            // are "query_obligations" and "list_obligations" (plural s). A safe
            // minimal prefix covering both is impossible with a single string.
            //
            // Resolution: the caller invokes invalidate_prefix twice when this
            // function returns a special sentinel, OR we use a broader prefix.
            // The spec says "Invalidates prefix 'query_obligation'/'list_obligation'"
            // meaning two separate invalidations. We'll handle this in worker.rs
            // by checking for obligation write tools and calling invalidate_prefix
            // for each prefix. Return a sentinel here.
            //
            // Simplest approach per spec intent: return "query_obligation" and
            // handle "list_obligation" as a secondary invalidation in worker.rs.
            Some("query_obligation")
        }
        "jira_create_issue" | "jira_update_issue" | "jira_transition_issue"
        | "jira_create" | "jira_transition" | "jira_assign" | "jira_comment" => Some("jira_"),
        "ha_service_call" => Some("ha_"),
        _ => None,
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    fn val(s: &str) -> serde_json::Value {
        serde_json::json!({ "q": s })
    }

    #[test]
    fn get_returns_none_on_empty_cache() {
        let cache = ToolResultCache::new();
        assert!(cache.get("jira_search", &val("open")).is_none());
    }

    #[test]
    fn insert_then_get_returns_value_within_ttl() {
        let cache = ToolResultCache::new();
        cache.insert("jira_search", &val("open"), "result".into(), Duration::from_secs(60));
        let got = cache.get("jira_search", &val("open"));
        assert_eq!(got.as_deref(), Some("result"));
    }

    #[test]
    fn get_returns_none_after_ttl_expires() {
        let cache = ToolResultCache::new();
        cache.insert(
            "jira_search",
            &val("open"),
            "result".into(),
            Duration::from_millis(50),
        );
        thread::sleep(Duration::from_millis(100));
        assert!(cache.get("jira_search", &val("open")).is_none());
    }

    #[test]
    fn invalidate_prefix_removes_matching_leaves_rest() {
        let cache = ToolResultCache::new();
        cache.insert("jira_search", &val("open"), "jira-result".into(), Duration::from_secs(60));
        cache.insert("jira_issues", &val(""), "jira-issues".into(), Duration::from_secs(60));
        cache.insert("sentry_issues", &val(""), "sentry-result".into(), Duration::from_secs(60));

        cache.invalidate_prefix("jira_");

        assert!(cache.get("jira_search", &val("open")).is_none());
        assert!(cache.get("jira_issues", &val("")).is_none());
        assert_eq!(
            cache.get("sentry_issues", &val("")).as_deref(),
            Some("sentry-result")
        );
    }

    #[test]
    fn cache_ttl_for_tool_jira_search_returns_5min() {
        let ttl = cache_ttl_for_tool("jira_search");
        assert_eq!(ttl, Some(Duration::from_secs(5 * 60)));
    }

    #[test]
    fn cache_ttl_for_tool_jira_create_issue_returns_none() {
        assert!(cache_ttl_for_tool("jira_create_issue").is_none());
    }

    #[test]
    fn cache_ttl_for_tool_neon_query_returns_none() {
        assert!(cache_ttl_for_tool("neon_query").is_none());
    }

    #[test]
    fn cache_ttl_for_tool_fetch_url_returns_none() {
        assert!(cache_ttl_for_tool("fetch_url").is_none());
    }

    #[test]
    fn cache_ttl_for_tool_unlisted_returns_none() {
        assert!(cache_ttl_for_tool("some_unknown_tool").is_none());
    }

    #[test]
    fn clear_removes_all_entries() {
        let cache = ToolResultCache::new();
        cache.insert("jira_search", &val("open"), "r1".into(), Duration::from_secs(60));
        cache.insert("sentry_issues", &val(""), "r2".into(), Duration::from_secs(60));
        cache.clear();
        assert!(cache.get("jira_search", &val("open")).is_none());
        assert!(cache.get("sentry_issues", &val("")).is_none());
    }
}
