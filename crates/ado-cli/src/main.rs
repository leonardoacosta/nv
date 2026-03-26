//! Azure DevOps CLI (`ado`) — thin wrapper over the `nv-tools` ADO client.
//!
//! Usage:
//!   ado pipelines <project> [--json]
//!   ado builds <project> [--json]
//!   ado work-items <project> [--assigned-to <identity>] [--json]
//!   ado run-pipeline <project> <pipeline-id> [--json]
//!
//! Auth: Set `ADO_ORG` and `ADO_PAT` environment variables (inject via Doppler).

use anyhow::Result;
use clap::{Parser, Subcommand};

use nv_tools::tools::ado::{
    AdoBuildWithPipeline, AdoClient, AdoPipeline, AdoWorkItem, build_wiql,
};
use nv_tools::tools::relative_time;

// ── CLI Definition ────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "ado",
    about = "Azure DevOps CLI — manage pipelines, builds, and work items",
    long_about = "Requires ADO_ORG and ADO_PAT environment variables.\n\
                  Inject secrets via: doppler run -- ado <command>"
)]
struct Cli {
    /// Output raw JSON instead of human-readable tables
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List pipeline definitions for a project
    Pipelines {
        /// Azure DevOps project name
        project: String,
    },
    /// List the 20 most recent builds for a project
    Builds {
        /// Azure DevOps project name
        project: String,
    },
    /// Query active work items in a project
    WorkItems {
        /// Azure DevOps project name
        project: String,
        /// Filter by assignee — use '@Me' for the authenticated PAT user
        #[arg(long, default_value = "@Me")]
        assigned_to: String,
    },
    /// Trigger a pipeline run
    RunPipeline {
        /// Azure DevOps project name
        project: String,
        /// Pipeline definition ID (from `ado pipelines`)
        pipeline_id: u32,
    },
}

// ── Entry Point ───────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli).await {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<()> {
    let client = AdoClient::from_env()?;

    match cli.command {
        Commands::Pipelines { project } => {
            let pipelines = client.pipelines(&project).await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&pipelines)?);
            } else {
                print_pipelines(&project, &pipelines);
            }
        }

        Commands::Builds { project } => {
            let builds = client.builds_all(&project, 20).await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&builds)?);
            } else {
                print_builds(&project, &builds);
            }
        }

        Commands::WorkItems {
            project,
            assigned_to,
        } => {
            let wiql = build_wiql(&project, &assigned_to, "active");
            let items = client.work_items_by_wiql(&project, &wiql, 50).await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&items)?);
            } else {
                print_work_items(&project, &items);
            }
        }

        Commands::RunPipeline {
            project,
            pipeline_id,
        } => {
            let run = client.run_pipeline(&project, pipeline_id).await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&run)?);
            } else {
                let url = run
                    .links
                    .as_ref()
                    .and_then(|l| l.web.as_ref())
                    .and_then(|w| w.href.as_deref())
                    .unwrap_or("(url unavailable)");
                println!("Run #{} queued: {}", run.id, url);
            }
        }
    }

    Ok(())
}

// ── Table Formatters ─────────────────────────────────────────────────

fn print_pipelines(project: &str, pipelines: &[AdoPipeline]) {
    if pipelines.is_empty() {
        println!("No pipelines found for {project}.");
        return;
    }
    println!("{:<8} {:<40} {}", "ID", "Name", "Folder");
    println!("{}", "-".repeat(72));
    for p in pipelines {
        let folder = p.folder.as_deref().unwrap_or("/");
        println!("{:<8} {:<40} {}", p.id, p.name, folder);
    }
}

fn print_builds(project: &str, builds: &[AdoBuildWithPipeline]) {
    if builds.is_empty() {
        println!("No recent builds found for {project}.");
        return;
    }
    println!(
        "{:<16} {:<30} {:<12} {:<10} {:<28} {:<20} {}",
        "Build#", "Pipeline", "Status", "Result", "Branch", "Requester", "Queued"
    );
    println!("{}", "-".repeat(120));
    for b in builds {
        let number = b.build_number.as_deref().unwrap_or("?");
        let pipeline = b
            .definition
            .as_ref()
            .and_then(|d| d.name.as_deref())
            .unwrap_or("?");
        let status = b.status.as_deref().unwrap_or("unknown");
        let result = b.result.as_deref().unwrap_or("-");
        let branch = b
            .source_branch
            .as_deref()
            .unwrap_or("?")
            .trim_start_matches("refs/heads/");
        let requester = b
            .requested_for
            .as_ref()
            .and_then(|r| r.display_name.as_deref())
            .unwrap_or("unknown");
        let queued = b
            .queue_time
            .as_deref()
            .map(relative_time)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "-".to_string());

        println!(
            "{:<16} {:<30} {:<12} {:<10} {:<28} {:<20} {}",
            number,
            truncate(pipeline, 29),
            status,
            result,
            truncate(branch, 27),
            truncate(requester, 19),
            queued
        );
    }
}

fn print_work_items(project: &str, items: &[AdoWorkItem]) {
    if items.is_empty() {
        println!("No active work items found for {project}.");
        return;
    }
    println!(
        "{:<8} {:<12} {:<12} {:<50} {:<24} {}",
        "ID", "Type", "State", "Title", "Assignee", "Changed"
    );
    println!("{}", "-".repeat(120));
    for item in items {
        let id = item.fields.system_id.unwrap_or(item.id);
        let work_type = item
            .fields
            .system_work_item_type
            .as_deref()
            .unwrap_or("Item");
        let state = item.fields.system_state.as_deref().unwrap_or("Unknown");
        let title = item.fields.system_title.as_deref().unwrap_or("(no title)");
        let assignee = item
            .fields
            .system_assigned_to
            .as_ref()
            .and_then(|a| a.display_name.as_deref())
            .unwrap_or("Unassigned");
        let changed = item
            .fields
            .system_changed_date
            .as_deref()
            .map(relative_time)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "-".to_string());

        println!(
            "{:<8} {:<12} {:<12} {:<50} {:<24} {}",
            id,
            truncate(work_type, 11),
            truncate(state, 11),
            truncate(title, 49),
            truncate(assignee, 23),
            changed
        );
    }
}

/// Truncate a string to at most `max` characters, appending `…` if clipped.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{truncated}…")
    }
}
