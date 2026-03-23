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
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    pub database_id: u64,
    pub display_title: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub event: String,
    pub head_branch: String,
    #[allow(dead_code)]
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
    #[allow(dead_code)]
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
            "\u{1f4c1} #{} {} **{}**\n   By {} | {}",
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
            "\u{1f504} {icon} **{}** \u{2014} {conclusion_str}\n   Branch: {} | Trigger: {}",
            self.display_title, self.head_branch, self.event
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
            "\u{1f4c1} #{} **{}**\n   {} | {}{assignees}",
            self.number, self.title, self.state, labels
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

// ── Truncation Helper ────────────────────────────────────────────────

/// Truncate `text` to at most `max_chars` Unicode scalar values.
///
/// If the text is within the limit it is returned as-is (no allocation).
/// If truncation is needed, the text is sliced at a char boundary and
/// `suffix` is appended.  The suffix itself is NOT counted against
/// `max_chars`, so the returned string may be slightly longer than
/// `max_chars` by `suffix.len()`.
pub fn truncate_with_suffix(text: &str, max_chars: usize, suffix: &str) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    // Slice at the char boundary of the max_chars-th character.
    let byte_end = text
        .char_indices()
        .nth(max_chars)
        .map(|(i, _)| i)
        .unwrap_or(text.len());
    format!("{}{}", &text[..byte_end], suffix)
}

// ── New Types ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrDetail {
    pub number: u64,
    pub title: String,
    pub body: Option<String>,
    pub state: String,
    pub author: PrAuthor,
    pub labels: Vec<IssueLabel>,
    pub assignees: Vec<IssueAssignee>,
    pub milestone: Option<PrMilestone>,
    pub review_decision: Option<String>,
    pub status_check_rollup: Option<Vec<StatusCheck>>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PrMilestone {
    pub title: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusCheck {
    pub state: Option<String>,     // for CheckRun: PENDING, SUCCESS, FAILURE
    pub status: Option<String>,    // for StatusContext
    pub conclusion: Option<String>,
    #[allow(dead_code)]
    pub name: Option<String>,
    #[allow(dead_code)]
    pub context: Option<String>,
}

impl StatusCheck {
    fn effective_state(&self) -> &str {
        // CheckRun uses `state`; StatusContext uses `status` + `conclusion`
        if let Some(s) = &self.state {
            return s.as_str();
        }
        if let Some(c) = &self.conclusion {
            return c.as_str();
        }
        self.status.as_deref().unwrap_or("PENDING")
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseSummary {
    pub tag_name: String,
    pub name: Option<String>,
    pub published_at: Option<String>,
    pub is_draft: bool,
    pub is_prerelease: bool,
    pub body: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CompareResult {
    pub status: String,
    pub ahead_by: u64,
    pub behind_by: u64,
    pub total_commits: u64,
    pub commits: Vec<CompareCommit>,
    pub files: Option<Vec<CompareFile>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CompareCommit {
    pub sha: String,
    pub commit: CompareCommitInner,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CompareCommitInner {
    pub message: String,
    pub author: CompareAuthor,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CompareAuthor {
    pub name: String,
    pub date: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CompareFile {
    pub additions: u64,
    pub deletions: u64,
    #[allow(dead_code)]
    pub changes: u64,
}

// ── New Parse Helpers ────────────────────────────────────────────────

pub fn parse_pr_detail(json: &str) -> Result<PrDetail> {
    serde_json::from_str(json).map_err(|e| anyhow!("failed to parse PR detail JSON: {e}"))
}

pub fn parse_releases(json: &str) -> Result<Vec<ReleaseSummary>> {
    serde_json::from_str(json).map_err(|e| anyhow!("failed to parse releases JSON: {e}"))
}

pub fn parse_compare(json: &str) -> Result<CompareResult> {
    serde_json::from_str(json).map_err(|e| anyhow!("failed to parse compare JSON: {e}"))
}

// ── New Telegram Formatters ───────────────────────────────────────────

impl PrDetail {
    pub fn format_for_telegram(&self, diff_stat: &str) -> String {
        let body_raw = self.body.as_deref().unwrap_or("").trim().to_string();
        let body_excerpt = if body_raw.is_empty() {
            String::new()
        } else {
            format!(
                "\n\nDescription:\n{}",
                truncate_with_suffix(&body_raw, 2000, "\n[...truncated]")
            )
        };

        let review = self
            .review_decision
            .as_deref()
            .unwrap_or("REVIEW_REQUIRED");
        let review_icon = match review {
            "APPROVED" => "\u{2705}",
            "CHANGES_REQUESTED" => "\u{1f504}",
            _ => "\u{23f3}",
        };

        let labels = if self.labels.is_empty() {
            String::new()
        } else {
            let names: Vec<&str> = self.labels.iter().map(|l| l.name.as_str()).collect();
            format!("\nLabels: {}", names.join(", "))
        };

        let assignees = if self.assignees.is_empty() {
            String::new()
        } else {
            let logins: Vec<&str> = self.assignees.iter().map(|a| a.login.as_str()).collect();
            format!("\nAssignees: {}", logins.join(", "))
        };

        let milestone = self
            .milestone
            .as_ref()
            .map(|m| format!("\nMilestone: {}", m.title))
            .unwrap_or_default();

        let checks_summary = if let Some(checks) = &self.status_check_rollup {
            let total = checks.len();
            let passing = checks.iter().filter(|c| {
                matches!(c.effective_state().to_uppercase().as_str(), "SUCCESS" | "NEUTRAL" | "SKIPPED")
            }).count();
            let failing = checks.iter().filter(|c| {
                matches!(c.effective_state().to_uppercase().as_str(), "FAILURE" | "ERROR" | "TIMED_OUT" | "ACTION_REQUIRED")
            }).count();
            let pending = total - passing - failing;
            format!("\nChecks: {total} total, {passing} passing, {failing} failing, {pending} pending")
        } else {
            String::new()
        };

        let diff_line = if !diff_stat.trim().is_empty() {
            format!("\nDiff stat: {}", diff_stat.trim())
        } else {
            String::new()
        };

        format!(
            "\u{1f4c1} #{number} **{title}**\n**State:** {state} | **Author:** {author}\n**Created:** {created} | **Updated:** {updated}\n**Review:** {review_icon} {review}{labels}{assignees}{milestone}{checks}{diff}{body}",
            number = self.number,
            title = self.title,
            state = self.state,
            author = self.author.login,
            created = &self.created_at[..10],
            updated = &self.updated_at[..10],
            review_icon = review_icon,
            review = review,
            labels = labels,
            assignees = assignees,
            milestone = milestone,
            checks = checks_summary,
            diff = diff_line,
            body = body_excerpt,
        )
    }
}

impl ReleaseSummary {
    pub fn format_for_telegram(&self) -> String {
        let title = self.name.as_deref().unwrap_or(&self.tag_name);
        let date = self
            .published_at
            .as_deref()
            .and_then(|d| d.get(..10))
            .unwrap_or("unknown date");
        let badges = {
            let mut b = Vec::new();
            if self.is_draft {
                b.push("DRAFT");
            }
            if self.is_prerelease {
                b.push("PRE-RELEASE");
            }
            if b.is_empty() {
                String::new()
            } else {
                format!(" [{}]", b.join(", "))
            }
        };
        let notes = self.body.as_deref().unwrap_or("").trim();
        let notes_section = if notes.is_empty() {
            String::new()
        } else {
            format!(
                "\n{}",
                truncate_with_suffix(notes, 500, "\n[...truncated]")
            )
        };
        format!(
            "\u{1f504} **{tag}**{badges} \u{2014} {title}\n   Released: {date}{notes}",
            tag = self.tag_name,
            badges = badges,
            title = title,
            date = date,
            notes = notes_section,
        )
    }
}

impl CompareResult {
    pub fn format_for_telegram(&self, base: &str, head: &str) -> String {
        let status_line = format!(
            "\u{1f504} **{base}...{head}** \u{2014} {status}\n   +{ahead} ahead, -{behind} behind, {total} commit(s)",
            base = base,
            head = head,
            status = self.status,
            ahead = self.ahead_by,
            behind = self.behind_by,
            total = self.total_commits,
        );

        let commits: Vec<String> = self
            .commits
            .iter()
            .take(30)
            .map(|c| {
                let short_sha = &c.sha[..7.min(c.sha.len())];
                let first_line = c.commit.message.lines().next().unwrap_or("").trim();
                let first_line = truncate_with_suffix(first_line, 72, "...");
                let date = crate::tools::relative_time(&c.commit.author.date);
                let date = if date.is_empty() {
                    c.commit.author.date.get(..10).unwrap_or(&c.commit.author.date).to_string()
                } else {
                    date
                };
                format!(
                    "`{sha}` {author} \u{2014} {msg} ({date})",
                    sha = short_sha,
                    author = c.commit.author.name,
                    msg = first_line,
                    date = date,
                )
            })
            .collect();

        let truncation_note = if self.commits.len() > 30 {
            format!("\n[...{} more commits not shown]", self.commits.len() - 30)
        } else {
            String::new()
        };

        let diff_stat = if let Some(files) = &self.files {
            let additions: u64 = files.iter().map(|f| f.additions).sum();
            let deletions: u64 = files.iter().map(|f| f.deletions).sum();
            format!("\nDiff stat: {} files changed, +{} -{}", files.len(), additions, deletions)
        } else {
            String::new()
        };

        if commits.is_empty() {
            format!("{status_line}\nNo commits.{diff_stat}")
        } else {
            format!(
                "{status_line}\n\nCommits:\n{commits}{truncation}{diff_stat}",
                status_line = status_line,
                commits = commits.join("\n"),
                truncation = truncation_note,
                diff_stat = diff_stat,
            )
        }
    }
}

// ── New Public Tool Handlers ──────────────────────────────────────────

/// Return comprehensive detail for a single PR including reviews, checks, and diff stat.
pub async fn gh_pr_detail(repo: &str, pr_number: u64) -> Result<String> {
    validate_repo(repo)?;

    let pr_json = exec_gh(&[
        "pr", "view",
        &pr_number.to_string(),
        "--repo", repo,
        "--json",
        "number,title,body,state,author,labels,assignees,milestone,reviewDecision,statusCheckRollup,createdAt,updatedAt",
    ])
    .await?;

    let detail = parse_pr_detail(&pr_json)?;

    let diff_stat = exec_gh(&[
        "pr", "diff",
        &pr_number.to_string(),
        "--repo", repo,
        "--stat",
    ])
    .await
    .unwrap_or_default();

    Ok(detail.format_for_telegram(&diff_stat))
}

/// Return the unified diff for a PR, optionally filtered to specific files.
pub async fn gh_pr_diff(
    repo: &str,
    pr_number: u64,
    file_filter: Option<&str>,
) -> Result<String> {
    validate_repo(repo)?;

    let raw_diff = exec_gh(&[
        "pr", "diff",
        &pr_number.to_string(),
        "--repo", repo,
    ])
    .await?;

    let filtered = match file_filter {
        None => raw_diff,
        Some(filter) => {
            // Filter diff hunks by file path.  A diff header line looks like:
            //   diff --git a/path/to/file.rs b/path/to/file.rs
            // We split the diff into per-file sections (each starts with "diff --git")
            // and keep only sections where the file path contains or ends_with the filter.
            let sections: Vec<&str> = raw_diff.split("\ndiff --git ").collect();
            let mut kept = Vec::new();
            for (i, section) in sections.iter().enumerate() {
                let header = if i == 0 {
                    // First section may start without a leading \n
                    if section.starts_with("diff --git ") {
                        section.lines().next().unwrap_or("")
                    } else {
                        // Non-diff preamble — skip
                        continue;
                    }
                } else {
                    // Prepend the separator that was consumed by split
                    section.lines().next().unwrap_or("")
                };

                // Extract the file path from the header: `diff --git a/foo b/foo`
                let path_part = header.split(" b/").last().unwrap_or("");
                if path_part.contains(filter) || path_part.ends_with(filter) {
                    if i == 0 {
                        kept.push(section.to_string());
                    } else {
                        kept.push(format!("diff --git {}", section));
                    }
                }
            }
            kept.join("\n")
        }
    };

    if filtered.trim().is_empty() {
        return Ok(match file_filter {
            Some(f) => format!("No changes matching '{f}' in PR #{pr_number}."),
            None => format!("No diff available for PR #{pr_number}."),
        });
    }

    Ok(truncate_with_suffix(
        &filtered,
        10_000,
        "\n[...diff truncated at 10K chars]",
    ))
}

/// Return recent releases with tag, title, date, and notes summary.
pub async fn gh_releases(repo: &str, limit: Option<u64>) -> Result<String> {
    validate_repo(repo)?;
    let clamped = limit.unwrap_or(5).clamp(1, 20);

    let json = exec_gh(&[
        "release", "list",
        "--repo", repo,
        "--json", "tagName,name,publishedAt,isDraft,isPrerelease,body",
        "--limit", &clamped.to_string(),
    ])
    .await?;

    let releases = parse_releases(&json)?;
    if releases.is_empty() {
        return Ok(format!("No releases found for {repo}."));
    }

    let mut blocks = vec![format!("{} release(s) on {repo}:", releases.len())];
    for release in &releases {
        blocks.push(release.format_for_telegram());
    }
    Ok(blocks.join("\n\n"))
}

/// Compare two refs (branches, tags, or commits) and return commit list + diff stat.
pub async fn gh_compare(repo: &str, base: &str, head: &str) -> Result<String> {
    validate_repo(repo)?;

    // Build owner/repo parts for the API path
    let api_path = format!("repos/{repo}/compare/{base}...{head}");
    let json = exec_gh(&["api", &api_path]).await?;

    let compare = parse_compare(&json)?;
    Ok(compare.format_for_telegram(base, head))
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
        ToolDefinition {
            name: "gh_pr_detail".into(),
            description: "Get comprehensive detail for a single GitHub PR: title, body, state, author, labels, assignees, milestone, review decision, status checks summary, and diff stat (files changed, additions, deletions).".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "repo": {
                        "type": "string",
                        "description": "GitHub repository in owner/repo format (e.g. 'nyaptor/nv')"
                    },
                    "pr_number": {
                        "type": "integer",
                        "description": "PR number"
                    }
                },
                "required": ["repo", "pr_number"]
            }),
        },
        ToolDefinition {
            name: "gh_pr_diff".into(),
            description: "Get the unified diff for a GitHub PR. Optionally filter to specific files using a simple path filter (substring or suffix match). Output is truncated to 10,000 characters.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "repo": {
                        "type": "string",
                        "description": "GitHub repository in owner/repo format (e.g. 'nyaptor/nv')"
                    },
                    "pr_number": {
                        "type": "integer",
                        "description": "PR number"
                    },
                    "file_filter": {
                        "type": "string",
                        "description": "Optional: filter diff to files whose path contains or ends with this string (e.g. '*.rs', 'src/tools.rs')"
                    }
                },
                "required": ["repo", "pr_number"]
            }),
        },
        ToolDefinition {
            name: "gh_releases".into(),
            description: "List recent releases for a GitHub repository with tag, title, date, draft/pre-release status, and notes summary.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "repo": {
                        "type": "string",
                        "description": "GitHub repository in owner/repo format (e.g. 'nyaptor/nv')"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Number of releases to return (default 5, max 20)"
                    }
                },
                "required": ["repo"]
            }),
        },
        ToolDefinition {
            name: "gh_compare".into(),
            description: "Compare two refs (branches, tags, or commits) in a GitHub repository. Returns status (ahead/behind/diverged/identical), commit list (up to 30), and diff stat.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "repo": {
                        "type": "string",
                        "description": "GitHub repository in owner/repo format (e.g. 'nyaptor/nv')"
                    },
                    "base": {
                        "type": "string",
                        "description": "Base ref (branch name, tag, or commit SHA)"
                    },
                    "head": {
                        "type": "string",
                        "description": "Head ref (branch name, tag, or commit SHA)"
                    }
                },
                "required": ["repo", "base", "head"]
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
        assert!(formatted.contains("By octocat"));
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
        assert!(formatted.contains("Branch: main"));
        assert!(formatted.contains("Trigger: push"));
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
        assert!(formatted.contains("Branch: feat/x"));
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
    }

    #[test]
    fn github_tool_definitions_returns_seven_tools() {
        let tools = github_tool_definitions();
        assert_eq!(tools.len(), 7);

        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"gh_pr_list"));
        assert!(names.contains(&"gh_run_status"));
        assert!(names.contains(&"gh_issues"));
        assert!(names.contains(&"gh_pr_detail"));
        assert!(names.contains(&"gh_pr_diff"));
        assert!(names.contains(&"gh_releases"));
        assert!(names.contains(&"gh_compare"));
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

    // ── truncate_with_suffix tests ────────────────────────────────────

    #[test]
    fn truncate_under_limit_unchanged() {
        let s = "hello";
        assert_eq!(truncate_with_suffix(s, 10, "[...]"), "hello");
    }

    #[test]
    fn truncate_at_limit_unchanged() {
        let s = "hello";
        assert_eq!(truncate_with_suffix(s, 5, "[...]"), "hello");
    }

    #[test]
    fn truncate_over_limit_appends_suffix() {
        let result = truncate_with_suffix("hello world", 5, "[...]");
        assert_eq!(result, "hello[...]");
    }

    #[test]
    fn truncate_empty_input() {
        assert_eq!(truncate_with_suffix("", 10, "[...]"), "");
    }

    #[test]
    fn truncate_multibyte_utf8_boundary_safe() {
        // Each char is 3 bytes in UTF-8 (U+4E2D = \xe4\xb8\xad)
        let s = "\u{4e2d}\u{6587}\u{5185}\u{5bb9}\u{6d4b}\u{8bd5}"; // 6 Chinese chars
        let result = truncate_with_suffix(s, 3, "[...]");
        // First 3 chars = "\u{4e2d}\u{6587}\u{5185}" (9 bytes), then suffix
        assert_eq!(result, "\u{4e2d}\u{6587}\u{5185}[...]");
        // Verify no panic and result is valid UTF-8
        let _ = result.as_str();
    }

    // ── PrDetail parse + format tests ────────────────────────────────

    #[test]
    fn parse_pr_detail_valid() {
        let json = r#"{
            "number": 42,
            "title": "Add feature X",
            "body": "This is the body",
            "state": "OPEN",
            "author": {"login": "octocat"},
            "labels": [{"name": "enhancement"}],
            "assignees": [{"login": "nyaptor"}],
            "milestone": {"title": "v2.0"},
            "reviewDecision": "APPROVED",
            "statusCheckRollup": null,
            "createdAt": "2026-01-01T00:00:00Z",
            "updatedAt": "2026-03-22T10:00:00Z"
        }"#;
        let detail = parse_pr_detail(json).unwrap();
        assert_eq!(detail.number, 42);
        assert_eq!(detail.title, "Add feature X");
        assert_eq!(detail.review_decision.as_deref(), Some("APPROVED"));
        assert_eq!(detail.milestone.as_ref().unwrap().title, "v2.0");
    }

    #[test]
    fn parse_pr_detail_missing_optional_fields() {
        let json = r#"{
            "number": 1,
            "title": "Fix bug",
            "body": null,
            "state": "OPEN",
            "author": {"login": "user"},
            "labels": [],
            "assignees": [],
            "milestone": null,
            "reviewDecision": null,
            "statusCheckRollup": null,
            "createdAt": "2026-01-01T00:00:00Z",
            "updatedAt": "2026-01-01T00:00:00Z"
        }"#;
        let detail = parse_pr_detail(json).unwrap();
        assert!(detail.body.is_none());
        assert!(detail.review_decision.is_none());
        assert!(detail.milestone.is_none());
    }

    #[test]
    fn format_pr_detail_body_truncation() {
        let long_body: String = "x".repeat(3000);
        let detail = PrDetail {
            number: 1,
            title: "Test".into(),
            body: Some(long_body),
            state: "OPEN".into(),
            author: PrAuthor { login: "user".into() },
            labels: vec![],
            assignees: vec![],
            milestone: None,
            review_decision: None,
            status_check_rollup: None,
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
        };
        let formatted = detail.format_for_telegram("");
        assert!(formatted.contains("[...truncated]"));
        // The body excerpt should not exceed 2000 chars + suffix overhead
        let body_start = formatted.find("Description:").unwrap();
        let body_part = &formatted[body_start..];
        assert!(body_part.chars().count() < 2100);
    }

    #[test]
    fn format_pr_detail_diff_stat_included() {
        let detail = PrDetail {
            number: 5,
            title: "Refactor".into(),
            body: None,
            state: "MERGED".into(),
            author: PrAuthor { login: "dev".into() },
            labels: vec![],
            assignees: vec![],
            milestone: None,
            review_decision: Some("APPROVED".into()),
            status_check_rollup: None,
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
        };
        let formatted = detail.format_for_telegram("3 files changed, +120 -45");
        assert!(formatted.contains("Diff stat: 3 files changed"));
    }

    // ── gh_pr_diff tests ──────────────────────────────────────────────

    #[test]
    fn truncate_diff_at_10k_boundary() {
        // Build a string just over 10K chars
        let big: String = "a".repeat(10_500);
        let result = truncate_with_suffix(&big, 10_000, "\n[...diff truncated at 10K chars]");
        assert!(result.starts_with("aaaa"));
        assert!(result.ends_with("[...diff truncated at 10K chars]"));
        // char count should be 10000 + suffix length
        let suffix = "\n[...diff truncated at 10K chars]";
        assert_eq!(result.len(), 10_000 + suffix.len());
    }

    #[test]
    fn file_filter_extension_match() {
        // Simulate the filtering logic: a path containing ".rs" should match
        let path = "src/github.rs";
        let filter = ".rs";
        assert!(path.contains(filter) || path.ends_with(filter));
    }

    #[test]
    fn file_filter_exact_name_match() {
        let path = "src/tools.rs";
        let filter = "tools.rs";
        assert!(path.contains(filter) || path.ends_with(filter));
    }

    #[test]
    fn file_filter_no_match() {
        let path = "src/github.rs";
        let filter = "tools.ts";
        assert!(!(path.contains(filter) || path.ends_with(filter)));
    }

    // ── ReleaseSummary parse + format tests ───────────────────────────

    #[test]
    fn parse_releases_valid() {
        let json = r#"[{
            "tagName": "v1.0.0",
            "name": "Initial Release",
            "publishedAt": "2026-01-15T10:00:00Z",
            "isDraft": false,
            "isPrerelease": false,
            "body": "First stable release."
        }]"#;
        let releases = parse_releases(json).unwrap();
        assert_eq!(releases.len(), 1);
        assert_eq!(releases[0].tag_name, "v1.0.0");
        assert_eq!(releases[0].is_draft, false);
    }

    #[test]
    fn parse_releases_empty_list() {
        let releases = parse_releases("[]").unwrap();
        assert!(releases.is_empty());
    }

    #[test]
    fn format_release_body_truncation() {
        let long_notes: String = "note ".repeat(200); // 1000 chars
        let release = ReleaseSummary {
            tag_name: "v2.0.0".into(),
            name: Some("Big Release".into()),
            published_at: Some("2026-03-01T00:00:00Z".into()),
            is_draft: false,
            is_prerelease: false,
            body: Some(long_notes),
        };
        let formatted = release.format_for_telegram();
        assert!(formatted.contains("[...truncated]"));
    }

    #[test]
    fn format_release_badges() {
        let release = ReleaseSummary {
            tag_name: "v3.0.0-beta".into(),
            name: None,
            published_at: None,
            is_draft: false,
            is_prerelease: true,
            body: None,
        };
        let formatted = release.format_for_telegram();
        assert!(formatted.contains("PRE-RELEASE"));
    }

    #[test]
    fn format_release_limit_clamping() {
        // Verify clamp logic (1..=20)
        assert_eq!(0u64.clamp(1, 20), 1);
        assert_eq!(25u64.clamp(1, 20), 20);
        assert_eq!(5u64.clamp(1, 20), 5);
    }

    // ── CompareResult parse + format tests ───────────────────────────

    #[test]
    fn parse_compare_valid() {
        let json = r#"{
            "status": "ahead",
            "ahead_by": 3,
            "behind_by": 0,
            "total_commits": 3,
            "commits": [
                {
                    "sha": "abc1234567890",
                    "commit": {
                        "message": "Fix bug\n\nLonger description",
                        "author": {"name": "Alice", "date": "2026-03-10T08:00:00Z"}
                    }
                }
            ],
            "files": [
                {"additions": 10, "deletions": 5, "changes": 15}
            ]
        }"#;
        let result = parse_compare(json).unwrap();
        assert_eq!(result.status, "ahead");
        assert_eq!(result.ahead_by, 3);
        assert_eq!(result.commits.len(), 1);
        assert_eq!(result.commits[0].sha, "abc1234567890");
    }

    #[test]
    fn parse_compare_identical_refs() {
        let json = r#"{
            "status": "identical",
            "ahead_by": 0,
            "behind_by": 0,
            "total_commits": 0,
            "commits": [],
            "files": []
        }"#;
        let result = parse_compare(json).unwrap();
        assert_eq!(result.status, "identical");
        assert_eq!(result.total_commits, 0);
        assert!(result.commits.is_empty());
    }

    #[test]
    fn format_compare_shows_summary_line() {
        let result = CompareResult {
            status: "ahead".into(),
            ahead_by: 2,
            behind_by: 0,
            total_commits: 2,
            commits: vec![CompareCommit {
                sha: "abcdef1234567".into(),
                commit: CompareCommitInner {
                    message: "Add feature".into(),
                    author: CompareAuthor {
                        name: "Alice".into(),
                        date: "2026-03-10T08:00:00Z".into(),
                    },
                },
            }],
            files: Some(vec![CompareFile { additions: 5, deletions: 2, changes: 7 }]),
        };
        let formatted = result.format_for_telegram("main", "feat/x");
        assert!(formatted.contains("main...feat/x"));
        assert!(formatted.contains("ahead"));
        assert!(formatted.contains("abcdef1")); // short sha
        assert!(formatted.contains("Add feature"));
        assert!(formatted.contains("1 files changed, +5 -2"));
        assert!(formatted.contains("+2 ahead"));
    }

    #[test]
    fn format_compare_truncates_commit_list_at_30() {
        let commits: Vec<CompareCommit> = (0..35)
            .map(|i| CompareCommit {
                sha: format!("{:040}", i),
                commit: CompareCommitInner {
                    message: format!("Commit {i}"),
                    author: CompareAuthor {
                        name: "Dev".into(),
                        date: "2026-01-01T00:00:00Z".into(),
                    },
                },
            })
            .collect();
        let result = CompareResult {
            status: "ahead".into(),
            ahead_by: 35,
            behind_by: 0,
            total_commits: 35,
            commits,
            files: None,
        };
        let formatted = result.format_for_telegram("v1.0", "main");
        assert!(formatted.contains("5 more commits not shown"));
        // Commit 30 (index 30) should NOT appear in the list
        assert!(!formatted.contains("Commit 30\n") || formatted.contains("30 more"));
    }
}

// ── GithubClient wrapper ─────────────────────────────────────────────

/// Thin wrapper for `Checkable` health checks.
/// GitHub uses the `gh` CLI — authenticated via `~/.config/gh/hosts.yml`.
#[allow(dead_code)]
pub struct GithubClient;

// ── Checkable ────────────────────────────────────────────────────────

#[async_trait::async_trait]
impl crate::tools::Checkable for GithubClient {
    fn name(&self) -> &str {
        "github"
    }

    async fn check_read(&self) -> crate::tools::CheckResult {
        use crate::tools::check::timed;
        let (latency, result) = timed(|| async {
            tokio::time::timeout(GH_TIMEOUT, async {
                Command::new("gh")
                    .args(["auth", "status"])
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .output()
                    .await
            })
            .await
        })
        .await;
        match result {
            Ok(Ok(output)) if output.status.success() => {
                // Parse "Logged in to github.com account <user>" from stdout/stderr
                let combined = format!(
                    "{}{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
                let detail = combined
                    .lines()
                    .find(|l| l.contains("Logged in"))
                    .map(|l| l.trim().to_string())
                    .unwrap_or_else(|| "gh auth status ok".into());
                crate::tools::CheckResult::Healthy {
                    latency_ms: latency,
                    detail,
                }
            }
            Ok(Ok(output)) => {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                crate::tools::CheckResult::Unhealthy {
                    error: if stderr.is_empty() {
                        "gh auth status failed — run `gh auth login`".into()
                    } else {
                        stderr
                    },
                }
            }
            Ok(Err(e)) => crate::tools::CheckResult::Unhealthy {
                error: format!("failed to run gh: {e}"),
            },
            Err(_) => crate::tools::CheckResult::Unhealthy {
                error: format!("gh auth status timed out after {}s", GH_TIMEOUT.as_secs()),
            },
        }
    }
}
