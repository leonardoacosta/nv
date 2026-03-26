//! Python sidecar process manager.
//!
//! `SidecarManager` spawns `scripts/agent-sidecar.py` as a child process and
//! communicates with it over newline-delimited JSON on stdin/stdout.
//!
//! The sidecar handles Agent SDK calls (including OAuth auth and the tool-use
//! loop via MCP) and returns the final text response.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tokio::time::timeout;

// ── Request / Response wire types ────────────────────────────────────

/// Tool definition passed to the sidecar for MCP registration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// A content block in the sidecar response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    #[serde(default)]
    pub text: String,
}

/// Request sent to the sidecar over stdin.
#[derive(Debug, Serialize, Deserialize)]
pub struct SidecarRequest {
    pub id: String,
    pub system: String,
    pub prompt: String,
    pub tools: Vec<ToolDefinition>,
    pub max_turns: u32,
    pub timeout_secs: u64,
}

/// Response received from the sidecar over stdout.
#[derive(Debug, Serialize, Deserialize)]
pub struct SidecarResponse {
    pub id: String,
    pub content: Vec<ContentBlock>,
    pub stop_reason: String,
    pub error: Option<String>,
}

// ── SidecarManager ────────────────────────────────────────────────────

const MAX_RESTARTS: u32 = 3;
const RESTART_DELAY_SECS: u64 = 5;

/// Manages the Python agent-sidecar child process.
///
/// Spawns the sidecar on creation, communicates over newline-delimited JSON,
/// and restarts the process on crash (up to `MAX_RESTARTS` times).
pub struct SidecarManager {
    /// Path to the sidecar script (`{repo_root}/scripts/agent-sidecar.py`).
    script_path: PathBuf,
    /// Mutable inner state protected by a Mutex so `send_request` can
    /// respawn on crash.
    inner: Mutex<SidecarInner>,
    /// Total number of crash-restarts so far.
    restart_count: AtomicU32,
}

/// Inner mutable state — replaced on each (re)spawn.
struct SidecarInner {
    child: Child,
    stdin: tokio::process::ChildStdin,
    stdout: BufReader<tokio::process::ChildStdout>,
}

impl SidecarManager {
    /// Resolve the script path from the binary's parent directory or from
    /// `NV_REPO_ROOT` env var, then spawn the sidecar process.
    ///
    /// `repo_root` is the directory that contains `scripts/agent-sidecar.py`.
    pub async fn spawn(repo_root: &std::path::Path) -> Result<Arc<Self>> {
        let script_path = repo_root.join("scripts").join("agent-sidecar.py");
        if !script_path.exists() {
            return Err(anyhow!(
                "sidecar script not found at {}",
                script_path.display()
            ));
        }

        let inner = Self::spawn_child(&script_path).await?;
        tracing::info!(
            script = %script_path.display(),
            "agent sidecar spawned"
        );

        Ok(Arc::new(Self {
            script_path,
            inner: Mutex::new(inner),
            restart_count: AtomicU32::new(0),
        }))
    }

    /// Spawn the child process and return the inner state.
    async fn spawn_child(script_path: &std::path::Path) -> Result<SidecarInner> {
        let mut child = Command::new("python3")
            .arg(script_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .context("failed to spawn python3 agent-sidecar.py")?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("sidecar stdin handle missing"))?;
        let stdout_raw = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("sidecar stdout handle missing"))?;
        let stdout = BufReader::new(stdout_raw);

        Ok(SidecarInner { child, stdin, stdout })
    }

    /// Send a request to the sidecar and wait for the response.
    ///
    /// If the sidecar process has crashed, this method attempts to respawn it
    /// (up to `MAX_RESTARTS` times) before returning an error.
    pub async fn send_request(&self, req: SidecarRequest) -> Result<SidecarResponse> {
        let req_timeout = Duration::from_secs(req.timeout_secs.max(30));

        let mut inner = self.inner.lock().await;

        // Check if the child has already exited.
        if let Ok(Some(status)) = inner.child.try_wait() {
            tracing::warn!(status = ?status, "sidecar exited unexpectedly, attempting respawn");
            self.attempt_respawn(&mut inner).await?;
        }

        // Serialize and write the request.
        let mut line = serde_json::to_string(&req).context("failed to serialize SidecarRequest")?;
        line.push('\n');

        if let Err(e) = inner.stdin.write_all(line.as_bytes()).await {
            tracing::warn!(error = %e, "stdin write failed, attempting respawn");
            self.attempt_respawn(&mut inner).await?;
            // Retry once after respawn.
            let mut line2 = serde_json::to_string(&req).context("serialize retry")?;
            line2.push('\n');
            inner
                .stdin
                .write_all(line2.as_bytes())
                .await
                .context("stdin write failed after respawn")?;
        }

        inner
            .stdin
            .flush()
            .await
            .context("failed to flush sidecar stdin")?;

        // Read one response line with timeout.
        let mut response_line = String::new();
        let read_result = timeout(req_timeout, inner.stdout.read_line(&mut response_line)).await;

        match read_result {
            Err(_) => {
                // Timeout — kill and respawn.
                tracing::warn!(
                    timeout_secs = req.timeout_secs,
                    "sidecar response timed out, killing process"
                );
                let _ = inner.child.kill().await;
                drop(inner);
                // Allow caller to retry or fall back.
                Err(anyhow!("sidecar response timed out after {}s", req.timeout_secs))
            }
            Ok(Err(e)) => {
                tracing::warn!(error = %e, "sidecar stdout read error");
                Err(anyhow!("sidecar stdout read error: {e}"))
            }
            Ok(Ok(0)) => {
                // EOF — sidecar crashed.
                tracing::warn!("sidecar stdout EOF (process crashed)");
                self.attempt_respawn(&mut inner).await?;
                Err(anyhow!("sidecar crashed (EOF); respawned for next request"))
            }
            Ok(Ok(_)) => {
                serde_json::from_str::<SidecarResponse>(response_line.trim())
                    .context("failed to deserialize SidecarResponse")
            }
        }
    }

    /// Respawn the sidecar child process.  Called when a crash is detected.
    ///
    /// Increments the restart counter and fails if `MAX_RESTARTS` is exceeded.
    async fn attempt_respawn(&self, inner: &mut SidecarInner) -> Result<()> {
        let count = self.restart_count.fetch_add(1, Ordering::SeqCst) + 1;
        if count > MAX_RESTARTS {
            return Err(anyhow!(
                "sidecar exceeded max restart limit ({MAX_RESTARTS}); giving up"
            ));
        }

        tracing::warn!(
            restart = count,
            max = MAX_RESTARTS,
            delay_secs = RESTART_DELAY_SECS,
            "respawning sidecar after crash"
        );

        // Wait before respawning to avoid tight crash loops.
        tokio::time::sleep(Duration::from_secs(RESTART_DELAY_SECS)).await;

        let new_inner = Self::spawn_child(&self.script_path)
            .await
            .context("failed to respawn sidecar")?;
        *inner = new_inner;

        tracing::info!(restart = count, "sidecar respawned successfully");
        Ok(())
    }

    /// Graceful shutdown: send SIGTERM to the child, wait up to 5 seconds,
    /// then SIGKILL if it hasn't exited.
    pub async fn shutdown(&self) {
        let mut inner = self.inner.lock().await;

        // Close stdin to signal EOF to the Python process.
        // We can't easily close it without replacing it, so we just send SIGTERM.
        #[cfg(unix)]
        {
            if let Some(pid) = inner.child.id() {
                unsafe {
                    libc::kill(pid as libc::pid_t, libc::SIGTERM);
                }
            }
        }

        match timeout(Duration::from_secs(5), inner.child.wait()).await {
            Ok(Ok(status)) => {
                tracing::info!(status = ?status, "sidecar exited cleanly on shutdown");
            }
            Ok(Err(e)) => {
                tracing::warn!(error = %e, "error waiting for sidecar on shutdown");
            }
            Err(_) => {
                tracing::warn!("sidecar did not exit within 5s, killing");
                let _ = inner.child.kill().await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidecar_request_roundtrip() {
        let req = SidecarRequest {
            id: "test-123".into(),
            system: "You are Nova.".into(),
            prompt: "hello".into(),
            tools: vec![ToolDefinition {
                name: "read_memory".into(),
                description: "Reads a memory topic.".into(),
                input_schema: serde_json::json!({ "type": "object", "properties": { "topic": { "type": "string" } } }),
            }],
            max_turns: 5,
            timeout_secs: 30,
        };
        let json = serde_json::to_string(&req).unwrap();
        let decoded: SidecarRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.id, "test-123");
        assert_eq!(decoded.tools.len(), 1);
        assert_eq!(decoded.tools[0].name, "read_memory");
    }

    #[test]
    fn sidecar_response_roundtrip() {
        let resp = SidecarResponse {
            id: "test-123".into(),
            content: vec![ContentBlock {
                block_type: "text".into(),
                text: "Memory contains: hello".into(),
            }],
            stop_reason: "end_turn".into(),
            error: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let decoded: SidecarResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.id, "test-123");
        assert_eq!(decoded.content[0].text, "Memory contains: hello");
        assert!(decoded.error.is_none());
    }
}
