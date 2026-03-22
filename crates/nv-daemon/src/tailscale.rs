use std::collections::HashMap;

use anyhow::{anyhow, Result};
use serde::Deserialize;

/// Client that queries Tailscale network status via `docker exec tailscale tailscale status --json`.
pub struct TailscaleClient;

// ── JSON Types (tailscale status --json) ─────────────────────────────

/// Top-level response from `tailscale status --json`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TailscaleStatus {
    #[serde(rename = "Self")]
    pub self_node: TailscalePeer,
    pub peer: HashMap<String, TailscalePeer>,
}

/// A single node (self or peer) in the Tailscale network.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TailscalePeer {
    #[serde(default)]
    pub host_name: String,
    #[serde(rename = "DNSName", default)]
    pub dns_name: String,
    #[serde(rename = "OS", default)]
    pub os: String,
    #[serde(default)]
    pub online: bool,
    #[serde(rename = "TailscaleIPs", default)]
    pub tailscale_ips: Vec<String>,
    #[serde(default)]
    pub last_seen: String,
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub relay: String,
    #[serde(default)]
    pub cur_addr: String,
}

impl TailscaleClient {
    /// Execute `docker exec tailscale tailscale status --json` and return raw stdout.
    async fn run_status_cmd() -> Result<String> {
        let output = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            tokio::process::Command::new("docker")
                .args(["exec", "tailscale", "tailscale", "status", "--json"])
                .output(),
        )
        .await
        .map_err(|_| anyhow!("tailscale status timed out after 5s"))?
        .map_err(|e| anyhow!("failed to exec docker: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!(
                "tailscale status failed (exit {}): {}",
                output.status,
                stderr.trim()
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    /// Parse raw JSON into `TailscaleStatus`.
    fn parse(json: &str) -> Result<TailscaleStatus> {
        serde_json::from_str(json).map_err(|e| anyhow!("failed to parse tailscale JSON: {e}"))
    }

    /// Return a concise text table of all nodes (online first, then offline).
    pub async fn status() -> Result<String> {
        let raw = Self::run_status_cmd().await?;
        let ts = Self::parse(&raw)?;

        // Collect all nodes: self + peers
        let mut nodes: Vec<&TailscalePeer> = vec![&ts.self_node];
        nodes.extend(ts.peer.values());

        // Sort: online first, then alphabetical by hostname
        nodes.sort_by(|a, b| {
            b.online
                .cmp(&a.online)
                .then_with(|| a.host_name.to_lowercase().cmp(&b.host_name.to_lowercase()))
        });

        let mut lines = Vec::with_capacity(nodes.len() + 2);
        lines.push(format!(
            "{:<20} {:<8} {:<18} {:<10} {}",
            "Hostname", "Online", "IP", "OS", "Last Seen"
        ));
        lines.push("-".repeat(70));

        for node in &nodes {
            let ip = node
                .tailscale_ips
                .iter()
                .find(|ip| ip.contains('.'))
                .cloned()
                .unwrap_or_else(|| "-".into());

            let last_seen = if node.online {
                "-".to_string()
            } else if node.last_seen.is_empty() {
                "unknown".to_string()
            } else {
                // Truncate to date+time portion
                node.last_seen.chars().take(19).collect()
            };

            lines.push(format!(
                "{:<20} {:<8} {:<18} {:<10} {}",
                truncate(&node.host_name, 20),
                if node.online { "yes" } else { "no" },
                ip,
                truncate(&node.os, 10),
                last_seen,
            ));
        }

        lines.push(format!(
            "\n{} nodes total ({} online)",
            nodes.len(),
            nodes.iter().filter(|n| n.online).count()
        ));

        Ok(lines.join("\n"))
    }

    /// Return detailed info for a specific node, matched case-insensitively by hostname.
    pub async fn node(name: &str) -> Result<String> {
        let raw = Self::run_status_cmd().await?;
        let ts = Self::parse(&raw)?;

        let lower = name.to_lowercase();

        // Search self node + all peers
        let found = if ts.self_node.host_name.to_lowercase() == lower {
            Some(&ts.self_node)
        } else {
            ts.peer.values().find(|p| p.host_name.to_lowercase() == lower)
        };

        let node = found.ok_or_else(|| {
            let known: Vec<&str> = std::iter::once(ts.self_node.host_name.as_str())
                .chain(ts.peer.values().map(|p| p.host_name.as_str()))
                .collect();
            anyhow!("node '{name}' not found. Known nodes: {}", known.join(", "))
        })?;

        let connection = if !node.online {
            "offline"
        } else if node.cur_addr.is_empty() || node.cur_addr.contains("derp") {
            "relayed"
        } else {
            "direct"
        };

        let mut lines = Vec::new();
        lines.push(format!("Hostname:   {}", node.host_name));
        lines.push(format!("DNSName:    {}", node.dns_name));
        lines.push(format!("Online:     {}", node.online));
        lines.push(format!("Active:     {}", node.active));
        lines.push(format!(
            "IPs:        {}",
            if node.tailscale_ips.is_empty() {
                "-".into()
            } else {
                node.tailscale_ips.join(", ")
            }
        ));
        lines.push(format!("OS:         {}", node.os));
        lines.push(format!(
            "Relay:      {}",
            if node.relay.is_empty() {
                "-"
            } else {
                &node.relay
            }
        ));
        lines.push(format!(
            "Last Seen:  {}",
            if node.last_seen.is_empty() {
                "-"
            } else {
                &node.last_seen
            }
        ));
        lines.push(format!("Connection: {connection}"));

        Ok(lines.join("\n"))
    }

    /// Check if the Tailscale container is reachable.
    pub async fn is_available() -> bool {
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            tokio::process::Command::new("docker")
                .args(["inspect", "tailscale"])
                .output(),
        )
        .await;

        matches!(result, Ok(Ok(output)) if output.status.success())
    }
}

/// Truncate a string to `max` chars, appending "..." if truncated.
fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max.saturating_sub(3)])
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Sample JSON output from `tailscale status --json`.
    const SAMPLE_STATUS_JSON: &str = r#"{
        "Self": {
            "HostName": "homelab",
            "DNSName": "homelab.tail12345.ts.net.",
            "OS": "linux",
            "Online": true,
            "TailscaleIPs": ["100.64.0.1", "fd7a:115c:a1e0::1"],
            "LastSeen": "2026-03-22T10:00:00Z",
            "Active": true,
            "Relay": "",
            "CurAddr": "192.168.1.10:41641"
        },
        "Peer": {
            "nodekey:abc123": {
                "HostName": "macbook",
                "DNSName": "macbook.tail12345.ts.net.",
                "OS": "macOS",
                "Online": true,
                "TailscaleIPs": ["100.64.0.2", "fd7a:115c:a1e0::2"],
                "LastSeen": "2026-03-22T09:50:00Z",
                "Active": true,
                "Relay": "sfo",
                "CurAddr": "192.168.1.20:41641"
            },
            "nodekey:def456": {
                "HostName": "nas",
                "DNSName": "nas.tail12345.ts.net.",
                "OS": "linux",
                "Online": false,
                "TailscaleIPs": ["100.64.0.3"],
                "LastSeen": "2026-03-21T18:30:00Z",
                "Active": false,
                "Relay": "sfo",
                "CurAddr": ""
            },
            "nodekey:ghi789": {
                "HostName": "phone",
                "DNSName": "phone.tail12345.ts.net.",
                "OS": "android",
                "Online": true,
                "TailscaleIPs": ["100.64.0.4"],
                "LastSeen": "2026-03-22T09:55:00Z",
                "Active": false,
                "Relay": "lax",
                "CurAddr": ""
            }
        }
    }"#;

    #[test]
    fn parse_status_json() {
        let status = TailscaleClient::parse(SAMPLE_STATUS_JSON).unwrap();

        assert_eq!(status.self_node.host_name, "homelab");
        assert!(status.self_node.online);
        assert_eq!(status.self_node.os, "linux");
        assert_eq!(status.peer.len(), 3);

        let macbook = status.peer.values().find(|p| p.host_name == "macbook").unwrap();
        assert!(macbook.online);
        assert_eq!(macbook.os, "macOS");
        assert_eq!(macbook.tailscale_ips.len(), 2);
        assert_eq!(macbook.tailscale_ips[0], "100.64.0.2");
    }

    #[test]
    fn status_sorts_online_first() {
        let status = TailscaleClient::parse(SAMPLE_STATUS_JSON).unwrap();

        let mut nodes: Vec<&TailscalePeer> = vec![&status.self_node];
        nodes.extend(status.peer.values());
        nodes.sort_by(|a, b| {
            b.online
                .cmp(&a.online)
                .then_with(|| a.host_name.to_lowercase().cmp(&b.host_name.to_lowercase()))
        });

        // Online nodes should come first
        let online_count = nodes.iter().take_while(|n| n.online).count();
        assert_eq!(online_count, 3); // homelab, macbook, phone

        // Offline node should be last
        assert!(!nodes.last().unwrap().online);
        assert_eq!(nodes.last().unwrap().host_name, "nas");
    }

    #[test]
    fn node_case_insensitive_match() {
        let status = TailscaleClient::parse(SAMPLE_STATUS_JSON).unwrap();
        let lower = "macbook".to_lowercase();

        let found = status
            .peer
            .values()
            .find(|p| p.host_name.to_lowercase() == lower);

        assert!(found.is_some());
        assert_eq!(found.unwrap().host_name, "macbook");
    }

    #[test]
    fn node_not_found_lists_known_nodes() {
        let status = TailscaleClient::parse(SAMPLE_STATUS_JSON).unwrap();
        let lower = "nonexistent".to_lowercase();

        let found_self = status.self_node.host_name.to_lowercase() == lower;
        let found_peer = status
            .peer
            .values()
            .find(|p| p.host_name.to_lowercase() == lower);

        assert!(!found_self);
        assert!(found_peer.is_none());

        // Verify we can build the known nodes list
        let known: Vec<&str> = std::iter::once(status.self_node.host_name.as_str())
            .chain(status.peer.values().map(|p| p.host_name.as_str()))
            .collect();
        assert!(known.contains(&"homelab"));
        assert!(known.contains(&"macbook"));
        assert!(known.contains(&"nas"));
    }

    #[test]
    fn parse_ignores_unknown_fields() {
        // JSON with an extra unknown field should parse fine
        let json = r#"{
            "Self": {
                "HostName": "test",
                "UnknownField": "value",
                "Online": true,
                "TailscaleIPs": []
            },
            "Peer": {},
            "ExtraTopLevel": 42
        }"#;
        let result = TailscaleClient::parse(json);
        assert!(result.is_ok());
    }

    #[test]
    fn truncate_short_string() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn truncate_long_string() {
        let result = truncate("averylonghostname", 10);
        assert_eq!(result.len(), 10);
        assert!(result.ends_with("..."));
    }
}
