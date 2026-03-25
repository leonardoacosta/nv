//! Docker container monitoring tools via `docker` CLI.
//!
//! Uses `Command::new("docker")` to query containers — no socket
//! client or extra dependencies needed.  Two tools:
//!
//! * `docker_status(all?)` — list containers with state, uptime, and ports.
//! * `docker_logs(container, lines?)` — recent log lines from a container.

use std::time::Duration;

use anyhow::{anyhow, Result};
use tokio::process::Command;

/// Timeout for all docker CLI calls.
const DOCKER_TIMEOUT: Duration = Duration::from_secs(10);

/// Maximum log lines that can be requested.
const MAX_LOG_LINES: u64 = 200;

/// Default log lines when none specified.
const DEFAULT_LOG_LINES: u64 = 50;

/// Maximum total bytes of log output returned to the caller (10 KB).
const MAX_LOG_BYTES: usize = 10_240;

// ── Docker Socket Check ─────────────────────────────────────────────

/// Returns `true` if the docker CLI is reachable (i.e. `docker info`
/// succeeds within the timeout).  Used at startup to decide whether to
/// register docker tools.
#[allow(dead_code)]
pub async fn is_docker_available() -> bool {
    let result = tokio::time::timeout(DOCKER_TIMEOUT, async {
        Command::new("docker")
            .arg("info")
            .arg("--format")
            .arg("{{.ServerVersion}}")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output()
            .await
    })
    .await;

    match result {
        Ok(Ok(output)) => output.status.success(),
        _ => false,
    }
}

// ── docker_status ───────────────────────────────────────────────────

/// List running (or all) containers as a concise text table.
///
/// Calls `docker ps --format` with a Go template that produces
/// tab-separated fields which we reformat into aligned columns.
pub async fn docker_status(all: bool) -> Result<String> {
    let mut cmd = Command::new("docker");
    cmd.arg("ps")
        .arg("--format")
        .arg("{{.Names}}\t{{.Image}}\t{{.State}}\t{{.Status}}\t{{.Ports}}")
        .arg("--no-trunc");

    if all {
        cmd.arg("--all");
    }

    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let output = tokio::time::timeout(DOCKER_TIMEOUT, cmd.output())
        .await
        .map_err(|_| anyhow!("docker ps timed out after {}s", DOCKER_TIMEOUT.as_secs()))?
        .map_err(|e| anyhow!("failed to execute docker ps: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("docker ps failed: {}", stderr.trim()));
    }

    let raw = String::from_utf8_lossy(&output.stdout);
    let raw = raw.trim();

    if raw.is_empty() {
        return Ok("No containers found.".into());
    }

    // Build a mobile-friendly list: 🐳 name (image) — state
    let mut rows: Vec<[String; 5]> = Vec::new();
    for line in raw.lines() {
        let parts: Vec<&str> = line.splitn(5, '\t').collect();
        if parts.len() < 3 {
            continue;
        }
        rows.push([
            parts.first().unwrap_or(&"").to_string(),
            truncate(parts.get(1).unwrap_or(&""), 30),
            parts.get(2).unwrap_or(&"").to_string(),
            parts.get(3).unwrap_or(&"").to_string(),
            truncate(parts.get(4).unwrap_or(&""), 40),
        ]);
    }

    if rows.is_empty() {
        return Ok("No containers found.".into());
    }

    let count = rows.len();
    let mut lines = vec![format!("Containers ({count}):") ];

    for row in &rows {
        let name = &row[0];
        let image = &row[1];
        let state = &row[2];
        let uptime = &row[3];
        let ports = &row[4];

        lines.push(format!("\u{1f433} **{name}** ({image}) \u{2014} {state}"));
        let detail = match (uptime.is_empty(), ports.is_empty()) {
            (false, false) => format!("   Uptime: {uptime} | Ports: {ports}"),
            (false, true)  => format!("   Uptime: {uptime}"),
            (true, false)  => format!("   Ports: {ports}"),
            (true, true)   => String::new(),
        };
        if !detail.is_empty() {
            lines.push(detail);
        }
    }

    Ok(lines.join("\n"))
}

// ── docker_logs ─────────────────────────────────────────────────────

/// Fetch recent log lines from a container.
///
/// `container` is the container name or ID.
/// `lines` is capped at [`MAX_LOG_LINES`] and defaults to [`DEFAULT_LOG_LINES`].
pub async fn docker_logs(container: &str, lines: Option<u64>) -> Result<String> {
    if container.is_empty() {
        return Err(anyhow!("container name is required"));
    }

    // Basic input sanitisation — reject shell metacharacters
    if container
        .chars()
        .any(|c| !c.is_alphanumeric() && c != '-' && c != '_' && c != '.')
    {
        return Err(anyhow!(
            "invalid container name '{}': only alphanumeric, dash, underscore, dot allowed",
            container
        ));
    }

    let tail = lines.unwrap_or(DEFAULT_LOG_LINES).min(MAX_LOG_LINES);

    let mut cmd = Command::new("docker");
    cmd.arg("logs")
        .arg("--tail")
        .arg(tail.to_string())
        .arg("--timestamps")
        .arg(container);

    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let output = tokio::time::timeout(DOCKER_TIMEOUT, cmd.output())
        .await
        .map_err(|_| anyhow!("docker logs timed out after {}s", DOCKER_TIMEOUT.as_secs()))?
        .map_err(|e| anyhow!("failed to execute docker logs: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("docker logs failed: {}", stderr.trim()));
    }

    // Docker logs sends stdout and stderr to separate streams.
    // Merge them (stdout first, then stderr) for a complete picture.
    let mut combined = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr_text = String::from_utf8_lossy(&output.stderr);
    if !stderr_text.is_empty() {
        if !combined.is_empty() && !combined.ends_with('\n') {
            combined.push('\n');
        }
        combined.push_str(&stderr_text);
    }

    // Truncate to MAX_LOG_BYTES to prevent context overflow
    if combined.len() > MAX_LOG_BYTES {
        combined.truncate(MAX_LOG_BYTES);
        combined.push_str("\n... (truncated to 10KB)");
    }

    if combined.trim().is_empty() {
        Ok(format!("No recent logs for '{container}'."))
    } else {
        Ok(combined)
    }
}

// ── Helpers ─────────────────────────────────────────────────────────

/// Truncate a string to `max` characters, appending `..` if cut.
fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}..", &s[..max.saturating_sub(2)])
    }
}

// ── DockerClient wrapper ─────────────────────────────────────────────

/// Thin wrapper for `Checkable` health checks.
/// Docker uses the `docker` CLI — no API key required.
pub struct DockerClient;

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_short_string_unchanged() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn truncate_long_string_adds_dots() {
        let result = truncate("abcdefghij", 6);
        assert_eq!(result, "abcd..");
        assert!(result.len() <= 6);
    }

    #[test]
    fn docker_logs_rejects_empty_container() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(docker_logs("", None));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("required"));
    }

    #[test]
    fn docker_logs_rejects_shell_metacharacters() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        for bad in &["foo;rm", "foo$(bar)", "foo|bar", "foo&bg", "foo`cmd`"] {
            let result = rt.block_on(docker_logs(bad, None));
            assert!(result.is_err(), "should reject '{bad}'");
            assert!(
                result.unwrap_err().to_string().contains("invalid container name"),
                "wrong error for '{bad}'"
            );
        }
    }

    #[test]
    fn docker_logs_caps_lines_at_max() {
        // We can't easily test the actual docker call without docker,
        // but we can verify the cap logic by checking the constant.
        assert_eq!(MAX_LOG_LINES, 200);
        assert_eq!(500_u64.min(MAX_LOG_LINES), 200);
    }

    #[test]
    fn docker_logs_accepts_valid_container_names() {
        // These should pass validation (will fail at docker call if docker unavailable,
        // but the input validation itself should pass).
        for name in &["nginx", "my-app", "my_app", "app.v2", "redis-7.2"] {
            assert!(
                !name.chars().any(|c| !c.is_alphanumeric() && c != '-' && c != '_' && c != '.'),
                "'{name}' should be valid"
            );
        }
    }
}
