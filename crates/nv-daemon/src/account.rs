//! Account info cache — queries `claude --version` for account metadata.
//!
//! Caches the result in `~/.nv/account-info.json` with a 6-hour refresh cycle.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Cached account metadata from the Claude CLI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountInfo {
    /// Plan name (e.g., "Pro", "Max", "Free").
    pub plan: String,
    /// Username or organization.
    pub username: String,
    /// Auth method (e.g., "OAuth", "API key").
    pub auth_method: String,
    /// When this cache entry was written (seconds since UNIX epoch).
    pub cached_at: u64,
}

impl Default for AccountInfo {
    fn default() -> Self {
        Self {
            plan: "unknown".into(),
            username: "unknown".into(),
            auth_method: "unknown".into(),
            cached_at: 0,
        }
    }
}

/// Resolve the cache file path: `~/.nv/account-info.json`.
fn cache_path() -> Result<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_default();
    Ok(Path::new(&home).join(".nv").join("account-info.json"))
}

/// Read cached account info, if it exists and is fresh (< 6 hours old).
pub fn load_cached() -> Option<AccountInfo> {
    let path = cache_path().ok()?;
    let contents = std::fs::read_to_string(&path).ok()?;
    let info: AccountInfo = serde_json::from_str(&contents).ok()?;

    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()?
        .as_secs();

    // 6 hours = 21600 seconds
    if now.saturating_sub(info.cached_at) < 21600 {
        Some(info)
    } else {
        None
    }
}

/// Write account info to the cache file.
fn save_cache(info: &AccountInfo) -> Result<()> {
    let path = cache_path()?;
    let json = serde_json::to_string_pretty(info)?;
    std::fs::write(&path, json)?;
    Ok(())
}

/// Query `claude --version` and parse account info from the output.
///
/// The CLI prints lines like:
/// ```text
/// claude v1.x.x
/// Authenticated as: username (Pro plan)
/// ```
///
/// Returns a fresh `AccountInfo` and caches it.
pub async fn query_account_info() -> Result<AccountInfo> {
    // Check cache first
    if let Some(cached) = load_cached() {
        return Ok(cached);
    }

    let output = tokio::process::Command::new("claude")
        .arg("--version")
        .output()
        .await?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}\n{stderr}");

    let mut plan = "unknown".to_string();
    let mut username = "unknown".to_string();
    let mut auth_method = "OAuth".to_string(); // default assumption

    for line in combined.lines() {
        let lower = line.to_lowercase();

        // Parse "Authenticated as: username (Pro plan)" or similar
        if lower.contains("authenticated") || lower.contains("logged in") {
            if let Some(rest) = line.split(':').nth(1) {
                let rest = rest.trim();
                // Try to extract username and plan from "username (Plan plan)"
                if let Some(paren_start) = rest.find('(') {
                    username = rest[..paren_start].trim().to_string();
                    if let Some(paren_end) = rest.find(')') {
                        plan = rest[paren_start + 1..paren_end].trim().to_string();
                    }
                } else {
                    username = rest.to_string();
                }
            }
        }

        // Detect auth method hints
        if lower.contains("api key") || lower.contains("api_key") {
            auth_method = "API key".to_string();
        } else if lower.contains("oauth") {
            auth_method = "OAuth".to_string();
        }
    }

    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_secs();

    let info = AccountInfo {
        plan,
        username,
        auth_method,
        cached_at: now,
    };

    // Best-effort cache write
    if let Err(e) = save_cache(&info) {
        tracing::warn!(error = %e, "failed to cache account info");
    }

    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_account_info() {
        let info = AccountInfo::default();
        assert_eq!(info.plan, "unknown");
        assert_eq!(info.username, "unknown");
        assert_eq!(info.auth_method, "unknown");
        assert_eq!(info.cached_at, 0);
    }

    #[test]
    fn account_info_serialization_roundtrip() {
        let info = AccountInfo {
            plan: "Pro".into(),
            username: "leo".into(),
            auth_method: "OAuth".into(),
            cached_at: 1700000000,
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: AccountInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.plan, "Pro");
        assert_eq!(parsed.username, "leo");
        assert_eq!(parsed.auth_method, "OAuth");
        assert_eq!(parsed.cached_at, 1700000000);
    }
}
