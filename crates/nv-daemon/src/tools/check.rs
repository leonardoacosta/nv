//! Service health check orchestrator.
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
}
