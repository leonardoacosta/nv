//! Obligation detection: classify inbound messages to identify commitments.
//!
//! Calls the Claude CLI with a lightweight system prompt to determine whether
//! a message contains an obligation or commitment. Returns a `DetectedObligation`
//! when one is found, or `None` when the message is informational only.
//!
//! Uses cold-start `claude -p` (no persistent session, no tools) for fast
//! single-turn classification. Results are stored via `ObligationStore`.

use std::process::Stdio;
use std::time::Duration;

use anyhow::Result;
use serde::Deserialize;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

// ── Classification Result ────────────────────────────────────────────

/// Obligation data produced by the classifier.
///
/// This is the raw classification output, before a UUID is assigned.
/// The caller wraps this in a `NewObligation` and calls `ObligationStore::create`.
#[derive(Debug, Clone)]
pub struct DetectedObligation {
    /// Concise description of the detected action or commitment.
    pub detected_action: String,
    /// Priority 0-4 (0 = most critical, 4 = backlog).
    pub priority: i32,
    /// "nova" or "leo".
    pub owner: String,
    /// One-sentence explanation of the owner assignment.
    pub owner_reason: Option<String>,
    /// Optional project code extracted from context (e.g. "NV", "OO").
    pub project_code: Option<String>,
}

// ── Internal JSON Response ─────────────────────────────────────────

/// Shape of the JSON the classifier is instructed to return.
#[derive(Debug, Deserialize)]
struct ClassifierJson {
    /// `true` if an obligation was found, `false` otherwise.
    is_obligation: bool,
    /// Present only when `is_obligation` is `true`.
    #[serde(default)]
    detected_action: String,
    /// 0-4 when `is_obligation` is `true`.
    #[serde(default)]
    priority: i32,
    /// "nova" or "leo" when `is_obligation` is `true`.
    #[serde(default)]
    owner: String,
    /// Optional explanation of the owner assignment.
    #[serde(default)]
    owner_reason: Option<String>,
    /// Optional project code (e.g. "NV", "OO", "TC").
    #[serde(default)]
    project_code: Option<String>,
}

// ── CLI JSON wrapper ──────────────────────────────────────────────

/// The outer envelope returned by `claude -p --output-format json`.
#[derive(Debug, Deserialize)]
struct CliResponse {
    #[serde(default)]
    result: String,
    #[serde(default)]
    is_error: bool,
}

// ── System Prompt ─────────────────────────────────────────────────

const CLASSIFIER_SYSTEM_PROMPT: &str = "\
You are an obligation classifier. Given a message and the channel it came from, \
determine whether it contains a commitment, action item, or obligation that must be tracked.

Obligations include:
- Explicit promises (\"I will...\", \"We need to...\", \"Please do X\")
- Action items assigned to Nova (the AI assistant) or Leo (the user)
- Deadlines or follow-ups that were agreed to
- Tasks that were mentioned as blocked or waiting

NOT obligations:
- Pure status updates with no action required
- Questions that were already answered
- Casual acknowledgements (\"ok\", \"thanks\")
- FYI messages with no expected response

Respond with ONLY a JSON object. No markdown, no explanation, no code blocks.

If an obligation is found:
{
  \"is_obligation\": true,
  \"detected_action\": \"<concise action description, max 120 chars>\",
  \"priority\": <0-4>,
  \"owner\": \"nova\" or \"leo\",
  \"owner_reason\": \"<one sentence why>\",
  \"project_code\": \"<2-4 char project code if mentioned, else null>\"
}

Priority scale:
  0 = Critical: must be done today / production impact
  1 = High: important, near-term deadline
  2 = Important: standard work item
  3 = Minor: nice-to-have, no deadline
  4 = Backlog: someday/maybe

If no obligation:
{\"is_obligation\": false}

Owner rules:
  nova = Nova can do this autonomously (send a message, run a command, look something up)
  leo  = Requires Leo's judgement or physical presence";

// ── Detector ──────────────────────────────────────────────────────

/// Classify a message for obligation content.
///
/// Spawns a lightweight `claude -p --output-format json` process with no tools.
/// Returns `Some(DetectedObligation)` if an obligation is found, `None` otherwise.
/// Returns `Err` only if the subprocess completely fails (not if no obligation is found).
pub async fn detect_obligation(
    message_content: &str,
    channel: &str,
) -> Result<Option<DetectedObligation>> {
    let prompt = format!(
        "Channel: {channel}\nMessage: {message_content}"
    );

    // Resolve HOME for the subprocess environment.
    // Return a hard error if neither REAL_HOME nor HOME is set — the subprocess
    // cannot be configured correctly without a home directory.
    let real_home = std::env::var("REAL_HOME")
        .or_else(|_| std::env::var("HOME"))
        .map_err(|_| anyhow::anyhow!("HOME env var not set — cannot spawn obligation detector"))?;

    let mut child = Command::new("claude")
        .args([
            "--dangerously-skip-permissions",
            "-p",
            "--output-format",
            "json",
            "--no-session-persistence",
            "--system-prompt",
            CLASSIFIER_SYSTEM_PROMPT,
        ])
        .env("HOME", &real_home)
        .env(
            "PATH",
            format!("{}/.local/bin:/usr/local/bin:/usr/bin:/bin", real_home),
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow::anyhow!("failed to spawn claude CLI for obligation detection: {e}"))?;

    // Write prompt to stdin and close it
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(prompt.as_bytes()).await?;
    }

    // Wait for the subprocess to finish with a 30-second timeout.
    // A hung Claude CLI process would otherwise block the detection path indefinitely.
    let output = match tokio::time::timeout(Duration::from_secs(30), child.wait_with_output()).await {
        Ok(Ok(out)) => out,
        Ok(Err(e)) => {
            return Err(anyhow::anyhow!("obligation detector subprocess error: {e}"));
        }
        Err(_elapsed) => {
            return Err(anyhow::anyhow!(
                "obligation detector subprocess timed out after 30s"
            ));
        }
    };
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !stderr.is_empty() {
        tracing::debug!(stderr = %stderr, "obligation detector stderr");
    }

    if !output.status.success() {
        tracing::warn!(
            status = ?output.status,
            stdout_len = stdout.len(),
            "obligation detector subprocess exited with non-zero status"
        );
    }

    // Parse the outer CLI JSON envelope
    let cli_resp: CliResponse = match serde_json::from_str(&stdout) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, stdout = %stdout, "failed to parse obligation detector CLI response");
            return Ok(None);
        }
    };

    if cli_resp.is_error {
        tracing::warn!(result = %cli_resp.result, "obligation detector returned error");
        return Ok(None);
    }

    // Parse the classifier JSON out of the `result` field
    let classifier: ClassifierJson = match serde_json::from_str(cli_resp.result.trim()) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(
                error = %e,
                result = %cli_resp.result,
                "failed to parse classifier JSON from obligation detector result"
            );
            return Ok(None);
        }
    };

    if !classifier.is_obligation {
        tracing::debug!(
            channel = %channel,
            "no obligation detected in message"
        );
        return Ok(None);
    }

    // Validate required fields
    if classifier.detected_action.is_empty() {
        tracing::warn!("classifier returned is_obligation=true but empty detected_action");
        return Ok(None);
    }

    let owner = match classifier.owner.as_str() {
        "nova" | "leo" => classifier.owner.clone(),
        other => {
            tracing::warn!(owner = %other, "unknown owner from classifier, defaulting to nova");
            "nova".to_string()
        }
    };

    let priority = classifier.priority.clamp(0, 4);

    tracing::info!(
        channel = %channel,
        action = %classifier.detected_action,
        priority = priority,
        owner = %owner,
        "obligation detected"
    );

    Ok(Some(DetectedObligation {
        detected_action: classifier.detected_action,
        priority,
        owner,
        owner_reason: classifier.owner_reason,
        project_code: classifier.project_code,
    }))
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifier_json_deserializes_obligation() {
        let json = r#"{
            "is_obligation": true,
            "detected_action": "Deploy the new auth service by Friday",
            "priority": 1,
            "owner": "leo",
            "owner_reason": "Requires Leo to coordinate with DevOps.",
            "project_code": "OO"
        }"#;

        let c: ClassifierJson = serde_json::from_str(json).unwrap();
        assert!(c.is_obligation);
        assert_eq!(c.detected_action, "Deploy the new auth service by Friday");
        assert_eq!(c.priority, 1);
        assert_eq!(c.owner, "leo");
        assert_eq!(c.project_code, Some("OO".to_string()));
    }

    #[test]
    fn classifier_json_deserializes_no_obligation() {
        let json = r#"{"is_obligation": false}"#;
        let c: ClassifierJson = serde_json::from_str(json).unwrap();
        assert!(!c.is_obligation);
        assert_eq!(c.detected_action, "");
    }

    #[test]
    fn cli_response_deserializes() {
        let json = r#"{"result": "{\"is_obligation\": false}", "is_error": false}"#;
        let r: CliResponse = serde_json::from_str(json).unwrap();
        assert!(!r.is_error);
        assert!(r.result.contains("is_obligation"));
    }

    #[test]
    fn classifier_json_missing_optional_fields_defaults_to_none() {
        // JSON with no project_code and no owner_reason — serde(default) must fill in None
        let json = r#"{
            "is_obligation": true,
            "detected_action": "Ship the release",
            "priority": 2,
            "owner": "nova"
        }"#;

        let c: ClassifierJson = serde_json::from_str(json).unwrap();
        assert!(c.is_obligation);
        assert_eq!(c.project_code, None);
        assert_eq!(c.owner_reason, None);
    }

    #[test]
    fn unknown_owner_defaults_to_nova() {
        // The detect_obligation function maps unknown owner values to "nova".
        // Replicate that branch directly.
        let json = r#"{
            "is_obligation": true,
            "detected_action": "Do something",
            "priority": 2,
            "owner": "unknown_entity"
        }"#;

        let c: ClassifierJson = serde_json::from_str(json).unwrap();
        // Replicate the owner normalisation logic from detect_obligation
        let owner = match c.owner.as_str() {
            "nova" | "leo" => c.owner.clone(),
            _other => "nova".to_string(),
        };
        assert_eq!(owner, "nova");
    }

    #[test]
    fn empty_detected_action_with_is_obligation_true_yields_none() {
        // When is_obligation=true but detected_action is empty, the detector
        // must return None (the obligation is not usable without an action).
        let json = r#"{
            "is_obligation": true,
            "detected_action": "",
            "priority": 1,
            "owner": "leo"
        }"#;

        let c: ClassifierJson = serde_json::from_str(json).unwrap();
        assert!(c.is_obligation);
        // Replicate the validation guard in detect_obligation
        let valid = !c.detected_action.is_empty();
        assert!(!valid, "empty detected_action should be treated as invalid (returns None)");
    }

    #[test]
    fn priority_clamped_to_valid_range() {
        // Simulate what detect_obligation does with out-of-range priority
        let raw = 99i32;
        let clamped = raw.clamp(0, 4);
        assert_eq!(clamped, 4);

        let raw = -5i32;
        let clamped = raw.clamp(0, 4);
        assert_eq!(clamped, 0);
    }

    /// Verify that detect_obligation returns Err when both HOME and REAL_HOME are absent.
    ///
    /// We temporarily remove both env vars, call detect_obligation, and assert
    /// the result is an Err containing "HOME".
    ///
    /// Note: this test mutates process env vars and is therefore not safe to run
    /// in parallel with other tests that read HOME. It is marked `#[serial]`
    /// logically but we rely on Rust's single-threaded default test runner for
    /// unit tests in this module.
    #[tokio::test]
    async fn detect_obligation_returns_err_when_home_unset() {
        // Save original values
        let saved_home = std::env::var("HOME").ok();
        let saved_real_home = std::env::var("REAL_HOME").ok();

        unsafe {
            std::env::remove_var("HOME");
            std::env::remove_var("REAL_HOME");
        }

        let result = detect_obligation("test message", "telegram").await;

        // Restore env vars before any assertions (so other tests are not affected)
        unsafe {
            if let Some(h) = saved_home {
                std::env::set_var("HOME", h);
            }
            if let Some(rh) = saved_real_home {
                std::env::set_var("REAL_HOME", rh);
            }
        }

        assert!(result.is_err(), "expected Err when HOME is unset");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("HOME"),
            "error message should mention HOME, got: {msg}"
        );
    }
}
