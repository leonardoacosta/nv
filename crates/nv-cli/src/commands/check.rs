//! `nv check` — standalone service connectivity diagnostics.
//!
//! Loads clients directly from environment variables (no daemon required),
//! runs `check_all()` probes concurrently, then prints the results.

use nv_daemon::tools::{
    self, Checkable,
    check::{MissingService, check_all, format_json, format_terminal},
};

// ── Check Command ────────────────────────────────────────────────────

/// Run the `nv check` command.
///
/// Builds all configured service clients from environment variables,
/// runs read (and optionally write) probes concurrently, and formats
/// the output as either terminal or JSON.
pub async fn run(json: bool, read_only: bool, service_filter: Option<&str>) {
    // ── Collect Checkable instances ──────────────────────────────────
    //
    // Each client is constructed via `from_env()` — missing credentials
    // produce a `CheckResult::Missing` entry, not a hard error.
    let mut services: Vec<Box<dyn Checkable>> = Vec::new();

    // Stripe
    match tools::stripe::StripeClient::from_env() {
        Ok(c) => services.push(Box::new(c)),
        Err(_) => services.push(Box::new(MissingService::new("stripe", "STRIPE_SECRET_KEY"))),
    }

    // Vercel
    match tools::vercel::VercelClient::from_env() {
        Ok(c) => services.push(Box::new(c)),
        Err(_) => services.push(Box::new(MissingService::new("vercel", "VERCEL_API_TOKEN"))),
    }

    // Sentry
    match tools::sentry::SentryClient::from_env() {
        Ok(c) => services.push(Box::new(c)),
        Err(_) => services.push(Box::new(MissingService::new("sentry", "SENTRY_AUTH_TOKEN"))),
    }

    // Resend
    match tools::resend::ResendClient::from_env() {
        Ok(c) => services.push(Box::new(c)),
        Err(_) => services.push(Box::new(MissingService::new("resend", "RESEND_API_KEY"))),
    }

    // Home Assistant
    match tools::ha::HAClient::from_env() {
        Ok(c) => services.push(Box::new(c)),
        Err(_) => services.push(Box::new(MissingService::new("ha", "HA_TOKEN"))),
    }

    // Upstash
    match tools::upstash::UpstashClient::from_env() {
        Ok(c) => services.push(Box::new(c)),
        Err(_) => services.push(Box::new(MissingService::new("upstash", "UPSTASH_REDIS_REST_URL"))),
    }

    // Azure DevOps
    match tools::ado::AdoClient::from_env() {
        Ok(c) => services.push(Box::new(c)),
        Err(_) => services.push(Box::new(MissingService::new("ado", "ADO_PAT"))),
    }

    // Cloudflare
    match tools::cloudflare::CloudflareClient::from_env() {
        Ok(c) => services.push(Box::new(c)),
        Err(_) => services.push(Box::new(MissingService::new("cloudflare", "CLOUDFLARE_API_TOKEN"))),
    }

    // Doppler
    match tools::doppler::DopplerClient::from_env() {
        Ok(c) => services.push(Box::new(c)),
        Err(_) => services.push(Box::new(MissingService::new("doppler", "DOPPLER_TOKEN"))),
    }

    // Neon — NeonClient takes a project code; use "default" as the probe target.
    // The check_read() impl looks for POSTGRES_URL_DEFAULT.
    services.push(Box::new(tools::neon::NeonClient::new("default")));

    // PostHog — zero-arg constructor (reads env internally during probe)
    services.push(Box::new(tools::posthog::PosthogClient));

    // GitHub — zero-arg constructor (uses `gh auth status` CLI)
    services.push(Box::new(tools::github::GithubClient));

    // Docker — zero-arg constructor (uses `docker info` CLI)
    services.push(Box::new(tools::docker::DockerClient));

    // Plaid — zero-arg constructor
    services.push(Box::new(tools::plaid::PlaidClient));

    // Teams / MS Graph — zero-arg constructor
    services.push(Box::new(tools::teams::TeamsCheck));

    // ── Apply service filter ─────────────────────────────────────────
    let filtered: Vec<&dyn Checkable> = if let Some(filter) = service_filter {
        services
            .iter()
            .filter(|s| s.name().contains(filter))
            .map(|s| s.as_ref())
            .collect()
    } else {
        services.iter().map(|s| s.as_ref()).collect()
    };

    if filtered.is_empty() {
        if let Some(filter) = service_filter {
            eprintln!("No services matching '{filter}'.");
        } else {
            eprintln!("No services configured.");
        }
        std::process::exit(1);
    }

    // ── Run probes ───────────────────────────────────────────────────
    let include_write = !read_only;
    let report = check_all(&filtered, include_write).await;

    // ── Format and print output ──────────────────────────────────────
    if json {
        let value = format_json(&report);
        println!("{}", serde_json::to_string_pretty(&value).unwrap_or_default());
    } else {
        print!("{}", format_terminal(&report));
    }

    // Exit non-zero if any service is unhealthy (not counting missing)
    if report.summary.unhealthy > 0 {
        std::process::exit(1);
    }
}

