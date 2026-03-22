use std::collections::HashMap;
use std::process::Command;

use chrono::{DateTime, Utc};
use serde::Deserialize;

// ── Health Response (mirrors daemon's HealthResponse) ───────────────

#[derive(Debug, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub uptime_secs: u64,
    pub version: String,
    pub channels: HashMap<String, String>,
    pub last_digest_at: Option<DateTime<Utc>>,
    pub triggers_processed: u64,
}

// ── Status Command ──────────────────────────────────────────────────

/// Run the `nv status` command.
///
/// Queries the daemon's /health endpoint and systemd unit status,
/// then prints a combined summary. Exits non-zero if the daemon
/// is not running.
pub async fn run(health_port: u16) {
    let url = format!("http://127.0.0.1:{health_port}/health");
    let systemd_status = get_systemd_status();

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .expect("failed to create HTTP client");

    match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let body = match resp.text().await {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("Failed to read response: {e}");
                    show_fallback(&systemd_status);
                    std::process::exit(1);
                }
            };

            match serde_json::from_str::<HealthResponse>(&body) {
                Ok(health) => show_full(&health, &systemd_status),
                Err(e) => {
                    eprintln!("Failed to parse health response: {e}");
                    show_fallback(&systemd_status);
                    std::process::exit(1);
                }
            }
        }
        Ok(resp) => {
            eprintln!("Daemon returned HTTP {}", resp.status());
            show_fallback(&systemd_status);
            std::process::exit(1);
        }
        Err(e) => {
            if e.is_connect() {
                show_fallback(&systemd_status);
            } else {
                eprintln!("Failed to connect to daemon: {e}");
                show_fallback(&systemd_status);
            }
            std::process::exit(1);
        }
    }
}

// ── Display Helpers ─────────────────────────────────────────────────

fn show_full(health: &HealthResponse, systemd_status: &str) {
    let uptime = format_uptime(health.uptime_secs);
    println!("NV Daemon: running (uptime: {uptime})");
    println!("Version:   {}", health.version);
    println!("Health:    {}", health.status);
    println!("systemd:   {systemd_status}");

    if !health.channels.is_empty() {
        println!("Channels:");
        let mut sorted: Vec<_> = health.channels.iter().collect();
        sorted.sort_by_key(|(k, _)| k.as_str());
        for (name, status) in sorted {
            println!("  {name:<20} {status}");
        }
    }

    match health.last_digest_at {
        Some(ts) => {
            let ago = format_relative_time(ts);
            println!("Last Digest: {ago}");
        }
        None => println!("Last Digest: never"),
    }

    println!("Triggers:    {} processed", health.triggers_processed);
}

fn show_fallback(systemd_status: &str) {
    if systemd_status == "not-found" {
        println!("NV Daemon: not installed");
    } else {
        println!("NV Daemon: stopped (systemd: {systemd_status})");
    }
}

fn format_uptime(secs: u64) -> String {
    let hours = secs / 3600;
    let mins = (secs % 3600) / 60;
    if hours > 0 {
        format!("{hours}h {mins}m")
    } else {
        format!("{mins}m")
    }
}

fn format_relative_time(ts: DateTime<Utc>) -> String {
    let now = Utc::now();
    let diff = now.signed_duration_since(ts);
    let secs = diff.num_seconds();

    if secs < 0 {
        return "just now".into();
    }
    if secs < 60 {
        return format!("{secs} seconds ago");
    }
    let mins = secs / 60;
    if mins < 60 {
        return format!("{mins} minutes ago");
    }
    let hours = mins / 60;
    if hours < 24 {
        let remaining_mins = mins % 60;
        return format!("{hours}h {remaining_mins}m ago");
    }
    let days = hours / 24;
    format!("{days} days ago")
}

fn get_systemd_status() -> String {
    let output = Command::new("systemctl")
        .args(["--user", "is-active", "nv.service"])
        .output();

    match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        Err(_) => "unknown".into(),
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_uptime_minutes_only() {
        assert_eq!(format_uptime(0), "0m");
        assert_eq!(format_uptime(59), "0m");
        assert_eq!(format_uptime(60), "1m");
        assert_eq!(format_uptime(3599), "59m");
    }

    #[test]
    fn format_uptime_hours_and_minutes() {
        assert_eq!(format_uptime(3600), "1h 0m");
        assert_eq!(format_uptime(8040), "2h 14m");
    }

    #[test]
    fn format_relative_time_seconds() {
        let ts = Utc::now() - chrono::Duration::seconds(30);
        let result = format_relative_time(ts);
        assert!(result.contains("seconds ago"), "got: {result}");
    }

    #[test]
    fn format_relative_time_minutes() {
        let ts = Utc::now() - chrono::Duration::minutes(14);
        let result = format_relative_time(ts);
        assert!(result.contains("14 minutes ago"), "got: {result}");
    }

    #[test]
    fn format_relative_time_hours() {
        let ts = Utc::now() - chrono::Duration::hours(3);
        let result = format_relative_time(ts);
        assert!(result.contains("3h"), "got: {result}");
    }

    #[test]
    fn deserialize_health_response() {
        let json = r#"{
            "status": "ok",
            "uptime_secs": 3600,
            "version": "0.1.0",
            "channels": {"telegram": "connected", "nexus_homelab": "disconnected"},
            "last_digest_at": "2026-03-21T10:00:00Z",
            "triggers_processed": 142
        }"#;
        let resp: HealthResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.status, "ok");
        assert_eq!(resp.uptime_secs, 3600);
        assert_eq!(resp.channels.len(), 2);
        assert_eq!(resp.triggers_processed, 142);
    }

    #[test]
    fn deserialize_health_response_no_digest() {
        let json = r#"{
            "status": "ok",
            "uptime_secs": 0,
            "version": "0.1.0",
            "channels": {},
            "last_digest_at": null,
            "triggers_processed": 0
        }"#;
        let resp: HealthResponse = serde_json::from_str(json).unwrap();
        assert!(resp.last_digest_at.is_none());
    }
}
