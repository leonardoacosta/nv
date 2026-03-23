//! Aggregation layer — composite tools that call individual data sources in parallel.
//!
//! Three tools:
//! * `project_health(code)` — Vercel + Sentry + Jira + Nexus + GitHub CI status.
//! * `homelab_status()` — Docker + Tailscale + Home Assistant.
//! * `financial_summary()` — Plaid + Stripe.
//!
//! Each sub-call has an independent 5-second timeout. Partial failures are tolerated:
//! failed sources show as "unavailable", not errors.

use std::collections::HashMap;
use std::time::Duration;

use anyhow::Result;

use crate::claude::ToolDefinition;
use crate::tools::docker as docker_tools;
use crate::tools::github;
use crate::tools::ha as ha_tools;
use crate::tools::jira;
use crate::nexus;
use crate::tools::plaid as plaid_tools;
use crate::tools::sentry as sentry_tools;
use crate::tools::stripe as stripe_tools;
use crate::tailscale;
use crate::tools::vercel as vercel_tools;

/// Timeout for each individual data source call.
const SOURCE_TIMEOUT: Duration = Duration::from_secs(5);

// ── Project-Resource Mapping ────────────────────────────────────────

/// Resources associated with a project across external services.
struct ProjectResources {
    vercel_project: Option<&'static str>,
    sentry_slug: Option<&'static str>,
    jira_key: Option<&'static str>,
    github_repo: Option<&'static str>,
}

/// Hardcoded mapping of project codes to their external resource identifiers.
fn project_map() -> HashMap<&'static str, ProjectResources> {
    let mut m = HashMap::new();
    m.insert(
        "oo",
        ProjectResources {
            vercel_project: Some("otaku-odyssey"),
            sentry_slug: Some("otaku-odyssey"),
            jira_key: Some("OO"),
            github_repo: Some("leonardoacosta/otaku-odyssey"),
        },
    );
    m.insert(
        "tc",
        ProjectResources {
            vercel_project: Some("tribal-cities"),
            sentry_slug: Some("tribal-cities"),
            jira_key: Some("TC"),
            github_repo: Some("leonardoacosta/tribal-cities"),
        },
    );
    m.insert(
        "tl",
        ProjectResources {
            vercel_project: Some("tavern-ledger"),
            sentry_slug: Some("tavern-ledger"),
            jira_key: Some("TL"),
            github_repo: Some("leonardoacosta/tavern-ledger"),
        },
    );
    m.insert(
        "mv",
        ProjectResources {
            vercel_project: Some("modern-visa"),
            sentry_slug: Some("modern-visa"),
            jira_key: Some("MV"),
            github_repo: Some("leonardoacosta/modern-visa"),
        },
    );
    m.insert(
        "ss",
        ProjectResources {
            vercel_project: Some("styles-silas"),
            sentry_slug: Some("styles-silas"),
            jira_key: Some("SS"),
            github_repo: Some("leonardoacosta/styles-silas"),
        },
    );
    m.insert(
        "cl",
        ProjectResources {
            vercel_project: None,
            sentry_slug: None,
            jira_key: Some("CL"),
            github_repo: Some("leonardoacosta/central-leo"),
        },
    );
    m.insert(
        "co",
        ProjectResources {
            vercel_project: None,
            sentry_slug: None,
            jira_key: Some("CO"),
            github_repo: Some("leonardoacosta/central-wholesale"),
        },
    );
    m.insert(
        "cw",
        ProjectResources {
            vercel_project: None,
            sentry_slug: None,
            jira_key: Some("CW"),
            github_repo: Some("leonardoacosta/central-wholesale"),
        },
    );
    m.insert(
        "nv",
        ProjectResources {
            vercel_project: None,
            sentry_slug: None,
            jira_key: None,
            github_repo: Some("leonardoacosta/nv"),
        },
    );
    m
}

// ── Tool Definitions ────────────────────────────────────────────────

/// Return tool definitions for the 3 aggregation tools.
pub fn aggregation_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "project_health".into(),
            description: "Comprehensive health check for a single project. Calls Vercel, Sentry, Jira, Nexus, and GitHub CI in parallel and returns a unified status summary.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "code": {
                        "type": "string",
                        "description": "Project code (e.g. 'oo', 'tc', 'tl', 'mv', 'ss', 'cl', 'co', 'nv')"
                    }
                },
                "required": ["code"]
            }),
        },
        ToolDefinition {
            name: "homelab_status".into(),
            description: "Health check for homelab infrastructure. Calls Docker, Tailscale, and Home Assistant in parallel and returns a unified status summary.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "financial_summary".into(),
            description: "Financial overview combining Plaid account balances and Stripe open invoices in parallel.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
    ]
}

// ── Source Result Helper ────────────────────────────────────────────

/// Wrapper for a source call result: either the string output or "unavailable".
async fn timed_call<F, Fut>(label: &str, f: F) -> (String, String)
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<String>>,
{
    let name = label.to_string();
    match tokio::time::timeout(SOURCE_TIMEOUT, f()).await {
        Ok(Ok(output)) => (name, output),
        Ok(Err(e)) => {
            tracing::warn!(source = label, error = %e, "aggregation source failed");
            (name, format!("[{label}]: unavailable"))
        }
        Err(_) => {
            tracing::warn!(source = label, "aggregation source timed out (5s)");
            (name, format!("[{label}]: unavailable (timeout)"))
        }
    }
}

/// Check if a result line indicates the source was unavailable.
fn is_unavailable(s: &str) -> bool {
    s.contains("]: unavailable")
}

// ── project_health ──────────────────────────────────────────────────

/// Execute `project_health(code)` — calls Vercel, Sentry, Jira, Nexus, and
/// GitHub CI in parallel, each with a 5-second timeout.
pub async fn project_health(
    code: &str,
    jira_client: Option<&jira::JiraClient>,
    nexus_client: Option<&nexus::client::NexusClient>,
) -> Result<String> {
    let map = project_map();
    let resources = map.get(code);

    if resources.is_none() {
        return Ok(format!("Unknown project code: {code}. Known: oo, tc, tl, mv, ss, cl, co, cw, nv"));
    }
    let res = resources.unwrap();

    let start = std::time::Instant::now();

    // Spawn all source calls in parallel via tokio::join!
    let (deploy_result, sentry_result, jira_result, nexus_result, ci_result) = tokio::join!(
        // Vercel deployments
        async {
            if let Some(project) = res.vercel_project {
                timed_call("Deploy", || async move {
                    let client = vercel_tools::VercelClient::from_env()?;
                    vercel_tools::vercel_deployments(&client, project).await
                })
                .await
            } else {
                ("Deploy".to_string(), "N/A (no Vercel project)".to_string())
            }
        },
        // Sentry issues
        async {
            if let Some(slug) = res.sentry_slug {
                let slug_owned = slug.to_string();
                timed_call("Errors", || async move {
                    sentry_tools::sentry_issues(&slug_owned).await
                })
                .await
            } else {
                ("Errors".to_string(), "N/A (no Sentry project)".to_string())
            }
        },
        // Jira issues
        async {
            if let (Some(key), Some(client)) = (res.jira_key, jira_client) {
                let jql = format!("project={key} AND status!=Done ORDER BY priority ASC");
                timed_call("Issues", || async move {
                    let issues = client.search(&jql).await?;
                    Ok(jira::format_issues_for_claude(&issues))
                })
                .await
            } else {
                ("Issues".to_string(), "N/A (Jira not configured)".to_string())
            }
        },
        // Nexus sessions
        async {
            if let Some(client) = nexus_client {
                timed_call("Sessions", || async {
                    nexus::tools::format_query_sessions(client).await
                })
                .await
            } else {
                (
                    "Sessions".to_string(),
                    "N/A (Nexus not configured)".to_string(),
                )
            }
        },
        // GitHub CI status
        async {
            if let Some(repo) = res.github_repo {
                let repo_owned = repo.to_string();
                timed_call("CI", || async move {
                    github::gh_run_status(&repo_owned).await
                })
                .await
            } else {
                ("CI".to_string(), "N/A (no GitHub repo)".to_string())
            }
        },
    );

    let elapsed = start.elapsed();

    let results = [
        &deploy_result,
        &sentry_result,
        &jira_result,
        &nexus_result,
        &ci_result,
    ];

    let succeeded = results
        .iter()
        .filter(|(_, v)| !is_unavailable(v) && !v.starts_with("N/A"))
        .count();
    let attempted = results
        .iter()
        .filter(|(_, v)| !v.starts_with("N/A"))
        .count();

    // Check if ALL non-N/A sources failed
    if attempted > 0 && succeeded == 0 {
        return Ok(format!(
            "{} Health: All sources unavailable. Check individual tools.",
            code.to_uppercase()
        ));
    }

    let mut output = format!("{} Health:", code.to_uppercase());
    for (label, value) in &results {
        output.push_str(&format!("\n  {label}: {value}"));
    }
    output.push_str(&format!(
        "\n  ---\n  ({succeeded}/{attempted} sources, {:.0}ms)",
        elapsed.as_millis()
    ));

    tracing::info!(
        tool = "project_health",
        code,
        succeeded,
        attempted,
        duration_ms = elapsed.as_millis() as u64,
        "aggregation complete"
    );

    Ok(output)
}

// ── homelab_status ──────────────────────────────────────────────────

/// Execute `homelab_status()` — calls Docker, Tailscale, and Home Assistant
/// in parallel, each with a 5-second timeout.
pub async fn homelab_status() -> Result<String> {
    let start = std::time::Instant::now();

    let (docker_result, tailscale_result, ha_result) = tokio::join!(
        timed_call("Docker", || async { docker_tools::docker_status(false).await }),
        timed_call("Tailscale", || async {
            tailscale::TailscaleClient::status().await
        }),
        timed_call("Home", || async { ha_tools::ha_states().await }),
    );

    let elapsed = start.elapsed();

    let results = [&docker_result, &tailscale_result, &ha_result];

    let succeeded = results
        .iter()
        .filter(|(_, v)| !is_unavailable(v))
        .count();
    let total = results.len();

    if succeeded == 0 {
        return Ok("Homelab: All sources unavailable. Check individual tools.".to_string());
    }

    let mut output = String::from("Homelab:");
    for (label, value) in &results {
        output.push_str(&format!("\n  {label}: {value}"));
    }
    output.push_str(&format!(
        "\n  ---\n  ({succeeded}/{total} sources, {:.0}ms)",
        elapsed.as_millis()
    ));

    tracing::info!(
        tool = "homelab_status",
        succeeded,
        total,
        duration_ms = elapsed.as_millis() as u64,
        "aggregation complete"
    );

    Ok(output)
}

// ── financial_summary ───────────────────────────────────────────────

/// Execute `financial_summary()` — calls Plaid and Stripe in parallel,
/// each with a 5-second timeout.
pub async fn financial_summary() -> Result<String> {
    let start = std::time::Instant::now();

    let (plaid_result, stripe_result) = tokio::join!(
        timed_call("Accounts", || async { plaid_tools::plaid_balances().await }),
        timed_call("Stripe", || async {
            stripe_tools::stripe_invoices("open").await
        }),
    );

    let elapsed = start.elapsed();

    let results = [&plaid_result, &stripe_result];

    let succeeded = results
        .iter()
        .filter(|(_, v)| !is_unavailable(v))
        .count();
    let total = results.len();

    if succeeded == 0 {
        return Ok("Finances: All sources unavailable. Check individual tools.".to_string());
    }

    let mut output = String::from("Finances:");
    for (label, value) in &results {
        output.push_str(&format!("\n  {label}: {value}"));
    }
    output.push_str(&format!(
        "\n  ---\n  ({succeeded}/{total} sources, {:.0}ms)",
        elapsed.as_millis()
    ));

    tracing::info!(
        tool = "financial_summary",
        succeeded,
        total,
        duration_ms = elapsed.as_millis() as u64,
        "aggregation complete"
    );

    Ok(output)
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_map_contains_known_projects() {
        let map = project_map();
        assert!(map.contains_key("oo"));
        assert!(map.contains_key("tc"));
        assert!(map.contains_key("tl"));
        assert!(map.contains_key("mv"));
        assert!(map.contains_key("ss"));
        assert!(map.contains_key("cl"));
        assert!(map.contains_key("co"));
        assert!(map.contains_key("nv"));
    }

    #[test]
    fn project_map_oo_has_all_resources() {
        let map = project_map();
        let oo = map.get("oo").expect("oo should exist");
        assert!(oo.vercel_project.is_some());
        assert!(oo.sentry_slug.is_some());
        assert!(oo.jira_key.is_some());
        assert!(oo.github_repo.is_some());
    }

    #[test]
    fn project_map_cl_has_no_vercel() {
        let map = project_map();
        let cl = map.get("cl").expect("cl should exist");
        assert!(cl.vercel_project.is_none());
        assert!(cl.sentry_slug.is_none());
        assert!(cl.jira_key.is_some());
        assert!(cl.github_repo.is_some());
    }

    #[test]
    fn aggregation_tool_definitions_count() {
        let tools = aggregation_tool_definitions();
        assert_eq!(tools.len(), 3);
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"project_health"));
        assert!(names.contains(&"homelab_status"));
        assert!(names.contains(&"financial_summary"));
    }

    #[test]
    fn aggregation_tool_schemas_valid() {
        let tools = aggregation_tool_definitions();
        for tool in &tools {
            assert!(!tool.name.is_empty());
            assert!(!tool.description.is_empty());
            assert_eq!(tool.input_schema["type"], "object");
            assert!(tool.input_schema.get("properties").is_some());
            assert!(tool.input_schema.get("required").is_some());
        }
    }

    #[test]
    fn project_health_requires_code() {
        let tools = aggregation_tool_definitions();
        let ph = tools
            .iter()
            .find(|t| t.name == "project_health")
            .unwrap();
        let required = ph.input_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v.as_str() == Some("code")));
    }

    #[test]
    fn homelab_status_no_required_params() {
        let tools = aggregation_tool_definitions();
        let hs = tools
            .iter()
            .find(|t| t.name == "homelab_status")
            .unwrap();
        let required = hs.input_schema["required"].as_array().unwrap();
        assert!(required.is_empty());
    }

    #[test]
    fn financial_summary_no_required_params() {
        let tools = aggregation_tool_definitions();
        let fs = tools
            .iter()
            .find(|t| t.name == "financial_summary")
            .unwrap();
        let required = fs.input_schema["required"].as_array().unwrap();
        assert!(required.is_empty());
    }

    #[test]
    fn is_unavailable_detects_failure() {
        assert!(is_unavailable("[Deploy]: unavailable"));
        assert!(is_unavailable("[Deploy]: unavailable (timeout)"));
        assert!(!is_unavailable("3 deployments found"));
        assert!(!is_unavailable("N/A (no Vercel project)"));
    }

    #[tokio::test]
    async fn project_health_unknown_code_returns_message() {
        let result = project_health("zzz", None, None).await.unwrap();
        assert!(result.contains("Unknown project code"));
        assert!(result.contains("zzz"));
    }
}
