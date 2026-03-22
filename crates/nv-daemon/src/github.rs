//! GitHub tools — read-only data source via the `gh` CLI.
//!
//! Shells out to `gh` (already authenticated via `~/.config/gh/hosts.yml`) to
//! query PRs, CI runs, and issues.  All output is JSON-parsed into typed
//! structs and formatted for Telegram delivery.

use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use serde::Deserialize;
use tokio::process::Command;

use crate::claude::ToolDefinition;

// ── Constants ────────────────────────────────────────────────────────

/// Execution timeout for `gh` CLI invocations (network latency).
const GH_TIMEOUT: Duration = Duration::from_secs(15);

// ── Types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrSummary {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub author: PrAuthor,
    pub updated_at: String,
    pub mergeable: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PrAuthor {
    pub login: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSummary {
    pub database_id: u64,
    pub display_title: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub event: String,
    pub head_branch: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueSummary {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub labels: Vec<IssueLabel>,
    pub assignees: Vec<IssueAssignee>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IssueLabel {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IssueAssignee {
    pub login: String,
}

// ── Validation ───────────────────────────────────────────────────────

/// Validate that `repo` matches `owner/repo` format.
///
/// Accepts `[a-zA-Z0-9_.-]+/[a-zA-Z0-9_.-]+` without pulling in a regex crate.
fn validate_repo(repo: &str) -> Result<()> {
    let parts: Vec<&str> = repo.split('/').collect();
    if parts.len() != 2 {
        bail!("invalid repo format: '{repo}' — expected 'owner/repo'");
    }
    let valid_char = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-';
    for part in &parts {
        if part.is_empty() || !part.chars().all(valid_char) {
            bail!("invalid repo format: '{repo}' — expected 'owner/repo'");
        }
    }
    Ok(())
}

// ── Shell Execution ──────────────────────────────────────────────────

/// Execute `gh` with the given arguments, capture stdout, and return it
/// as a string.  Detects common failure modes (binary not found, auth
/// expired) and returns actionable error messages.
async fn exec_gh(args: &[&str]) -> Result<String> {
    let result = tokio::time::timeout(GH_TIMEOUT, async {
        let output = Command::new("gh")
            .args(args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn();

        let child = match output {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                bail!("gh CLI not found — install with `pacman -S github-cli`");
            }
            Err(e) => bail!("failed to spawn gh: {e}"),
        };

        let output = child.wait_with_output().await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("auth login") || stderr.contains("not logged") {
                bail!("gh auth expired — run `gh auth login` to re-authenticate");
            }
            bail!("gh failed (exit {}): {}", output.status, stderr.trim());
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    })
    .await;

    match result {
        Ok(inner) => inner,
        Err(_) => bail!("gh command timed out after {}s", GH_TIMEOUT.as_secs()),
    }
}

// ── Parse Helpers ────────────────────────────────────────────────────

pub fn parse_pr_list(json: &str) -> Result<Vec<PrSummary>> {
    serde_json::from_str(json).map_err(|e| anyhow!("failed to parse PR list JSON: {e}"))
}

pub fn parse_run_status(json: &str) -> Result<Vec<RunSummary>> {
    serde_json::from_str(json).map_err(|e| anyhow!("failed to parse run status JSON: {e}"))
}

pub fn parse_issues(json: &str) -> Result<Vec<IssueSummary>> {
    serde_json::from_str(json).map_err(|e| anyhow!("failed to parse issues JSON: {e}"))
}

// ── Telegram Formatters ──────────────────────────────────────────────

impl PrSummary {
    pub fn format_for_telegram(&self) -> String {
        let mergeable = self
            .mergeable
            .as_deref()
            .unwrap_or("unknown");
        let icon = match mergeable {
            "MERGEABLE" => "\u{2705}",   // green check
            "CONFLICTING" => "\u{26a0}", // warning
            _ => "\u{2753}",             // question mark
        };
        format!(
            "#{} {} — {} (by {}, {})",
            self.number, icon, self.title, self.author.login, self.state
        )
    }
}

impl RunSummary {
    pub fn format_for_telegram(&self) -> String {
        let icon = match (self.status.as_str(), self.conclusion.as_deref()) {
            ("completed", Some("success")) => "\u{2705}",
            ("completed", Some("failure")) => "\u{274c}",
            ("completed", Some("cancelled")) => "\u{23f9}",
            ("in_progress", _) => "\u{1f504}",
            ("queued", _) => "\u{23f3}",
            _ => "\u{2753}",
        };
        let conclusion_str = self
            .conclusion
            .as_deref()
            .unwrap_or(&self.status);
        format!(
            "{} {} — {} [{}] ({})",
            icon, self.display_title, conclusion_str, self.head_branch, self.event
        )
    }
}

impl IssueSummary {
    pub fn format_for_telegram(&self) -> String {
        let labels = if self.labels.is_empty() {
            String::new()
        } else {
            let names: Vec<&str> = self.labels.iter().map(|l| l.name.as_str()).collect();
            format!(" [{}]", names.join(", "))
        };
        let assignees = if self.assignees.is_empty() {
            "unassigned".to_string()
        } else {
            let logins: Vec<&str> = self.assignees.iter().map(|a| a.login.as_str()).collect();
            logins.join(", ")
        };
        format!(
            "#{} {} — {}{} ({})",
            self.number, self.state, self.title, labels, assignees
        )
    }
}

// ── Public Tool Handlers ─────────────────────────────────────────────

/// List open PRs for a repository.
pub async fn gh_pr_list(repo: &str) -> Result<String> {
    validate_repo(repo)?;
    let json = exec_gh(&[
        "pr", "list",
        "--repo", repo,
        "--json", "number,title,state,author,updatedAt,mergeable",
        "--limit", "20",
    ])
    .await?;

    let prs = parse_pr_list(&json)?;
    if prs.is_empty() {
        return Ok(format!("No open PRs on {repo}."));
    }

    let mut lines = vec![format!("{} open PR(s) on {repo}:", prs.len())];
    for pr in &prs {
        lines.push(pr.format_for_telegram());
    }
    Ok(lines.join("\n"))
}

/// Show latest CI/CD run status for a repository.
pub async fn gh_run_status(repo: &str) -> Result<String> {
    validate_repo(repo)?;
    let json = exec_gh(&[
        "run", "list",
        "--repo", repo,
        "--json", "databaseId,displayTitle,status,conclusion,event,headBranch,updatedAt",
        "--limit", "10",
    ])
    .await?;

    let runs = parse_run_status(&json)?;
    if runs.is_empty() {
        return Ok(format!("No recent CI runs on {repo}."));
    }

    let mut lines = vec![format!("{} recent run(s) on {repo}:", runs.len())];
    for run in &runs {
        lines.push(run.format_for_telegram());
    }
    Ok(lines.join("\n"))
}

/// List open issues for a repository.
pub async fn gh_issues(repo: &str) -> Result<String> {
    validate_repo(repo)?;
    let json = exec_gh(&[
        "issue", "list",
        "--repo", repo,
        "--json", "number,title,state,labels,assignees,updatedAt",
        "--limit", "20",
    ])
    .await?;

    let issues = parse_issues(&json)?;
    if issues.is_empty() {
        return Ok(format!("No open issues on {repo}."));
    }

    let mut lines = vec![format!("{} open issue(s) on {repo}:", issues.len())];
    for issue in &issues {
        lines.push(issue.format_for_telegram());
    }
    Ok(lines.join("\n"))
}

// ── Tool Definitions ─────────────────────────────────────────────────

/// Return tool definitions for all GitHub tools.
pub fn github_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "gh_pr_list".into(),
            description: "List open pull requests for a GitHub repository. Returns PR number, title, state, author, and mergeable status.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "repo": {
                        "type": "string",
                        "description": "GitHub repository in owner/repo format (e.g. 'nyaptor/nv')"
                    }
                },
                "required": ["repo"]
            }),
        },
        ToolDefinition {
            name: "gh_run_status".into(),
            description: "Show recent CI/CD workflow run status for a GitHub repository. Returns run title, status, conclusion, branch, and trigger event. Failed runs are highlighted.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "repo": {
                        "type": "string",
                        "description": "GitHub repository in owner/repo format (e.g. 'nyaptor/nv')"
                    }
                },
                "required": ["repo"]
            }),
        },
        ToolDefinition {
            name: "gh_issues".into(),
            description: "List open issues for a GitHub repository. Returns issue number, title, state, labels, and assignees.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "repo": {
                        "type": "string",
                        "description": "GitHub repository in owner/repo format (e.g. 'nyaptor/nv')"
                    }
                },
                "required": ["repo"]
            }),
        },
    ]
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_repo_valid() {
        assert!(validate_repo("nyaptor/nv").is_ok());
        assert!(validate_repo("octocat/Hello-World").is_ok());
        assert!(validate_repo("my_org/my.repo").is_ok());
    }

    #[test]
    fn validate_repo_invalid() {
        assert!(validate_repo("just-a-name").is_err());
        assert!(validate_repo("").is_err());
        assert!(validate_repo("a/b/c").is_err());
        assert!(validate_repo("owner/ repo").is_err());
    }

    #[test]
    fn parse_pr_list_empty() {
        let prs = parse_pr_list("[]").unwrap();
        assert!(prs.is_empty());
    }

    #[test]
    fn parse_pr_list_single() {
        let json = r#"[{
            "number": 42,
            "title": "Add feature X",
            "state": "OPEN",
            "author": {"login": "octocat"},
            "updatedAt": "2026-03-22T10:00:00Z",
            "mergeable": "MERGEABLE"
        }]"#;
        let prs = parse_pr_list(json).unwrap();
        assert_eq!(prs.len(), 1);
        assert_eq!(prs[0].number, 42);
        assert_eq!(prs[0].title, "Add feature X");
        assert_eq!(prs[0].author.login, "octocat");
        assert_eq!(prs[0].mergeable.as_deref(), Some("MERGEABLE"));
    }

    #[test]
    fn parse_run_status_single() {
        let json = r#"[{
            "databaseId": 12345,
            "displayTitle": "CI",
            "status": "completed",
            "conclusion": "success",
            "event": "push",
            "headBranch": "main",
            "updatedAt": "2026-03-22T10:00:00Z"
        }]"#;
        let runs = parse_run_status(json).unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].display_title, "CI");
        assert_eq!(runs[0].conclusion.as_deref(), Some("success"));
    }

    #[test]
    fn parse_issues_single() {
        let json = r#"[{
            "number": 7,
            "title": "Bug in login",
            "state": "OPEN",
            "labels": [{"name": "bug"}],
            "assignees": [{"login": "leo"}],
            "updatedAt": "2026-03-22T10:00:00Z"
        }]"#;
        let issues = parse_issues(json).unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].number, 7);
        assert_eq!(issues[0].labels[0].name, "bug");
        assert_eq!(issues[0].assignees[0].login, "leo");
    }

    #[test]
    fn parse_pr_list_malformed_json() {
        assert!(parse_pr_list("not json").is_err());
    }

    #[test]
    fn parse_run_status_malformed_json() {
        assert!(parse_run_status("{bad}").is_err());
    }

    #[test]
    fn parse_issues_malformed_json() {
        assert!(parse_issues("[{invalid}]").is_err());
    }

    #[test]
    fn format_pr_for_telegram() {
        let pr = PrSummary {
            number: 42,
            title: "Add feature X".into(),
            state: "OPEN".into(),
            author: PrAuthor { login: "octocat".into() },
            updated_at: "2026-03-22T10:00:00Z".into(),
            mergeable: Some("MERGEABLE".into()),
        };
        let formatted = pr.format_for_telegram();
        assert!(formatted.contains("#42"));
        assert!(formatted.contains("Add feature X"));
        assert!(formatted.contains("octocat"));
        assert!(formatted.contains("\u{2705}"));
    }

    #[test]
    fn format_run_success_for_telegram() {
        let run = RunSummary {
            database_id: 1,
            display_title: "CI".into(),
            status: "completed".into(),
            conclusion: Some("success".into()),
            event: "push".into(),
            head_branch: "main".into(),
            updated_at: "2026-03-22T10:00:00Z".into(),
        };
        let formatted = run.format_for_telegram();
        assert!(formatted.contains("\u{2705}"));
        assert!(formatted.contains("CI"));
        assert!(formatted.contains("success"));
    }

    #[test]
    fn format_run_failure_for_telegram() {
        let run = RunSummary {
            database_id: 2,
            display_title: "Deploy".into(),
            status: "completed".into(),
            conclusion: Some("failure".into()),
            event: "push".into(),
            head_branch: "feat/x".into(),
            updated_at: "2026-03-22T10:00:00Z".into(),
        };
        let formatted = run.format_for_telegram();
        assert!(formatted.contains("\u{274c}"));
        assert!(formatted.contains("failure"));
    }

    #[test]
    fn format_issue_for_telegram() {
        let issue = IssueSummary {
            number: 7,
            title: "Bug in login".into(),
            state: "OPEN".into(),
            labels: vec![IssueLabel { name: "bug".into() }],
            assignees: vec![IssueAssignee { login: "leo".into() }],
            updated_at: "2026-03-22T10:00:00Z".into(),
        };
        let formatted = issue.format_for_telegram();
        assert!(formatted.contains("#7"));
        assert!(formatted.contains("Bug in login"));
        assert!(formatted.contains("[bug]"));
        assert!(formatted.contains("leo"));
    }

    #[test]
    fn format_issue_unassigned_no_labels() {
        let issue = IssueSummary {
            number: 1,
            title: "Something".into(),
            state: "OPEN".into(),
            labels: vec![],
            assignees: vec![],
            updated_at: "2026-03-22T10:00:00Z".into(),
        };
        let formatted = issue.format_for_telegram();
        assert!(formatted.contains("unassigned"));
        assert!(!formatted.contains("["));
    }

    #[test]
    fn github_tool_definitions_returns_three_tools() {
        let tools = github_tool_definitions();
        assert_eq!(tools.len(), 3);

        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"gh_pr_list"));
        assert!(names.contains(&"gh_run_status"));
        assert!(names.contains(&"gh_issues"));
    }

    #[test]
    fn tool_definitions_have_correct_schema() {
        let tools = github_tool_definitions();
        for tool in &tools {
            assert!(!tool.name.is_empty());
            assert!(!tool.description.is_empty());
            assert_eq!(tool.input_schema["type"], "object");
            assert!(tool.input_schema.get("properties").is_some());
            let required = tool.input_schema["required"].as_array().unwrap();
            assert!(required.iter().any(|v| v.as_str() == Some("repo")));
        }
    }
}
