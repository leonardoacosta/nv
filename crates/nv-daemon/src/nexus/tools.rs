use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;

use crate::bash;

/// Format the `bd ready` output for a project via scoped bash.
///
/// Used by the `nexus_project_ready` tool.
pub async fn format_project_ready(
    project_code: &str,
    project_registry: &HashMap<String, PathBuf>,
) -> Result<String> {
    let project_path = bash::validate_project(project_code, project_registry)?;
    let output = bash::execute_command(
        &bash::AllowedCommand::BdReady,
        project_path,
    )
    .await?;

    if output.trim().is_empty() {
        Ok(format!("[{project_code}] No issues ready for work."))
    } else {
        Ok(format!("[{project_code}] Ready queue:\n{output}"))
    }
}

/// List open proposals in `openspec/changes/` for a project.
///
/// Used by the `nexus_project_proposals` tool.
pub async fn format_project_proposals(
    project_code: &str,
    project_registry: &HashMap<String, PathBuf>,
) -> Result<String> {
    let project_path = bash::validate_project(project_code, project_registry)?;
    let changes_dir = project_path.join("openspec").join("changes");

    if !changes_dir.is_dir() {
        return Ok(format!(
            "[{project_code}] No openspec/changes/ directory found."
        ));
    }

    let mut proposals = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(&changes_dir)?
        .filter_map(|e| e.ok())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let name = entry.file_name().to_string_lossy().to_string();
        // Skip archive directory
        if name == "archive" {
            continue;
        }
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        // Check for proposal.md and tasks.md
        let has_proposal = path.join("proposal.md").exists();
        let has_tasks = path.join("tasks.md").exists();

        let status = if has_proposal && has_tasks {
            "ready"
        } else if has_proposal {
            "proposal only"
        } else {
            "incomplete"
        };

        proposals.push(format!("  {name} ({status})"));
    }

    if proposals.is_empty() {
        Ok(format!("[{project_code}] No open proposals."))
    } else {
        Ok(format!(
            "[{project_code}] {} proposal(s):\n{}",
            proposals.len(),
            proposals.join("\n")
        ))
    }
}
