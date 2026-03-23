//! Service health check orchestrator.
//!
//! Dead-code is expected until the CLI batch ([4.x]) wires up `nv check`.
#![allow(dead_code)]
//!
//! `check_all()` runs `check_read()` (and optionally `check_write()`) probes
//! concurrently across all registered services using `FuturesUnordered`, then
//! collects results into a `CheckReport`.
//!
//! Two output formatters are provided:
//! - `format_terminal()` — human-readable colored output for `nv check`
//! - `format_json()` — machine-readable JSON for `nv check --json` and the
//!   `check_services` Nova tool

use std::time::Instant;

use futures_util::StreamExt;
use futures_util::stream::FuturesUnordered;

use super::{CheckResult, Checkable};

// ── MissingService ────────────────────────────────────────────────────

/// Placeholder `Checkable` for services whose client failed to construct.
///
/// Immediately returns `CheckResult::Missing` without making any network call.
/// Used when `from_env()` fails (credential not set) so the entry still appears
/// in the report rather than being silently omitted.
pub struct MissingService {
    name: String,
    env_var: String,
}

impl MissingService {
    pub fn new(name: &str, env_var: &str) -> Self {
        Self {
            name: name.to_string(),
            env_var: env_var.to_string(),
        }
    }
}

#[async_trait::async_trait]
impl Checkable for MissingService {
    fn name(&self) -> &str {
        &self.name
    }

    async fn check_read(&self) -> CheckResult {
        CheckResult::Missing {
            env_var: self.env_var.clone(),
        }
    }
}

// ── CheckEntry ───────────────────────────────────────────────────────

/// A single probe result for one service (read or write).
#[derive(Debug, Clone, serde::Serialize)]
pub struct CheckEntry {
    /// Service identifier, e.g. `"stripe"`, `"jira/personal"`.
    pub name: String,
    /// Whether this is a read or write probe.
    pub probe: ProbeKind,
    /// The result of the probe.
    pub result: CheckResult,
}

/// Distinguishes read vs write probes in a `CheckEntry`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeKind {
    Read,
    Write,
}

// ── CheckReport ──────────────────────────────────────────────────────

/// Aggregated results from running all service probes.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CheckReport {
    /// All read probe results (channels + tools).
    pub read_results: Vec<CheckEntry>,
    /// All write probe results (services that implement `check_write`).
    pub write_results: Vec<CheckEntry>,
    /// Summary counts.
    pub summary: Summary,
}

/// Summary statistics derived from probe results.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Summary {
    pub total: usize,
    pub healthy: usize,
    pub degraded: usize,
    pub unhealthy: usize,
    pub missing: usize,
}

impl CheckReport {
    fn build(read_results: Vec<CheckEntry>, write_results: Vec<CheckEntry>) -> Self {
        let all: Vec<&CheckEntry> = read_results.iter().chain(write_results.iter()).collect();
        let total = all.len();
        let healthy = all
            .iter()
            .filter(|e| matches!(e.result, CheckResult::Healthy { .. }))
            .count();
        let degraded = all
            .iter()
            .filter(|e| matches!(e.result, CheckResult::Degraded { .. }))
            .count();
        let unhealthy = all
            .iter()
            .filter(|e| matches!(e.result, CheckResult::Unhealthy { .. }))
            .count();
        let missing = all
            .iter()
            .filter(|e| matches!(e.result, CheckResult::Missing { .. }))
            .count();
        Self {
            read_results,
            write_results,
            summary: Summary {
                total,
                healthy,
                degraded,
                unhealthy,
                missing,
            },
        }
    }
}

// ── check_all ────────────────────────────────────────────────────────

/// Run read (and optionally write) probes against all provided services concurrently.
///
/// `services` is a slice of `&dyn Checkable` — callers collect instances from
/// their registries before calling. `include_write` corresponds to `--read-only`
/// being absent.
///
/// Uses `FuturesUnordered` so all probes start simultaneously and results are
/// collected as they complete.
pub async fn check_all(
    services: &[&dyn Checkable],
    include_write: bool,
) -> CheckReport {
    // ── Read probes ──────────────────────────────────────────────────
    let mut read_futures: FuturesUnordered<_> = services
        .iter()
        .map(|svc| {
            let name = svc.name().to_string();
            let fut = svc.check_read();
            async move {
                CheckEntry {
                    name,
                    probe: ProbeKind::Read,
                    result: fut.await,
                }
            }
        })
        .collect();

    let mut read_results = Vec::with_capacity(services.len());
    while let Some(entry) = read_futures.next().await {
        read_results.push(entry);
    }

    // Sort by name for stable output
    read_results.sort_by(|a, b| a.name.cmp(&b.name));

    // ── Write probes (optional) ──────────────────────────────────────
    let write_results = if include_write {
        let mut write_futures: FuturesUnordered<_> = services
            .iter()
            .map(|svc| {
                let name = svc.name().to_string();
                let fut = svc.check_write();
                async move { (name, fut.await) }
            })
            .collect();

        let mut results = Vec::new();
        while let Some((name, maybe_result)) = write_futures.next().await {
            if let Some(result) = maybe_result {
                results.push(CheckEntry {
                    name,
                    probe: ProbeKind::Write,
                    result,
                });
            }
        }
        results.sort_by(|a, b| a.name.cmp(&b.name));
        results
    } else {
        Vec::new()
    };

    CheckReport::build(read_results, write_results)
}

// ── Terminal formatter ────────────────────────────────────────────────

/// Format a `CheckReport` for human-readable terminal output.
///
/// Produces colored output matching the `nv check` display spec:
/// ```text
///  Tools (read)
///   ✓ stripe       sk_live_...abc     67ms
///   ✗ stripe/llc   STRIPE_SECRET_KEY_LLC missing  --
/// ```
pub fn format_terminal(report: &CheckReport) -> String {
    let mut out = String::new();

    if !report.read_results.is_empty() {
        out.push_str(" Services (read)\n");
        for entry in &report.read_results {
            out.push_str(&format_entry_terminal(entry));
        }
        out.push('\n');
    }

    if !report.write_results.is_empty() {
        out.push_str(" Services (write)\n");
        for entry in &report.write_results {
            out.push_str(&format_entry_terminal(entry));
        }
        out.push('\n');
    }

    let s = &report.summary;
    out.push_str(&format!(
        " Summary: {}/{} healthy",
        s.healthy,
        s.total,
    ));
    if s.degraded > 0 {
        out.push_str(&format!(", {} degraded", s.degraded));
    }
    if s.unhealthy > 0 {
        out.push_str(&format!(", {} unhealthy", s.unhealthy));
    }
    if s.missing > 0 {
        out.push_str(&format!(", {} missing", s.missing));
    }
    out.push('\n');
    out
}

fn format_entry_terminal(entry: &CheckEntry) -> String {
    match &entry.result {
        CheckResult::Healthy { latency_ms, detail } => {
            format!(
                "  \x1b[32m✓\x1b[0m {:<20} {:<35} {}ms\n",
                entry.name, detail, latency_ms
            )
        }
        CheckResult::Degraded { message } => {
            format!(
                "  \x1b[33m!\x1b[0m {:<20} {}\n",
                entry.name, message
            )
        }
        CheckResult::Unhealthy { error } => {
            format!(
                "  \x1b[31m✗\x1b[0m {:<20} {}\n",
                entry.name, error
            )
        }
        CheckResult::Missing { env_var } => {
            format!(
                "  \x1b[90m○\x1b[0m {:<20} {} not set\n",
                entry.name, env_var
            )
        }
    }
}

// ── Telegram formatter ─────────────────────────────────────────────────

/// Format a `CheckReport` for Telegram delivery — mobile-friendly compact output.
///
/// Uses status emoji (✅/⚠️/❌/○) and shows service name + brief detail per line.
///
/// Example output:
/// ```text
/// ✅/❌ Health — 8/10 healthy
///    ✅ stripe · balance endpoint reachable · 67ms
///    ❌ neon · connection refused
///    ○ vercel · VERCEL_API_TOKEN not set
/// ```
pub fn format_telegram(report: &CheckReport) -> String {
    let s = &report.summary;
    let overall = if s.unhealthy > 0 || s.missing > 0 {
        if s.healthy == s.total { "✅" } else { "❌" }
    } else if s.degraded > 0 {
        "⚠️"
    } else {
        "✅"
    };

    let mut out = format!(
        "{overall} **Health** — {}/{} healthy",
        s.healthy, s.total
    );
    if s.degraded > 0 {
        out.push_str(&format!(", {} degraded", s.degraded));
    }
    if s.unhealthy > 0 {
        out.push_str(&format!(", {} unhealthy", s.unhealthy));
    }
    if s.missing > 0 {
        out.push_str(&format!(", {} missing", s.missing));
    }
    out.push('\n');

    for entry in report.read_results.iter().chain(report.write_results.iter()) {
        out.push_str(&format_entry_telegram(entry));
    }

    // Remove trailing newline
    if out.ends_with('\n') {
        out.pop();
    }

    out
}

fn format_entry_telegram(entry: &CheckEntry) -> String {
    let probe_tag = match entry.probe {
        ProbeKind::Write => " (w)",
        ProbeKind::Read => "",
    };
    match &entry.result {
        CheckResult::Healthy { latency_ms, detail } => {
            format!("   ✅ {}{} · {detail} · {latency_ms}ms\n", entry.name, probe_tag)
        }
        CheckResult::Degraded { message } => {
            format!("   ⚠️ {}{} · {message}\n", entry.name, probe_tag)
        }
        CheckResult::Unhealthy { error } => {
            format!("   ❌ {}{} · {error}\n", entry.name, probe_tag)
        }
        CheckResult::Missing { env_var } => {
            format!("   ○ {}{} · {env_var} not set\n", entry.name, probe_tag)
        }
    }
}

// ── JSON formatter ────────────────────────────────────────────────────

/// Serialize the full `CheckReport` to a JSON string.
///
/// Used by `nv check --json` and the `check_services` Nova tool.
pub fn format_json(report: &CheckReport) -> serde_json::Value {
    serde_json::to_value(report).unwrap_or_else(|e| {
        serde_json::json!({ "error": format!("serialization failed: {e}") })
    })
}

// ── Timing helper ─────────────────────────────────────────────────────

/// Measure the elapsed time of an async operation in milliseconds.
///
/// Intended for use inside `check_read` / `check_write` implementations:
/// ```rust,ignore
/// let (latency_ms, result) = timed(|| async { /* probe */ }).await;
/// ```
pub async fn timed<F, Fut, T>(f: F) -> (u64, T)
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = T>,
{
    let start = Instant::now();
    let result = f().await;
    let latency_ms = start.elapsed().as_millis() as u64;
    (latency_ms, result)
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Mock Checkable ───────────────────────────────────────────────

    struct MockService {
        name: String,
        read_result: CheckResult,
        write_result: Option<CheckResult>,
    }

    #[async_trait::async_trait]
    impl Checkable for MockService {
        fn name(&self) -> &str {
            &self.name
        }

        async fn check_read(&self) -> CheckResult {
            self.read_result.clone()
        }

        async fn check_write(&self) -> Option<CheckResult> {
            self.write_result.clone()
        }
    }

    fn healthy_svc(name: &str) -> MockService {
        MockService {
            name: name.to_string(),
            read_result: CheckResult::Healthy {
                latency_ms: 10,
                detail: "ok".to_string(),
            },
            write_result: None,
        }
    }

    fn missing_svc(name: &str, env_var: &str) -> MockService {
        MockService {
            name: name.to_string(),
            read_result: CheckResult::Missing {
                env_var: env_var.to_string(),
            },
            write_result: None,
        }
    }

    fn writable_svc(name: &str) -> MockService {
        MockService {
            name: name.to_string(),
            read_result: CheckResult::Healthy {
                latency_ms: 5,
                detail: "read ok".to_string(),
            },
            write_result: Some(CheckResult::Healthy {
                latency_ms: 8,
                detail: "write ok".to_string(),
            }),
        }
    }

    // ── Tests ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn check_all_empty_services() {
        let report = check_all(&[], true).await;
        assert!(report.read_results.is_empty());
        assert!(report.write_results.is_empty());
        assert_eq!(report.summary.total, 0);
        assert_eq!(report.summary.healthy, 0);
    }

    #[tokio::test]
    async fn check_all_single_healthy() {
        let svc = healthy_svc("stripe");
        let services: Vec<&dyn Checkable> = vec![&svc];
        let report = check_all(&services, false).await;

        assert_eq!(report.read_results.len(), 1);
        assert_eq!(report.read_results[0].name, "stripe");
        assert!(matches!(
            report.read_results[0].result,
            CheckResult::Healthy { .. }
        ));
        assert_eq!(report.summary.healthy, 1);
        assert_eq!(report.summary.total, 1);
    }

    #[tokio::test]
    async fn check_all_mixed_results() {
        let svc_a = healthy_svc("sentry");
        let svc_b = missing_svc("stripe", "STRIPE_SECRET_KEY");
        let services: Vec<&dyn Checkable> = vec![&svc_a, &svc_b];
        let report = check_all(&services, false).await;

        assert_eq!(report.read_results.len(), 2);
        assert_eq!(report.summary.healthy, 1);
        assert_eq!(report.summary.missing, 1);
        assert_eq!(report.summary.total, 2);
    }

    #[tokio::test]
    async fn check_all_write_probes_included() {
        let svc = writable_svc("jira");
        let services: Vec<&dyn Checkable> = vec![&svc];
        let report = check_all(&services, true).await;

        assert_eq!(report.read_results.len(), 1);
        assert_eq!(report.write_results.len(), 1);
        assert_eq!(report.write_results[0].name, "jira");
        assert_eq!(report.summary.total, 2);
    }

    #[tokio::test]
    async fn check_all_write_probes_skipped_when_read_only() {
        let svc = writable_svc("jira");
        let services: Vec<&dyn Checkable> = vec![&svc];
        let report = check_all(&services, false).await;

        assert_eq!(report.read_results.len(), 1);
        assert!(report.write_results.is_empty());
        assert_eq!(report.summary.total, 1);
    }

    #[test]
    fn format_json_produces_valid_json() {
        let report = CheckReport::build(
            vec![CheckEntry {
                name: "stripe".to_string(),
                probe: ProbeKind::Read,
                result: CheckResult::Healthy {
                    latency_ms: 42,
                    detail: "connected".to_string(),
                },
            }],
            vec![],
        );
        let json = format_json(&report);
        assert!(json.is_object());
        assert!(json["summary"]["healthy"].as_u64().unwrap() == 1);
    }

    #[test]
    fn format_terminal_contains_service_name() {
        let report = CheckReport::build(
            vec![CheckEntry {
                name: "vercel".to_string(),
                probe: ProbeKind::Read,
                result: CheckResult::Missing {
                    env_var: "VERCEL_API_TOKEN".to_string(),
                },
            }],
            vec![],
        );
        let output = format_terminal(&report);
        assert!(output.contains("vercel"));
        assert!(output.contains("VERCEL_API_TOKEN"));
        assert!(output.contains("0/1 healthy"));
    }

    #[tokio::test]
    async fn timed_measures_duration() {
        let (ms, val) = timed(|| async {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            42u32
        })
        .await;
        assert_eq!(val, 42);
        assert!(ms >= 10, "expected at least 10ms, got {ms}ms");
    }

    // ── [5.3] Extended check_all tests ───────────────────────────────

    fn unhealthy_svc(name: &str, error: &str) -> MockService {
        MockService {
            name: name.to_string(),
            read_result: CheckResult::Unhealthy {
                error: error.to_string(),
            },
            write_result: None,
        }
    }

    fn degraded_svc(name: &str, message: &str) -> MockService {
        MockService {
            name: name.to_string(),
            read_result: CheckResult::Degraded {
                message: message.to_string(),
            },
            write_result: None,
        }
    }

    #[tokio::test]
    async fn check_all_unhealthy_counted_in_summary() {
        let svc = unhealthy_svc("neon", "connection refused");
        let services: Vec<&dyn Checkable> = vec![&svc];
        let report = check_all(&services, false).await;

        assert_eq!(report.summary.unhealthy, 1);
        assert_eq!(report.summary.healthy, 0);
        assert_eq!(report.summary.total, 1);
    }

    #[tokio::test]
    async fn check_all_degraded_counted_in_summary() {
        let svc = degraded_svc("posthog", "quota near limit");
        let services: Vec<&dyn Checkable> = vec![&svc];
        let report = check_all(&services, false).await;

        assert_eq!(report.summary.degraded, 1);
        assert_eq!(report.summary.total, 1);
    }

    #[tokio::test]
    async fn check_all_results_sorted_by_name() {
        let svc_z = healthy_svc("zebra");
        let svc_a = healthy_svc("alpha");
        let svc_m = healthy_svc("middle");
        let services: Vec<&dyn Checkable> = vec![&svc_z, &svc_a, &svc_m];
        let report = check_all(&services, false).await;

        assert_eq!(report.read_results[0].name, "alpha");
        assert_eq!(report.read_results[1].name, "middle");
        assert_eq!(report.read_results[2].name, "zebra");
    }

    #[tokio::test]
    async fn check_all_write_results_sorted_by_name() {
        let svc_z = writable_svc("z-svc");
        let svc_a = writable_svc("a-svc");
        let services: Vec<&dyn Checkable> = vec![&svc_z, &svc_a];
        let report = check_all(&services, true).await;

        assert_eq!(report.write_results[0].name, "a-svc");
        assert_eq!(report.write_results[1].name, "z-svc");
    }

    #[tokio::test]
    async fn check_all_write_only_counted_when_service_returns_some() {
        // writable_svc returns Some(Healthy) for write
        // missing_svc returns None for write (default impl)
        let svc_w = writable_svc("with-write");
        let svc_r = missing_svc("read-only", "READ_ONLY_KEY");
        let services: Vec<&dyn Checkable> = vec![&svc_w, &svc_r];
        let report = check_all(&services, true).await;

        // write_results only contains svc_w (svc_r returns None from check_write)
        assert_eq!(report.write_results.len(), 1);
        assert_eq!(report.write_results[0].name, "with-write");
    }

    #[tokio::test]
    async fn check_all_mixed_all_variants() {
        let h = healthy_svc("healthy");
        let d = degraded_svc("degraded", "slow");
        let u = unhealthy_svc("unhealthy", "dead");
        let m = missing_svc("missing", "MY_KEY");
        let services: Vec<&dyn Checkable> = vec![&h, &d, &u, &m];
        let report = check_all(&services, false).await;

        assert_eq!(report.summary.total, 4);
        assert_eq!(report.summary.healthy, 1);
        assert_eq!(report.summary.degraded, 1);
        assert_eq!(report.summary.unhealthy, 1);
        assert_eq!(report.summary.missing, 1);
    }

    #[tokio::test]
    async fn check_all_probe_kind_is_read_for_read_probes() {
        let svc = healthy_svc("stripe");
        let services: Vec<&dyn Checkable> = vec![&svc];
        let report = check_all(&services, false).await;

        assert_eq!(report.read_results[0].probe, ProbeKind::Read);
    }

    #[tokio::test]
    async fn check_all_probe_kind_is_write_for_write_probes() {
        let svc = writable_svc("stripe");
        let services: Vec<&dyn Checkable> = vec![&svc];
        let report = check_all(&services, true).await;

        assert_eq!(report.write_results[0].probe, ProbeKind::Write);
    }

    // ── [5.4] Extended formatter tests ───────────────────────────────

    #[test]
    fn format_terminal_degraded_shows_message() {
        let report = CheckReport::build(
            vec![CheckEntry {
                name: "posthog".to_string(),
                probe: ProbeKind::Read,
                result: CheckResult::Degraded {
                    message: "quota at 90%".to_string(),
                },
            }],
            vec![],
        );
        let output = format_terminal(&report);
        assert!(output.contains("posthog"));
        assert!(output.contains("quota at 90%"));
        assert!(output.contains("degraded"));
        assert!(output.contains("0/1 healthy"));
    }

    #[test]
    fn format_terminal_unhealthy_shows_error() {
        let report = CheckReport::build(
            vec![CheckEntry {
                name: "neon".to_string(),
                probe: ProbeKind::Read,
                result: CheckResult::Unhealthy {
                    error: "connection refused".to_string(),
                },
            }],
            vec![],
        );
        let output = format_terminal(&report);
        assert!(output.contains("neon"));
        assert!(output.contains("connection refused"));
        assert!(output.contains("unhealthy"));
        assert!(output.contains("0/1 healthy"));
    }

    #[test]
    fn format_terminal_healthy_shows_latency_and_detail() {
        let report = CheckReport::build(
            vec![CheckEntry {
                name: "stripe".to_string(),
                probe: ProbeKind::Read,
                result: CheckResult::Healthy {
                    latency_ms: 42,
                    detail: "sk_live_...abc".to_string(),
                },
            }],
            vec![],
        );
        let output = format_terminal(&report);
        assert!(output.contains("stripe"));
        assert!(output.contains("42ms"));
        assert!(output.contains("sk_live_...abc"));
        assert!(output.contains("1/1 healthy"));
    }

    #[test]
    fn format_terminal_shows_write_section_header() {
        let report = CheckReport::build(
            vec![],
            vec![CheckEntry {
                name: "stripe".to_string(),
                probe: ProbeKind::Write,
                result: CheckResult::Healthy {
                    latency_ms: 10,
                    detail: "write ok".to_string(),
                },
            }],
        );
        let output = format_terminal(&report);
        assert!(output.contains("Services (write)"));
    }

    #[test]
    fn format_json_includes_all_summary_fields() {
        let report = CheckReport::build(
            vec![
                CheckEntry {
                    name: "a".to_string(),
                    probe: ProbeKind::Read,
                    result: CheckResult::Healthy { latency_ms: 1, detail: "ok".to_string() },
                },
                CheckEntry {
                    name: "b".to_string(),
                    probe: ProbeKind::Read,
                    result: CheckResult::Unhealthy { error: "err".to_string() },
                },
                CheckEntry {
                    name: "c".to_string(),
                    probe: ProbeKind::Read,
                    result: CheckResult::Missing { env_var: "MY_VAR".to_string() },
                },
                CheckEntry {
                    name: "d".to_string(),
                    probe: ProbeKind::Read,
                    result: CheckResult::Degraded { message: "slow".to_string() },
                },
            ],
            vec![],
        );
        let json = format_json(&report);
        assert_eq!(json["summary"]["total"].as_u64().unwrap(), 4);
        assert_eq!(json["summary"]["healthy"].as_u64().unwrap(), 1);
        assert_eq!(json["summary"]["unhealthy"].as_u64().unwrap(), 1);
        assert_eq!(json["summary"]["missing"].as_u64().unwrap(), 1);
        assert_eq!(json["summary"]["degraded"].as_u64().unwrap(), 1);
    }

    #[test]
    fn format_json_read_results_array_present() {
        let report = CheckReport::build(
            vec![CheckEntry {
                name: "stripe".to_string(),
                probe: ProbeKind::Read,
                result: CheckResult::Healthy { latency_ms: 5, detail: "ok".to_string() },
            }],
            vec![],
        );
        let json = format_json(&report);
        assert!(json["read_results"].is_array());
        let entries = json["read_results"].as_array().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["name"].as_str().unwrap(), "stripe");
    }

    #[test]
    fn format_json_status_tag_present_in_result() {
        // CheckResult uses #[serde(tag = "status")] so each entry has a "status" field
        let report = CheckReport::build(
            vec![CheckEntry {
                name: "vercel".to_string(),
                probe: ProbeKind::Read,
                result: CheckResult::Healthy { latency_ms: 3, detail: "ok".to_string() },
            }],
            vec![],
        );
        let json = format_json(&report);
        let entry = &json["read_results"][0];
        assert_eq!(entry["result"]["status"].as_str().unwrap(), "healthy");
    }

    // ── format_telegram tests ─────────────────────────────────────────

    #[test]
    fn format_telegram_healthy_service() {
        let report = CheckReport::build(
            vec![CheckEntry {
                name: "stripe".to_string(),
                probe: ProbeKind::Read,
                result: CheckResult::Healthy {
                    latency_ms: 42,
                    detail: "balance endpoint reachable".to_string(),
                },
            }],
            vec![],
        );
        let out = format_telegram(&report);
        assert!(out.contains("✅"));
        assert!(out.contains("stripe"));
        assert!(out.contains("42ms"));
        assert!(out.contains("1/1 healthy"));
    }

    #[test]
    fn format_telegram_missing_service() {
        let report = CheckReport::build(
            vec![CheckEntry {
                name: "vercel".to_string(),
                probe: ProbeKind::Read,
                result: CheckResult::Missing {
                    env_var: "VERCEL_API_TOKEN".to_string(),
                },
            }],
            vec![],
        );
        let out = format_telegram(&report);
        assert!(out.contains("○"));
        assert!(out.contains("vercel"));
        assert!(out.contains("VERCEL_API_TOKEN not set"));
        assert!(out.contains("0/1 healthy"));
    }

    #[test]
    fn format_telegram_mixed_services() {
        let report = CheckReport::build(
            vec![
                CheckEntry {
                    name: "stripe".to_string(),
                    probe: ProbeKind::Read,
                    result: CheckResult::Healthy { latency_ms: 10, detail: "ok".to_string() },
                },
                CheckEntry {
                    name: "neon".to_string(),
                    probe: ProbeKind::Read,
                    result: CheckResult::Unhealthy { error: "connection refused".to_string() },
                },
            ],
            vec![],
        );
        let out = format_telegram(&report);
        assert!(out.contains("❌"));
        assert!(out.contains("1/2 healthy"));
        assert!(out.contains("stripe"));
        assert!(out.contains("neon"));
    }

    #[test]
    fn format_telegram_degraded_shows_warning() {
        let report = CheckReport::build(
            vec![CheckEntry {
                name: "posthog".to_string(),
                probe: ProbeKind::Read,
                result: CheckResult::Degraded { message: "quota near limit".to_string() },
            }],
            vec![],
        );
        let out = format_telegram(&report);
        assert!(out.contains("⚠️"));
        assert!(out.contains("posthog"));
        assert!(out.contains("quota near limit"));
    }

    #[test]
    fn format_telegram_write_probe_tagged() {
        let report = CheckReport::build(
            vec![],
            vec![CheckEntry {
                name: "stripe".to_string(),
                probe: ProbeKind::Write,
                result: CheckResult::Healthy { latency_ms: 8, detail: "write ok".to_string() },
            }],
        );
        let out = format_telegram(&report);
        assert!(out.contains("(w)"));
    }

    // ── [5.5] Integration test: check_all pipeline → valid JSON ──────

    #[tokio::test]
    async fn integration_check_all_pipeline_produces_valid_json() {
        // Simulates what `nv check --json` does:
        // 1. Build services (all missing → no network calls)
        // 2. Run check_all
        // 3. format_json → serde_json::to_string_pretty
        // 4. Verify result is parseable and structurally valid

        let svc_a = missing_svc("stripe", "STRIPE_SECRET_KEY");
        let svc_b = missing_svc("vercel", "VERCEL_API_TOKEN");
        let svc_c = healthy_svc("posthog");
        let services: Vec<&dyn Checkable> = vec![&svc_a, &svc_b, &svc_c];

        let report = check_all(&services, false).await;
        let json_value = format_json(&report);
        let json_str = serde_json::to_string_pretty(&json_value)
            .expect("format_json result must be serializable to string");

        // Must be valid JSON
        let reparsed: serde_json::Value =
            serde_json::from_str(&json_str).expect("output must be valid JSON");

        // Top-level keys
        assert!(reparsed["read_results"].is_array());
        assert!(reparsed["write_results"].is_array());
        assert!(reparsed["summary"].is_object());

        // Summary counts are consistent with input
        let summary = &reparsed["summary"];
        assert_eq!(summary["total"].as_u64().unwrap(), 3);
        assert_eq!(summary["missing"].as_u64().unwrap(), 2);
        assert_eq!(summary["healthy"].as_u64().unwrap(), 1);

        // Read results sorted alphabetically
        let read = reparsed["read_results"].as_array().unwrap();
        assert_eq!(read.len(), 3);
        assert_eq!(read[0]["name"].as_str().unwrap(), "posthog");
        assert_eq!(read[1]["name"].as_str().unwrap(), "stripe");
        assert_eq!(read[2]["name"].as_str().unwrap(), "vercel");

        // Each entry has name, probe, result
        for entry in read {
            assert!(entry["name"].is_string());
            assert!(entry["probe"].is_string());
            assert!(entry["result"].is_object());
            assert!(entry["result"]["status"].is_string());
        }
    }

    #[tokio::test]
    async fn integration_check_all_json_with_write_probes() {
        let svc = writable_svc("stripe");
        let services: Vec<&dyn Checkable> = vec![&svc];

        let report = check_all(&services, true).await;
        let json_value = format_json(&report);
        let json_str = serde_json::to_string_pretty(&json_value).unwrap();
        let reparsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        let write = reparsed["write_results"].as_array().unwrap();
        assert_eq!(write.len(), 1);
        assert_eq!(write[0]["probe"].as_str().unwrap(), "write");
    }
}
