//! Scoped Bash Toolkit — allowlisted read-only shell commands per project.
//!
//! Executes git, ls, cat, and bd commands via `tokio::process::Command::new()`
//! with strict project path scoping and argument validation. No shell invocation.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use tokio::process::Command;

/// Maximum number of git log entries.
const MAX_GIT_LOG_COUNT: u64 = 20;

/// Execution timeout for all commands.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(5);

/// File extensions allowed for `cat_config`.
const ALLOWED_EXTENSIONS: &[&str] = &["json", "toml", "yaml", "yml", "md"];

// ── Allowed Commands ────────────────────────────────────────────────

/// Enumeration of every command the toolkit can execute.
#[derive(Debug, Clone)]
pub enum AllowedCommand {
    GitStatus,
    GitLog { count: u64 },
    GitBranch,
    GitDiffStat,
    LsDir { subdir: Option<String> },
    CatConfig { file: String },
    BdReady,
    BdStats,
}

// ── Validation ──────────────────────────────────────────────────────

/// Look up a project code in the registry and return its validated path.
pub fn validate_project<'a>(
    project: &str,
    registry: &'a HashMap<String, PathBuf>,
) -> Result<&'a PathBuf> {
    let path = registry
        .get(project)
        .ok_or_else(|| anyhow!("unknown project code: '{project}'"))?;
    if !path.is_dir() {
        bail!(
            "project '{project}' path does not exist: {}",
            path.display()
        );
    }
    Ok(path)
}

/// Validate a subdirectory argument: reject `..` components and verify
/// the resolved path stays within the project root.
pub fn validate_subdir(project_root: &Path, subdir: &str) -> Result<PathBuf> {
    if subdir.contains("..") {
        bail!("path traversal not allowed: '{subdir}'");
    }
    let candidate = project_root.join(subdir);
    // Canonicalize both to resolve symlinks.
    let canon_root = project_root.canonicalize().map_err(|e| {
        anyhow!(
            "cannot canonicalize project root {}: {e}",
            project_root.display()
        )
    })?;
    let canon_candidate = candidate.canonicalize().map_err(|e| {
        anyhow!(
            "cannot canonicalize subdir path {}: {e}",
            candidate.display()
        )
    })?;
    if !canon_candidate.starts_with(&canon_root) {
        bail!(
            "resolved path escapes project root: {}",
            canon_candidate.display()
        );
    }
    Ok(canon_candidate)
}

/// Validate a config file path: check extension allowlist, reject `..`.
pub fn validate_config_file(project_root: &Path, file: &str) -> Result<PathBuf> {
    if file.contains("..") {
        bail!("path traversal not allowed: '{file}'");
    }
    let path = Path::new(file);
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    if !ALLOWED_EXTENSIONS.contains(&ext) {
        bail!(
            "file extension '.{ext}' not allowed — permitted: {}",
            ALLOWED_EXTENSIONS.join(", ")
        );
    }
    let full = project_root.join(file);
    if !full.exists() {
        bail!("file does not exist: {}", full.display());
    }
    Ok(full)
}

// ── Execution ───────────────────────────────────────────────────────

/// Execute an allowed command against a validated project path.
///
/// Returns the captured stdout on success, or an error with stderr/exit info.
pub async fn execute_command(
    cmd: &AllowedCommand,
    project_root: &Path,
) -> Result<String> {
    let mut process = match cmd {
        AllowedCommand::GitStatus => {
            let mut c = Command::new("git");
            c.arg("-C")
                .arg(project_root)
                .arg("status")
                .arg("--short");
            c
        }
        AllowedCommand::GitLog { count } => {
            let n = (*count).min(MAX_GIT_LOG_COUNT);
            let mut c = Command::new("git");
            c.arg("-C")
                .arg(project_root)
                .arg("log")
                .arg("--oneline")
                .arg(format!("-{n}"));
            c
        }
        AllowedCommand::GitBranch => {
            let mut c = Command::new("git");
            c.arg("-C")
                .arg(project_root)
                .arg("branch")
                .arg("--show-current");
            c
        }
        AllowedCommand::GitDiffStat => {
            let mut c = Command::new("git");
            c.arg("-C")
                .arg(project_root)
                .arg("diff")
                .arg("--stat");
            c
        }
        AllowedCommand::LsDir { subdir } => {
            let target = match subdir {
                Some(s) => validate_subdir(project_root, s)?,
                None => project_root.to_path_buf(),
            };
            let mut c = Command::new("ls");
            c.arg("-1").arg(&target);
            c
        }
        AllowedCommand::CatConfig { file } => {
            let full_path = validate_config_file(project_root, file)?;
            let mut c = Command::new("cat");
            c.arg(&full_path);
            c
        }
        AllowedCommand::BdReady => {
            let mut c = Command::new("bd");
            c.arg("-C").arg(project_root).arg("ready").arg("--json");
            c
        }
        AllowedCommand::BdStats => {
            let mut c = Command::new("bd");
            c.arg("-C").arg(project_root).arg("stats").arg("--json");
            c
        }
    };

    // No shell invocation — stdin closed, stdout/stderr captured.
    process
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let output = tokio::time::timeout(COMMAND_TIMEOUT, process.output())
        .await
        .map_err(|_| anyhow!("command timed out after {}s", COMMAND_TIMEOUT.as_secs()))?
        .map_err(|e| anyhow!("failed to execute command: {e}"))?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        if stdout.trim().is_empty() {
            Ok("(no output)".to_string())
        } else {
            Ok(stdout)
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let code = output.status.code().unwrap_or(-1);
        Err(anyhow!(
            "command failed (exit {code}): {}",
            stderr.trim()
        ))
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_registry(pairs: &[(&str, &Path)]) -> HashMap<String, PathBuf> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_path_buf()))
            .collect()
    }

    // ── validate_project ────────────────────────────────────────────

    #[test]
    fn validate_project_known() {
        let tmp = tempfile::tempdir().unwrap();
        let reg = make_registry(&[("oo", tmp.path())]);
        assert!(validate_project("oo", &reg).is_ok());
    }

    #[test]
    fn validate_project_unknown() {
        let reg: HashMap<String, PathBuf> = HashMap::new();
        let err = validate_project("oo", &reg).unwrap_err();
        assert!(err.to_string().contains("unknown project code"));
    }

    #[test]
    fn validate_project_nonexistent_path() {
        let reg = make_registry(&[("oo", Path::new("/tmp/nv-nonexistent-path-xyz"))]);
        let err = validate_project("oo", &reg).unwrap_err();
        assert!(err.to_string().contains("does not exist"));
    }

    // ── validate_subdir ─────────────────────────────────────────────

    #[test]
    fn validate_subdir_rejects_dotdot() {
        let tmp = tempfile::tempdir().unwrap();
        let err = validate_subdir(tmp.path(), "../etc").unwrap_err();
        assert!(err.to_string().contains("path traversal"));
    }

    #[test]
    fn validate_subdir_accepts_valid() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir(tmp.path().join("src")).unwrap();
        let result = validate_subdir(tmp.path(), "src");
        assert!(result.is_ok());
    }

    #[test]
    fn validate_subdir_rejects_nonexistent() {
        let tmp = tempfile::tempdir().unwrap();
        let err = validate_subdir(tmp.path(), "nope").unwrap_err();
        assert!(err.to_string().contains("cannot canonicalize"));
    }

    // ── validate_config_file ────────────────────────────────────────

    #[test]
    fn validate_config_file_accepts_allowed_extensions() {
        let tmp = tempfile::tempdir().unwrap();
        for ext in ALLOWED_EXTENSIONS {
            let name = format!("config.{ext}");
            std::fs::write(tmp.path().join(&name), "content").unwrap();
            assert!(
                validate_config_file(tmp.path(), &name).is_ok(),
                "should accept .{ext}"
            );
        }
    }

    #[test]
    fn validate_config_file_rejects_disallowed_extension() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("script.sh"), "#!/bin/sh").unwrap();
        let err = validate_config_file(tmp.path(), "script.sh").unwrap_err();
        assert!(err.to_string().contains("not allowed"));
    }

    #[test]
    fn validate_config_file_rejects_rs_extension() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("main.rs"), "fn main() {}").unwrap();
        let err = validate_config_file(tmp.path(), "main.rs").unwrap_err();
        assert!(err.to_string().contains("not allowed"));
    }

    #[test]
    fn validate_config_file_rejects_dotdot() {
        let tmp = tempfile::tempdir().unwrap();
        let err = validate_config_file(tmp.path(), "../etc/passwd").unwrap_err();
        assert!(err.to_string().contains("path traversal"));
    }

    #[test]
    fn validate_config_file_rejects_nonexistent() {
        let tmp = tempfile::tempdir().unwrap();
        let err = validate_config_file(tmp.path(), "missing.json").unwrap_err();
        assert!(err.to_string().contains("does not exist"));
    }

    // ── execute_command ─────────────────────────────────────────────

    #[tokio::test]
    async fn execute_git_status_on_repo() {
        // Use the nv project itself as a known git repo.
        let nv_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let result = execute_command(&AllowedCommand::GitStatus, &nv_root).await;
        // Should succeed (even if clean — returns "(no output)" or short status).
        assert!(result.is_ok(), "git status failed: {:?}", result);
    }

    #[tokio::test]
    async fn execute_git_branch_on_repo() {
        let nv_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let result = execute_command(&AllowedCommand::GitBranch, &nv_root)
            .await
            .unwrap();
        // Should return a branch name (e.g. "main").
        assert!(!result.trim().is_empty() || result == "(no output)");
    }

    #[tokio::test]
    async fn execute_git_log_capped_at_max() {
        let nv_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        // Request 100, should be capped to MAX_GIT_LOG_COUNT.
        let result = execute_command(
            &AllowedCommand::GitLog { count: 100 },
            &nv_root,
        )
        .await
        .unwrap();
        let lines: Vec<&str> = result.trim().lines().collect();
        assert!(lines.len() <= MAX_GIT_LOG_COUNT as usize);
    }

    #[tokio::test]
    async fn execute_ls_project_root() {
        let nv_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let result = execute_command(
            &AllowedCommand::LsDir { subdir: None },
            &nv_root,
        )
        .await
        .unwrap();
        assert!(result.contains("Cargo.toml") || result.contains("crates"));
    }

    #[tokio::test]
    async fn execute_cat_config_reads_file() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("test.json"), r#"{"key":"val"}"#).unwrap();
        let result = execute_command(
            &AllowedCommand::CatConfig {
                file: "test.json".into(),
            },
            tmp.path(),
        )
        .await
        .unwrap();
        assert!(result.contains("key"));
    }
}
