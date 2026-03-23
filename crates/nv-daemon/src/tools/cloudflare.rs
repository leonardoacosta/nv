//! Cloudflare DNS tools — read-only visibility into zones and DNS records.
//!
//! Three tools:
//! * `cf_zones` — list all Cloudflare zones with status, plan, nameservers (1h cache).
//! * `cf_dns_records` — list DNS records for a domain, with optional type filter.
//! * `cf_domain_status` — quick health check (nameservers, SSL mode, security level).
//!
//! Authentication: `CLOUDFLARE_API_TOKEN` environment variable.
//! Required token permissions: Zone:Read, DNS:Read.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Result};
use serde::Deserialize;

use crate::claude::ToolDefinition;

// ── Constants ────────────────────────────────────────────────────────

const CF_API: &str = "https://api.cloudflare.com/client/v4";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const ZONE_CACHE_TTL: Duration = Duration::from_secs(3600); // 1 hour

// ── Cloudflare API response types ────────────────────────────────────

/// Cloudflare API envelope.
#[derive(Deserialize)]
struct CfResponse<T> {
    result: Option<T>,
    success: bool,
    #[serde(default)]
    errors: Vec<CfError>,
}

#[derive(Deserialize)]
struct CfError {
    message: String,
}

/// A Cloudflare zone (domain).
#[derive(Deserialize, Clone, Debug)]
pub struct Zone {
    pub id: String,
    pub name: String,
    pub status: String,
    pub plan: ZonePlan,
    #[serde(default)]
    pub name_servers: Vec<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ZonePlan {
    pub name: String,
}

/// A DNS record within a zone.
#[derive(Deserialize, Clone, Debug)]
pub struct DnsRecord {
    #[serde(rename = "type")]
    pub record_type: String,
    pub name: String,
    pub content: String,
    #[serde(default)]
    pub proxied: bool,
    /// TTL in seconds. Cloudflare uses `1` to mean "auto".
    pub ttl: u32,
}

/// A Cloudflare settings value response.
#[derive(Deserialize)]
struct SettingResponse {
    value: serde_json::Value,
}

// ── Zone Cache ───────────────────────────────────────────────────────

/// Cached zone list with TTL.
static ZONE_CACHE: Mutex<Option<(Instant, Vec<Zone>)>> = Mutex::new(None);

/// Fetch zones from cache or API.
///
/// The cache is keyed globally (all users share the same Cloudflare account).
/// Returns cached data if less than 1 hour old.
async fn get_cached_zones(client: &CloudflareClient) -> Result<Vec<Zone>> {
    // Check cache first (acquire lock, read, drop immediately)
    {
        let guard = ZONE_CACHE.lock().unwrap();
        if let Some((ts, ref zones)) = *guard {
            if ts.elapsed() < ZONE_CACHE_TTL {
                return Ok(zones.clone());
            }
        }
    }

    // Fetch fresh zones
    let zones = fetch_zones(client).await?;

    // Update cache
    {
        let mut guard = ZONE_CACHE.lock().unwrap();
        *guard = Some((Instant::now(), zones.clone()));
    }

    Ok(zones)
}

/// Fetch all zones from the Cloudflare API (paginated, up to 500).
async fn fetch_zones(client: &CloudflareClient) -> Result<Vec<Zone>> {
    let mut all_zones = Vec::new();
    let mut page = 1u32;

    loop {
        let url = format!(
            "{CF_API}/zones?per_page=50&page={page}"
        );
        let resp = client.get(&url).send().await
            .map_err(|e| anyhow!("Cloudflare zones request failed: {e}"))?;

        if !resp.status().is_success() {
            return Err(CloudflareClient::map_status(resp.status(), "zones list"));
        }

        let body: CfResponse<Vec<Zone>> = resp.json().await
            .map_err(|e| anyhow!("Failed to parse zones response: {e}"))?;

        if !body.success {
            let msgs: Vec<&str> = body.errors.iter().map(|e| e.message.as_str()).collect();
            bail!("Cloudflare API error: {}", msgs.join(", "));
        }

        let zones = body.result.unwrap_or_default();
        let fetched = zones.len();
        all_zones.extend(zones);

        if fetched < 50 {
            break; // Last page
        }
        page += 1;
        if page > 10 {
            break; // Safety cap at 500 zones
        }
    }

    Ok(all_zones)
}

/// Resolve a domain name to its zone ID, using the zone cache.
///
/// Returns an error listing available zones if the domain is not found.
async fn resolve_zone_id(client: &CloudflareClient, domain: &str) -> Result<String> {
    let zones = get_cached_zones(client).await?;

    if let Some(zone) = zones.iter().find(|z| z.name == domain) {
        return Ok(zone.id.clone());
    }

    let available: Vec<&str> = zones.iter().map(|z| z.name.as_str()).collect();
    Err(anyhow!(
        "Domain '{}' not found in Cloudflare zone list.\nAvailable zones: {}",
        domain,
        if available.is_empty() {
            "(none — check CLOUDFLARE_API_TOKEN permissions)".to_string()
        } else {
            available.join(", ")
        }
    ))
}

// ── CloudflareClient ─────────────────────────────────────────────────

/// HTTP client for the Cloudflare API v4.
#[derive(Debug)]
pub struct CloudflareClient {
    http: reqwest::Client,
    token: String,
}

impl CloudflareClient {
    /// Create a client from `CLOUDFLARE_API_TOKEN` environment variable.
    pub fn from_env() -> Result<Self> {
        let token = std::env::var("CLOUDFLARE_API_TOKEN")
            .map_err(|_| anyhow!("Cloudflare not configured — CLOUDFLARE_API_TOKEN not set"))?;
        if token.is_empty() {
            bail!("CLOUDFLARE_API_TOKEN env var is empty");
        }
        Ok(Self {
            http: reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()?,
            token,
        })
    }

    /// Build a GET request with Bearer authorization.
    fn get(&self, url: &str) -> reqwest::RequestBuilder {
        self.http
            .get(url)
            .header("Authorization", format!("Bearer {}", self.token))
    }

    /// Map HTTP error codes to actionable messages.
    fn map_status(status: reqwest::StatusCode, context: &str) -> anyhow::Error {
        match status.as_u16() {
            401 => anyhow!("Cloudflare token invalid or expired (401) — check CLOUDFLARE_API_TOKEN"),
            403 => anyhow!("Token lacks Zone:Read or DNS:Read permission (403)"),
            404 => anyhow!("{context} not found (404)"),
            429 => anyhow!("Cloudflare rate limit hit (429) — wait a moment"),
            code => anyhow!("Cloudflare API error ({code}) for {context}"),
        }
    }
}

// ── Tool: cf_zones ───────────────────────────────────────────────────

/// List all Cloudflare zones (domains) with status, plan, and nameservers.
///
/// Results are cached for 1 hour.
pub async fn cf_zones(client: &CloudflareClient) -> Result<String> {
    let zones = get_cached_zones(client).await?;

    if zones.is_empty() {
        return Ok("No Cloudflare zones found. Check that CLOUDFLARE_API_TOKEN has Zone:Read permission.".to_string());
    }

    let mut lines = vec![format!("Cloudflare zones ({}):", zones.len())];
    for zone in &zones {
        let ns = if zone.name_servers.is_empty() {
            "(none)".to_string()
        } else {
            zone.name_servers.join(", ")
        };
        lines.push(format!("\u{1f310} **{}** \u{2014} {}", zone.name, zone.status));
        lines.push(format!("   Plan: {} | NS: {}", zone.plan.name, ns));
    }

    Ok(lines.join("\n"))
}

// ── Tool: cf_dns_records ─────────────────────────────────────────────

/// List DNS records for a domain, with optional record type filter.
pub async fn cf_dns_records(
    client: &CloudflareClient,
    domain: &str,
    record_type: Option<&str>,
) -> Result<String> {
    if domain.is_empty() {
        bail!("domain cannot be empty");
    }

    let zone_id = resolve_zone_id(client, domain).await?;

    let mut url = format!("{CF_API}/zones/{zone_id}/dns_records?per_page=100");
    if let Some(rt) = record_type {
        if !rt.is_empty() {
            url.push_str(&format!("&type={rt}"));
        }
    }

    let resp = client.get(&url).send().await
        .map_err(|e| anyhow!("Cloudflare DNS records request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(CloudflareClient::map_status(resp.status(), &format!("DNS records for '{domain}'")));
    }

    let body: CfResponse<Vec<DnsRecord>> = resp.json().await
        .map_err(|e| anyhow!("Failed to parse DNS records: {e}"))?;

    if !body.success {
        let msgs: Vec<&str> = body.errors.iter().map(|e| e.message.as_str()).collect();
        bail!("Cloudflare API error: {}", msgs.join(", "));
    }

    let records = body.result.unwrap_or_default();
    if records.is_empty() {
        let type_suffix = record_type.map(|t| format!(" (type: {t})")).unwrap_or_default();
        return Ok(format!("No DNS records found for {domain}{type_suffix}."));
    }

    let mut lines = vec![format!("DNS records for {domain} ({}):", records.len())];
    for r in &records {
        let proxied = if r.proxied { "proxied" } else { "DNS only" };
        let ttl = if r.ttl == 1 { "auto".to_string() } else { r.ttl.to_string() };
        let content_display = if r.content.len() > 50 {
            format!("{}...", &r.content[..47])
        } else {
            r.content.clone()
        };
        lines.push(format!(
            "\u{1f310} {} **{}** \u{2192} {}",
            r.record_type, r.name, content_display
        ));
        lines.push(format!("   Proxied: {proxied} | TTL: {ttl}"));
    }

    Ok(lines.join("\n"))
}

// ── Tool: cf_domain_status ───────────────────────────────────────────

/// Quick health check for a domain: status, plan, nameservers, SSL mode, security level.
pub async fn cf_domain_status(client: &CloudflareClient, domain: &str) -> Result<String> {
    if domain.is_empty() {
        bail!("domain cannot be empty");
    }

    let zone_id = resolve_zone_id(client, domain).await?;

    // Fetch zone info from cache (already resolved above)
    let zones = get_cached_zones(client).await?;
    let zone = zones
        .iter()
        .find(|z| z.id == zone_id)
        .ok_or_else(|| anyhow!("Zone '{domain}' not found after resolution (cache inconsistency)"))?;

    // Fetch SSL and security_level settings in parallel
    let ssl_url = format!("{CF_API}/zones/{zone_id}/settings/ssl");
    let sec_url = format!("{CF_API}/zones/{zone_id}/settings/security_level");

    let (ssl_resp, sec_resp) = tokio::join!(
        client.get(&ssl_url).send(),
        client.get(&sec_url).send(),
    );

    let ssl_mode = match ssl_resp {
        Ok(r) if r.status().is_success() => {
            r.json::<CfResponse<SettingResponse>>()
                .await
                .ok()
                .and_then(|b| b.result)
                .map(|s| s.value.as_str().unwrap_or("").to_string())
                .unwrap_or_else(|| "unknown".to_string())
        }
        _ => "unknown".to_string(),
    };

    let security_level = match sec_resp {
        Ok(r) if r.status().is_success() => {
            r.json::<CfResponse<SettingResponse>>()
                .await
                .ok()
                .and_then(|b| b.result)
                .map(|s| s.value.as_str().unwrap_or("").to_string())
                .unwrap_or_else(|| "unknown".to_string())
        }
        _ => "unknown".to_string(),
    };

    let nameservers = if zone.name_servers.is_empty() {
        "(none)".to_string()
    } else {
        zone.name_servers.join(", ")
    };

    let output = format!(
        "\u{1f310} **{}** \u{2014} {}\n\
         **Plan:** {}\n\
         **Nameservers:** {}\n\
         **SSL Mode:** {}\n\
         **Security Level:** {}",
        zone.name,
        zone.status,
        zone.plan.name,
        nameservers,
        ssl_mode,
        security_level,
    );

    Ok(output)
}

// ── Tool Definitions ─────────────────────────────────────────────────

/// Return tool definitions for all Cloudflare tools.
pub fn cloudflare_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "cf_zones".into(),
            description: "List all Cloudflare zones (domains) with their status (active/pending/moved), plan tier, and nameservers. Results are cached for 1 hour. Requires CLOUDFLARE_API_TOKEN with Zone:Read permission.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "cf_dns_records".into(),
            description: "List DNS records for a Cloudflare-managed domain. Returns record type, name, content, proxied status, and TTL. Use record_type to filter (e.g. 'A', 'CNAME', 'MX', 'TXT'). TTL of 'auto' means managed by Cloudflare.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "domain": {
                        "type": "string",
                        "description": "The domain name to look up (e.g. 'example.com')"
                    },
                    "record_type": {
                        "type": "string",
                        "description": "Optional DNS record type filter (e.g. 'A', 'CNAME', 'MX', 'TXT', 'NS')"
                    }
                },
                "required": ["domain"]
            }),
        },
        ToolDefinition {
            name: "cf_domain_status".into(),
            description: "Quick health check for a Cloudflare-managed domain. Returns zone status, plan, nameservers, SSL mode (off/flexible/full/strict), and security level. Use this to verify a domain's configuration at a glance.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "domain": {
                        "type": "string",
                        "description": "The domain name to check (e.g. 'example.com')"
                    }
                },
                "required": ["domain"]
            }),
        },
    ]
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloudflare_tool_definitions_returns_three_tools() {
        let tools = cloudflare_tool_definitions();
        assert_eq!(tools.len(), 3);
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"cf_zones"));
        assert!(names.contains(&"cf_dns_records"));
        assert!(names.contains(&"cf_domain_status"));
    }

    #[test]
    fn tool_definitions_have_schemas() {
        for tool in cloudflare_tool_definitions() {
            assert!(!tool.name.is_empty());
            assert!(!tool.description.is_empty());
            assert_eq!(tool.input_schema["type"], "object");
        }
    }

    #[test]
    fn cf_dns_records_requires_domain() {
        let tools = cloudflare_tool_definitions();
        let t = tools.iter().find(|t| t.name == "cf_dns_records").unwrap();
        let required = t.input_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v.as_str() == Some("domain")));
    }

    #[test]
    fn cf_zones_has_no_required_params() {
        let tools = cloudflare_tool_definitions();
        let t = tools.iter().find(|t| t.name == "cf_zones").unwrap();
        let required = t.input_schema["required"].as_array().unwrap();
        assert!(required.is_empty());
    }

    #[test]
    fn dns_record_ttl_display() {
        // TTL 1 = "auto", anything else = seconds
        let auto_ttl = 1u32;
        let display = if auto_ttl == 1 { "auto".to_string() } else { auto_ttl.to_string() };
        assert_eq!(display, "auto");

        let real_ttl = 3600u32;
        let display = if real_ttl == 1 { "auto".to_string() } else { real_ttl.to_string() };
        assert_eq!(display, "3600");
    }

    #[test]
    fn client_from_env_fails_without_token() {
        let saved = std::env::var("CLOUDFLARE_API_TOKEN").ok();
        unsafe { std::env::remove_var("CLOUDFLARE_API_TOKEN"); }
        let result = CloudflareClient::from_env();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("CLOUDFLARE_API_TOKEN"));
        if let Some(v) = saved {
            unsafe { std::env::set_var("CLOUDFLARE_API_TOKEN", v); }
        }
    }

    #[test]
    fn parse_zone_from_json() {
        let json = r#"{
            "id": "zone-abc",
            "name": "example.com",
            "status": "active",
            "plan": {"name": "Pro"},
            "name_servers": ["aria.ns.cloudflare.com", "bob.ns.cloudflare.com"]
        }"#;
        let zone: Zone = serde_json::from_str(json).unwrap();
        assert_eq!(zone.id, "zone-abc");
        assert_eq!(zone.name, "example.com");
        assert_eq!(zone.status, "active");
        assert_eq!(zone.plan.name, "Pro");
        assert_eq!(zone.name_servers.len(), 2);
    }

    #[test]
    fn parse_dns_record_from_json() {
        let json = r#"{
            "type": "A",
            "name": "example.com",
            "content": "203.0.113.1",
            "proxied": true,
            "ttl": 1
        }"#;
        let record: DnsRecord = serde_json::from_str(json).unwrap();
        assert_eq!(record.record_type, "A");
        assert_eq!(record.content, "203.0.113.1");
        assert!(record.proxied);
        assert_eq!(record.ttl, 1);
    }
}

// ── Checkable ────────────────────────────────────────────────────────

#[async_trait::async_trait]
impl crate::tools::Checkable for CloudflareClient {
    fn name(&self) -> &str {
        "cloudflare"
    }

    async fn check_read(&self) -> crate::tools::CheckResult {
        use crate::tools::check::timed;
        let url = format!("{CF_API}/user/tokens/verify");
        let (latency, result) =
            timed(|| async { self.get(&url).send().await }).await;
        match result {
            Ok(resp) if resp.status().is_success() => crate::tools::CheckResult::Healthy {
                latency_ms: latency,
                detail: "token verified".into(),
            },
            Ok(resp) if resp.status().as_u16() == 401 => crate::tools::CheckResult::Unhealthy {
                error: "token invalid (401) — check CLOUDFLARE_API_TOKEN".into(),
            },
            Ok(resp) if resp.status().as_u16() == 403 => crate::tools::CheckResult::Unhealthy {
                error: "token lacks Zone:Read permission (403)".into(),
            },
            Ok(resp) => crate::tools::CheckResult::Unhealthy {
                error: format!("HTTP {}", resp.status()),
            },
            Err(e) => crate::tools::CheckResult::Unhealthy {
                error: e.to_string(),
            },
        }
    }
}
