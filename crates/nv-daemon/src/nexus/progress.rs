//! Session progress parsing for workflow commands.
//!
//! Parses Nexus session metadata (command field, spec field, status) to
//! detect workflow phase and estimate completion percentage for the
//! `/apply`, `/ci:gh`, and `/feature` workflows.

use serde::{Deserialize, Serialize};

/// Workflow type detected from the session command or spec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowKind {
    Apply,
    CiGh,
    Feature,
    Unknown,
}

/// Current phase of a detected workflow.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowPhase {
    // /apply phases
    Db,
    Api,
    Ui,
    E2e,
    // /ci:gh phases
    Watch,
    Triage,
    Investigate,
    Propose,
    // /feature phases
    Discovery,
    Refinement,
    Spec,
    // Fallback
    Unknown,
}

impl WorkflowPhase {
    /// Estimated progress percentage for this phase.
    pub fn progress_pct(&self, kind: &WorkflowKind) -> u8 {
        match kind {
            WorkflowKind::Apply => match self {
                WorkflowPhase::Db => 15,
                WorkflowPhase::Api => 35,
                WorkflowPhase::Ui => 65,
                WorkflowPhase::E2e => 90,
                _ => 5,
            },
            WorkflowKind::CiGh => match self {
                WorkflowPhase::Watch => 10,
                WorkflowPhase::Triage => 30,
                WorkflowPhase::Investigate => 60,
                WorkflowPhase::Propose => 85,
                _ => 5,
            },
            WorkflowKind::Feature => match self {
                WorkflowPhase::Discovery => 20,
                WorkflowPhase::Refinement => 55,
                WorkflowPhase::Spec => 85,
                _ => 5,
            },
            WorkflowKind::Unknown => 0,
        }
    }

    /// Human-readable label for display in the dashboard.
    pub fn label(&self) -> &'static str {
        match self {
            WorkflowPhase::Db => "DB batch",
            WorkflowPhase::Api => "API batch",
            WorkflowPhase::Ui => "UI batch",
            WorkflowPhase::E2e => "E2E batch",
            WorkflowPhase::Watch => "Watching CI",
            WorkflowPhase::Triage => "Triaging",
            WorkflowPhase::Investigate => "Investigating",
            WorkflowPhase::Propose => "Proposing fix",
            WorkflowPhase::Discovery => "Discovery",
            WorkflowPhase::Refinement => "Refinement",
            WorkflowPhase::Spec => "Spec creation",
            WorkflowPhase::Unknown => "Running",
        }
    }
}

/// Progress information derived from session metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionProgress {
    /// Detected workflow kind.
    pub workflow: WorkflowKind,
    /// Current phase within the workflow.
    pub phase: WorkflowPhase,
    /// Estimated completion percentage 0–100.
    pub progress_pct: u8,
    /// Human-readable phase label.
    pub phase_label: String,
}

/// Detect workflow kind from a command string.
///
/// Checks for `/apply`, `/ci:gh`, and `/feature` command prefixes.
fn detect_workflow(command: Option<&str>, spec: Option<&str>) -> WorkflowKind {
    if let Some(cmd) = command {
        let cmd = cmd.trim();
        if cmd.contains("/apply") {
            return WorkflowKind::Apply;
        }
        if cmd.contains("/ci:gh") || cmd.contains("/ci") {
            return WorkflowKind::CiGh;
        }
        if cmd.contains("/feature") {
            return WorkflowKind::Feature;
        }
    }
    // Fall back: if a spec is set the session was likely started by /apply
    if spec.is_some() {
        return WorkflowKind::Apply;
    }
    WorkflowKind::Unknown
}

/// Detect current phase from command arguments or spec name.
///
/// Uses keyword matching on the command tail and spec string.
fn detect_phase(command: Option<&str>, spec: Option<&str>, kind: &WorkflowKind) -> WorkflowPhase {
    let haystack = format!(
        "{} {}",
        command.unwrap_or(""),
        spec.unwrap_or("")
    )
    .to_lowercase();

    match kind {
        WorkflowKind::Apply => {
            if haystack.contains("e2e") || haystack.contains("test") {
                WorkflowPhase::E2e
            } else if haystack.contains("ui") || haystack.contains("dashboard") {
                WorkflowPhase::Ui
            } else if haystack.contains("api") {
                WorkflowPhase::Api
            } else if haystack.contains("db") || haystack.contains("schema") || haystack.contains("migration") {
                WorkflowPhase::Db
            } else {
                WorkflowPhase::Unknown
            }
        }
        WorkflowKind::CiGh => {
            if haystack.contains("propose") || haystack.contains("fix") {
                WorkflowPhase::Propose
            } else if haystack.contains("investigate") {
                WorkflowPhase::Investigate
            } else if haystack.contains("triage") {
                WorkflowPhase::Triage
            } else {
                WorkflowPhase::Watch
            }
        }
        WorkflowKind::Feature => {
            if haystack.contains("spec") {
                WorkflowPhase::Spec
            } else if haystack.contains("refine") {
                WorkflowPhase::Refinement
            } else {
                WorkflowPhase::Discovery
            }
        }
        WorkflowKind::Unknown => WorkflowPhase::Unknown,
    }
}

/// Parse session metadata into progress information.
///
/// `command` is the raw command string stored on the session (may be None).
/// `spec` is the spec name associated with the session (may be None).
/// `status` is the session status string ("active", "idle", "stale", etc.).
pub fn parse_session_progress(
    command: Option<&str>,
    spec: Option<&str>,
    status: &str,
) -> SessionProgress {
    // Completed/idle sessions show 100%
    if status == "idle" || status == "stale" {
        return SessionProgress {
            workflow: WorkflowKind::Unknown,
            phase: WorkflowPhase::Unknown,
            progress_pct: 100,
            phase_label: "Complete".to_string(),
        };
    }

    let kind = detect_workflow(command, spec);
    let phase = detect_phase(command, spec, &kind);
    let progress_pct = phase.progress_pct(&kind);
    let phase_label = phase.label().to_string();

    SessionProgress {
        workflow: kind,
        phase,
        progress_pct,
        phase_label,
    }
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_apply_from_command() {
        let p = parse_session_progress(Some("/apply add-nexus-context-injection"), None, "active");
        assert_eq!(p.workflow, WorkflowKind::Apply);
    }

    #[test]
    fn detect_apply_from_spec() {
        let p = parse_session_progress(None, Some("add-nexus-context-injection"), "active");
        assert_eq!(p.workflow, WorkflowKind::Apply);
    }

    #[test]
    fn detect_cigh_from_command() {
        let p = parse_session_progress(Some("/ci:gh --fix"), None, "active");
        assert_eq!(p.workflow, WorkflowKind::CiGh);
    }

    #[test]
    fn detect_feature_from_command() {
        let p = parse_session_progress(Some("/feature new-payment-flow"), None, "active");
        assert_eq!(p.workflow, WorkflowKind::Feature);
    }

    #[test]
    fn idle_session_returns_100() {
        let p = parse_session_progress(Some("/apply foo"), None, "idle");
        assert_eq!(p.progress_pct, 100);
        assert_eq!(p.phase_label, "Complete");
    }

    #[test]
    fn stale_session_returns_100() {
        let p = parse_session_progress(None, None, "stale");
        assert_eq!(p.progress_pct, 100);
    }

    #[test]
    fn apply_db_phase_detection() {
        let p = parse_session_progress(Some("/apply add-db-schema"), None, "active");
        assert_eq!(p.phase, WorkflowPhase::Db);
        assert_eq!(p.progress_pct, 15);
    }

    #[test]
    fn apply_ui_phase_detection() {
        let p = parse_session_progress(Some("/apply add-dashboard-ui"), None, "active");
        assert_eq!(p.phase, WorkflowPhase::Ui);
        assert_eq!(p.progress_pct, 65);
    }

    #[test]
    fn cigh_watch_phase_is_default() {
        let p = parse_session_progress(Some("/ci:gh"), None, "active");
        assert_eq!(p.workflow, WorkflowKind::CiGh);
        assert_eq!(p.phase, WorkflowPhase::Watch);
        assert_eq!(p.progress_pct, 10);
    }

    #[test]
    fn feature_discovery_phase_is_default() {
        let p = parse_session_progress(Some("/feature"), None, "active");
        assert_eq!(p.phase, WorkflowPhase::Discovery);
        assert_eq!(p.progress_pct, 20);
    }

    #[test]
    fn unknown_command_returns_zero_progress() {
        let p = parse_session_progress(None, None, "active");
        assert_eq!(p.workflow, WorkflowKind::Unknown);
        assert_eq!(p.progress_pct, 0);
    }

    #[test]
    fn phase_labels_are_nonempty() {
        for phase in [
            WorkflowPhase::Db,
            WorkflowPhase::Api,
            WorkflowPhase::Ui,
            WorkflowPhase::E2e,
            WorkflowPhase::Watch,
            WorkflowPhase::Triage,
            WorkflowPhase::Investigate,
            WorkflowPhase::Propose,
            WorkflowPhase::Discovery,
            WorkflowPhase::Refinement,
            WorkflowPhase::Spec,
            WorkflowPhase::Unknown,
        ] {
            assert!(!phase.label().is_empty());
        }
    }
}
