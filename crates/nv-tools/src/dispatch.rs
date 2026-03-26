use anyhow::{anyhow, Result};
use serde_json::Value;

use crate::tools::*;

/// Dispatch a stateless (nv-tools-local) tool call.
///
/// All 16 stateless tool modules live in `crate::tools::*`. Each module exposes
/// standalone async functions that construct their own clients from environment
/// variables. This function extracts arguments and routes to those functions.
///
/// Returns `Err` with "Tool not found" for names not covered here — the caller
/// should fall through to daemon `SharedDeps`.
pub async fn dispatch_stateless(name: &str, args: &Value) -> Result<Value> {
    let output = match name {
        // ── ADO ──────────────────────────────────────────────────────────────
        "ado_projects" => ado::ado_projects().await?,
        "ado_pipelines" => {
            let project = args["project"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'project' parameter"))?;
            ado::ado_pipelines(project).await?
        }
        "ado_builds" => {
            let project = args["project"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'project' parameter"))?;
            let pipeline_id = args["pipeline_id"]
                .as_u64()
                .ok_or_else(|| anyhow!("missing 'pipeline_id' parameter"))? as u32;
            ado::ado_builds(project, pipeline_id).await?
        }

        // ── Calendar ─────────────────────────────────────────────────────────
        "calendar_today" => {
            let client = calendar::from_env()?;
            calendar::calendar_today(&client).await?
        }
        "calendar_upcoming" => {
            let client = calendar::from_env()?;
            let days = args["days"].as_u64().map(|d| d as u32);
            calendar::calendar_upcoming(&client, days).await?
        }
        "calendar_next" => {
            let client = calendar::from_env()?;
            calendar::calendar_next(&client).await?
        }

        // ── Cloudflare ────────────────────────────────────────────────────────
        "cf_zones" => {
            let client = cloudflare::CloudflareClient::from_env()?;
            cloudflare::cf_zones(&client).await?
        }
        "cf_dns_records" => {
            let client = cloudflare::CloudflareClient::from_env()?;
            let domain = args["domain"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'domain' parameter"))?;
            let record_type = args["record_type"].as_str();
            cloudflare::cf_dns_records(&client, domain, record_type).await?
        }
        "cf_domain_status" => {
            let client = cloudflare::CloudflareClient::from_env()?;
            let domain = args["domain"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'domain' parameter"))?;
            cloudflare::cf_domain_status(&client, domain).await?
        }

        // ── Docker ────────────────────────────────────────────────────────────
        "docker_status" => {
            let all = args["all"].as_bool().unwrap_or(false);
            docker::docker_status(all).await?
        }
        "docker_logs" => {
            let container = args["container"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'container' parameter"))?;
            let lines = args["lines"].as_u64();
            docker::docker_logs(container, lines).await?
        }

        // ── Doppler ───────────────────────────────────────────────────────────
        "doppler_secrets" => {
            let client = doppler::DopplerClient::from_env()?;
            let project = args["project"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'project' parameter"))?;
            let environment = args["environment"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'environment' parameter"))?;
            doppler::doppler_secrets(&client, project, environment, None).await?
        }
        "doppler_compare" => {
            let client = doppler::DopplerClient::from_env()?;
            let project = args["project"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'project' parameter"))?;
            let env_a = args["env_a"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'env_a' parameter"))?;
            let env_b = args["env_b"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'env_b' parameter"))?;
            doppler::doppler_compare(&client, project, env_a, env_b, None).await?
        }
        "doppler_activity" => {
            let client = doppler::DopplerClient::from_env()?;
            let project = args["project"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'project' parameter"))?;
            let count = args["count"].as_u64();
            doppler::doppler_activity(&client, project, count, None).await?
        }

        // ── GitHub ────────────────────────────────────────────────────────────
        "gh_pr_list" => {
            let repo = args["repo"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'repo' parameter"))?;
            github::gh_pr_list(repo).await?
        }
        "gh_run_status" => {
            let repo = args["repo"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'repo' parameter"))?;
            github::gh_run_status(repo).await?
        }
        "gh_issues" => {
            let repo = args["repo"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'repo' parameter"))?;
            github::gh_issues(repo).await?
        }
        "gh_pr_detail" => {
            let repo = args["repo"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'repo' parameter"))?;
            let pr_number = args["pr_number"]
                .as_u64()
                .ok_or_else(|| anyhow!("missing or invalid 'pr_number' parameter"))?;
            github::gh_pr_detail(repo, pr_number).await?
        }
        "gh_pr_diff" => {
            let repo = args["repo"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'repo' parameter"))?;
            let pr_number = args["pr_number"]
                .as_u64()
                .ok_or_else(|| anyhow!("missing or invalid 'pr_number' parameter"))?;
            let file_filter = args["file_filter"].as_str();
            github::gh_pr_diff(repo, pr_number, file_filter).await?
        }
        "gh_releases" => {
            let repo = args["repo"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'repo' parameter"))?;
            let limit = args["limit"].as_u64();
            github::gh_releases(repo, limit).await?
        }
        "gh_compare" => {
            let repo = args["repo"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'repo' parameter"))?;
            let base = args["base"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'base' parameter"))?;
            let head = args["head"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'head' parameter"))?;
            github::gh_compare(repo, base, head).await?
        }

        // ── Home Assistant ────────────────────────────────────────────────────
        "ha_states" => ha::ha_states().await?,
        "ha_entity" => {
            let id = args["id"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'id' parameter"))?;
            ha::ha_entity(id).await?
        }

        // ── Neon ──────────────────────────────────────────────────────────────
        "neon_query" => {
            let project = args["project"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'project' parameter"))?;
            let sql = args["sql"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'sql' parameter"))?;
            neon::neon_query(project, sql).await?
        }
        "neon_projects" => neon::neon_projects().await?,
        "neon_branches" => {
            let project_id = args["project_id"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'project_id' parameter"))?;
            neon::neon_branches(project_id).await?
        }
        "neon_compute" => {
            let project_id = args["project_id"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'project_id' parameter"))?;
            let branch_id = args["branch_id"].as_str();
            neon::neon_compute(project_id, branch_id).await?
        }

        // ── Plaid ─────────────────────────────────────────────────────────────
        "plaid_balances" => plaid::plaid_balances().await?,
        "plaid_bills" => plaid::plaid_bills().await?,

        // ── PostHog ───────────────────────────────────────────────────────────
        "posthog_trends" => {
            let project = args["project"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'project' parameter"))?;
            let event = args["event"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'event' parameter"))?;
            posthog::query_trends(project, event).await?
        }
        "posthog_flags" => {
            let project = args["project"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'project' parameter"))?;
            posthog::list_flags(project).await?
        }

        // ── Resend ────────────────────────────────────────────────────────────
        "resend_emails" => {
            let status = args["status"].as_str();
            resend::resend_emails(status).await?
        }
        "resend_bounces" => resend::resend_bounces().await?,

        // ── Sentry ────────────────────────────────────────────────────────────
        "sentry_issues" => {
            let client = sentry::SentryClient::from_env()?;
            let project = args["project"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'project' parameter"))?;
            sentry::sentry_issues(&client, project).await?
        }
        "sentry_issue" => {
            let client = sentry::SentryClient::from_env()?;
            let id = args["id"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'id' parameter"))?;
            sentry::sentry_issue(&client, id).await?
        }

        // ── Stripe ────────────────────────────────────────────────────────────
        "stripe_customers" => {
            let client = stripe::StripeClient::from_env()?;
            let query = args["query"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'query' parameter"))?;
            stripe::stripe_customers(&client, query).await?
        }
        "stripe_invoices" => {
            let client = stripe::StripeClient::from_env()?;
            let status = args["status"].as_str().unwrap_or("open");
            stripe::stripe_invoices(&client, status).await?
        }

        // ── Upstash ───────────────────────────────────────────────────────────
        "upstash_info" => upstash::upstash_info().await?,
        "upstash_keys" => {
            let pattern = args["pattern"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'pattern' parameter"))?;
            upstash::upstash_keys(pattern).await?
        }

        // ── Vercel ────────────────────────────────────────────────────────────
        "vercel_deployments" => {
            let client = vercel::VercelClient::from_env()?;
            let project = args["project"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'project' parameter"))?;
            vercel::vercel_deployments(&client, project).await?
        }
        "vercel_logs" => {
            let client = vercel::VercelClient::from_env()?;
            let deploy_id = args["deploy_id"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'deploy_id' parameter"))?;
            vercel::vercel_logs(&client, deploy_id).await?
        }

        // ── Outlook ───────────────────────────────────────────────────────────
        "outlook_inbox" => {
            let mut auth = outlook::GraphUserAuth::from_env_or_cache().await?;
            let folder = args["folder"].as_str();
            let count = args["count"].as_u64().unwrap_or(10) as u32;
            let unread_only = args["unread_only"].as_bool().unwrap_or(false);
            outlook::outlook_inbox(&mut auth, folder, count, unread_only).await?
        }
        "outlook_calendar" => {
            let mut auth = outlook::GraphUserAuth::from_env_or_cache().await?;
            let days_ahead = args["days_ahead"].as_u64().unwrap_or(1) as u32;
            let max_events = args["max_events"].as_u64().unwrap_or(10) as u32;
            outlook::outlook_calendar(&mut auth, days_ahead, max_events).await?
        }
        "outlook_read_email" => {
            let mut auth = outlook::GraphUserAuth::from_env_or_cache().await?;
            let message_id = args["message_id"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'message_id' parameter"))?;
            outlook::outlook_read_email(&mut auth, message_id).await?
        }

        // ── Web ───────────────────────────────────────────────────────────────
        "fetch_url" => {
            let url = args["url"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'url' parameter"))?;
            let format_hint = args["format"].as_str();
            web::fetch_url(url, format_hint).await?
        }
        "check_url" => {
            let url = args["url"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'url' parameter"))?;
            web::check_url(url).await?
        }
        "search_web" => {
            let query = args["query"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'query' parameter"))?;
            let count = args["count"].as_u64().unwrap_or(5) as usize;
            let search_url_env = std::env::var("NV_WEB_SEARCH_URL").ok();
            let search_url = search_url_env.as_deref();
            web::search_web(query, count, search_url).await?
        }

        _ => anyhow::bail!("Tool not found: {name}"),
    };

    Ok(Value::String(output))
}

/// Collect all stateless tool definitions from the 17 tool modules (+ 3 outlook).
pub fn stateless_tool_definitions() -> Vec<nv_core::ToolDefinition> {
    let mut tools = Vec::new();
    tools.extend(ado::ado_tool_definitions());
    tools.extend(calendar::calendar_tool_definitions());
    tools.extend(cloudflare::cloudflare_tool_definitions());
    tools.extend(docker::docker_tool_definitions());
    tools.extend(doppler::doppler_tool_definitions());
    tools.extend(github::github_tool_definitions());
    tools.extend(ha::ha_tool_definitions());
    tools.extend(neon::neon_tool_definitions());
    tools.extend(outlook::outlook_tool_definitions());
    tools.extend(plaid::plaid_tool_definitions());
    tools.extend(posthog::posthog_tool_definitions());
    tools.extend(resend::resend_tool_definitions());
    tools.extend(sentry::sentry_tool_definitions());
    tools.extend(stripe::stripe_tool_definitions());
    tools.extend(upstash::upstash_tool_definitions());
    tools.extend(vercel::vercel_tool_definitions());
    tools.extend(web::web_tool_definitions());
    tools
}
