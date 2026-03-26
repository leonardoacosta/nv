//! Self-assessment module for Nova — weekly performance analysis.
//!
//! Reads cold-start latency, tool usage statistics, and diary failure patterns,
//! then compiles a structured `SelfAssessmentEntry` stored as JSONL in
//! `~/.nv/state/self-assessment.jsonl`.
//!
//! The analysis is entirely rule-based (no Claude call) and fires once per week
//! via `CronEvent::WeeklySelfAssessment`.

use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::cold_start_store::ColdStartStore;
use crate::messages::MessageStore;

// ── Maximum entries stored in the JSONL file (1 year of weekly reports). ──
const MAX_ENTRIES: usize = 52;

// ── Failure signal phrases scanned in diary Result lines. ────────────────
const FAILURE_PHRASES: &[&str] = &[
    "failed", "error", "timeout", "unavailable", "retry", "auth", "403", "401",
];

// ── Latency thresholds for trend classification. ─────────────────────────
const DEGRADING_RATIO: f64 = 1.2;
const IMPROVING_RATIO: f64 = 0.8;

// ── Suggestion thresholds. ────────────────────────────────────────────────
const P95_SLOW_MS: u64 = 30_000;
const ERROR_RATE_HIGH_PCT: f64 = 10.0;
const TOOL_SLOW_MS: f64 = 5_000.0;
const MIN_EVENTS_FOR_DATA: usize = 7;

// ── Types ─────────────────────────────────────────────────────────────────

/// Trend direction for cold-start latency compared to the prior week.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LatencyTrend {
    /// Current P95 < prior P95 * 0.8
    Improving,
    /// Current P95 within 0.8x–1.2x of prior P95.
    Stable,
    /// Current P95 > prior P95 * 1.2
    Degrading,
}

impl std::fmt::Display for LatencyTrend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LatencyTrend::Improving => f.write_str("Improving"),
            LatencyTrend::Stable => f.write_str("Stable"),
            LatencyTrend::Degrading => f.write_str("Degrading"),
        }
    }
}

/// Cold-start latency summary for the assessment window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyAssessment {
    pub p50_ms: u64,
    pub p95_ms: u64,
    pub p99_ms: u64,
    pub sample_count: usize,
    pub trend: LatencyTrend,
    /// Previous week's P95 used for trend computation. None on first assessment.
    pub prior_p95_ms: Option<u64>,
}

/// Tool error rate summary for the assessment window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorAssessment {
    pub total_invocations: i64,
    /// `(total_invocations - success_count) / total_invocations * 100`
    pub error_rate_pct: f64,
    /// Top 3 tools by failure count: `(tool_name, fail_count)`.
    pub top_failing_tools: Vec<(String, i64)>,
}

/// Tool usage patterns for the assessment window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUsageAssessment {
    pub total_invocations: i64,
    /// Top 5 tools by invocation count: `(tool_name, count)`.
    pub most_used: Vec<(String, i64)>,
    /// Top 3 tools by average duration: `(tool_name, avg_ms)`.
    pub slowest: Vec<(String, f64)>,
}

/// A failure pattern extracted from diary scan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailurePattern {
    pub pattern: String,
    pub occurrences: usize,
    /// ISO 8601 date of the most recent occurrence.
    pub last_seen: String,
    /// Short excerpt from the diary `**Result:**` line.
    pub example: String,
}

/// A complete weekly self-assessment report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfAssessmentEntry {
    /// UUID v4 identifying this report.
    pub id: String,
    /// UTC timestamp when the report was generated.
    pub generated_at: DateTime<Utc>,
    /// Number of days covered by the analysis window.
    pub window_days: u32,
    pub latency: LatencyAssessment,
    pub errors: ErrorAssessment,
    pub tool_usage: ToolUsageAssessment,
    pub failure_patterns: Vec<FailurePattern>,
    /// Up to 3 actionable configuration suggestions (rule-based, no Claude call).
    pub suggestions: Vec<String>,
}

// ── SelfAssessmentStore ───────────────────────────────────────────────────

/// Append-only JSONL store for `SelfAssessmentEntry` records.
///
/// Stored at `~/.nv/state/self-assessment.jsonl`. Capped at `MAX_ENTRIES`
/// (52) entries; the oldest entry is pruned on each append when over the cap.
pub struct SelfAssessmentStore {
    path: PathBuf,
}

impl SelfAssessmentStore {
    /// Open (or create) the JSONL store at `path`.
    ///
    /// Creates parent directories if they do not exist.
    pub fn new(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create_dir_all {}", parent.display()))?;
        }
        // Touch the file if it doesn't exist yet.
        if !path.exists() {
            std::fs::File::create(path)
                .with_context(|| format!("create {}", path.display()))?;
        }
        Ok(Self { path: path.to_path_buf() })
    }

    /// Append a single entry to the JSONL file.
    ///
    /// If the file already contains ≥ `MAX_ENTRIES` lines, the oldest (first)
    /// line is discarded before writing the new one.
    pub fn append(&self, entry: &SelfAssessmentEntry) -> Result<()> {
        let line = serde_json::to_string(entry)
            .context("serialize SelfAssessmentEntry")?;

        // Read existing lines.
        let existing = self.read_all_lines()?;

        // Determine how many to keep: at most MAX_ENTRIES - 1 existing ones.
        let keep_from = if existing.len() >= MAX_ENTRIES {
            existing.len() - (MAX_ENTRIES - 1)
        } else {
            0
        };

        // Rewrite the file with pruned + new entry.
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&self.path)
            .with_context(|| format!("open {} for write", self.path.display()))?;

        for old_line in &existing[keep_from..] {
            writeln!(file, "{old_line}").context("write JSONL line")?;
        }
        writeln!(file, "{line}").context("write new JSONL line")?;

        Ok(())
    }

    /// Return up to `limit` most-recent entries, newest first.
    pub fn get_recent(&self, limit: usize) -> Result<Vec<SelfAssessmentEntry>> {
        let lines = self.read_all_lines()?;
        let mut entries: Vec<SelfAssessmentEntry> = lines
            .iter()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();
        entries.reverse(); // newest first
        entries.truncate(limit);
        Ok(entries)
    }

    /// Return the single most-recent entry, if any.
    pub fn get_latest(&self) -> Result<Option<SelfAssessmentEntry>> {
        Ok(self.get_recent(1)?.into_iter().next())
    }

    // ── Private helpers ──────────────────────────────────────────────

    fn read_all_lines(&self) -> Result<Vec<String>> {
        let file = match std::fs::File::open(&self.path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Vec::new());
            }
            Err(e) => return Err(e).with_context(|| format!("open {}", self.path.display())),
        };
        let reader = std::io::BufReader::new(file);
        let lines = reader
            .lines()
            .filter_map(|r| {
                let l = r.ok()?;
                let trimmed = l.trim().to_string();
                if trimmed.is_empty() { None } else { Some(trimmed) }
            })
            .collect();
        Ok(lines)
    }
}

// ── SelfAssessmentEngine ──────────────────────────────────────────────────

/// Analysis engine that reads operational data and produces a `SelfAssessmentEntry`.
pub struct SelfAssessmentEngine {
    cold_start_store: Arc<Mutex<ColdStartStore>>,
    message_store: Arc<Mutex<MessageStore>>,
    /// `~/.nv/diary/` — scanned for failure patterns.
    diary_base: PathBuf,
    /// JSONL store for prior-week comparison.
    assessment_store: Arc<SelfAssessmentStore>,
}

impl SelfAssessmentEngine {
    /// Create a new engine.
    pub fn new(
        cold_start_store: Arc<Mutex<ColdStartStore>>,
        message_store: Arc<Mutex<MessageStore>>,
        diary_base: PathBuf,
        assessment_store: Arc<SelfAssessmentStore>,
    ) -> Self {
        Self {
            cold_start_store,
            message_store,
            diary_base,
            assessment_store,
        }
    }

    /// Run the full analysis over the last `window_days` days.
    ///
    /// All analysis is rule-based — no Claude API call is made.
    pub fn analyze(&self, window_days: u32) -> Result<SelfAssessmentEntry> {
        let now = Utc::now();
        let window_hours = window_days * 24;

        // ── 1. Latency analysis ───────────────────────────────────────
        let latency = self.analyze_latency(window_hours)?;

        // ── 2. Error analysis ─────────────────────────────────────────
        let errors = self.analyze_errors()?;

        // ── 3. Tool usage analysis ────────────────────────────────────
        let tool_usage = self.analyze_tool_usage()?;

        // ── 4. Diary scan ─────────────────────────────────────────────
        let failure_patterns = self.scan_diary(window_days)?;

        // ── 5. Suggestion generation ──────────────────────────────────
        let suggestions = generate_suggestions(&latency, &errors, &tool_usage);

        Ok(SelfAssessmentEntry {
            id: Uuid::new_v4().to_string(),
            generated_at: now,
            window_days,
            latency,
            errors,
            tool_usage,
            failure_patterns,
            suggestions,
        })
    }

    // ── Private analysis steps ────────────────────────────────────────

    fn analyze_latency(&self, window_hours: u32) -> Result<LatencyAssessment> {
        let percentiles = {
            let store = self.cold_start_store.lock().map_err(|e| anyhow::anyhow!("cold_start_store lock: {e}"))?;
            store.get_percentiles(window_hours)?
        };

        // Fetch prior week p95 from the most recent assessment entry.
        let prior_p95 = self
            .assessment_store
            .get_latest()
            .ok()
            .flatten()
            .map(|e| e.latency.p95_ms);

        let trend = compute_trend(percentiles.p95_ms, prior_p95);

        Ok(LatencyAssessment {
            p50_ms: percentiles.p50_ms,
            p95_ms: percentiles.p95_ms,
            p99_ms: percentiles.p99_ms,
            sample_count: percentiles.sample_count,
            trend,
            prior_p95_ms: prior_p95,
        })
    }

    fn analyze_errors(&self) -> Result<ErrorAssessment> {
        let stats = {
            let store = self.message_store.lock().map_err(|e| anyhow::anyhow!("message_store lock: {e}"))?;
            store.tool_stats()?
        };

        let total = stats.total_invocations;
        let success: i64 = stats.per_tool.iter().map(|t| t.success_count).sum();
        let error_rate_pct = if total == 0 {
            0.0
        } else {
            (total - success) as f64 / total as f64 * 100.0
        };

        // Top 3 failing tools by fail count.
        let mut failing: Vec<(String, i64)> = stats
            .per_tool
            .iter()
            .map(|t| (t.name.clone(), t.count - t.success_count))
            .filter(|(_, fails)| *fails > 0)
            .collect();
        failing.sort_by(|a, b| b.1.cmp(&a.1));
        failing.truncate(3);

        Ok(ErrorAssessment {
            total_invocations: total,
            error_rate_pct,
            top_failing_tools: failing,
        })
    }

    fn analyze_tool_usage(&self) -> Result<ToolUsageAssessment> {
        let stats = {
            let store = self.message_store.lock().map_err(|e| anyhow::anyhow!("message_store lock: {e}"))?;
            store.tool_stats()?
        };

        // Top 5 by invocation count (per_tool is already sorted by cnt DESC).
        let most_used: Vec<(String, i64)> = stats
            .per_tool
            .iter()
            .take(5)
            .map(|t| (t.name.clone(), t.count))
            .collect();

        // Top 3 by average duration.
        let mut by_duration: Vec<(String, f64)> = stats
            .per_tool
            .iter()
            .filter_map(|t| t.avg_duration_ms.map(|d| (t.name.clone(), d)))
            .collect();
        by_duration.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        by_duration.truncate(3);

        Ok(ToolUsageAssessment {
            total_invocations: stats.total_invocations,
            most_used,
            slowest: by_duration,
        })
    }

    fn scan_diary(&self, window_days: u32) -> Result<Vec<FailurePattern>> {
        let today = Utc::now().date_naive();
        let window_start = today - chrono::Duration::days(window_days as i64);

        // Collect diary .md files from the last window_days days.
        let diary_files = collect_diary_files(&self.diary_base, window_start, today)?;

        // Pattern accumulator: phrase → (count, last_date, first_example)
        let mut pattern_map: HashMap<String, (usize, NaiveDate, String)> = HashMap::new();

        for (file_date, path) in &diary_files {
            let metadata = match std::fs::metadata(path) {
                Ok(m) => m,
                Err(_) => continue,
            };
            // Skip files > 1 MB.
            if metadata.len() > 1_024 * 1_024 {
                tracing::warn!(path = %path.display(), "diary file > 1MB, skipping");
                continue;
            }

            let content = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "failed to read diary file");
                    continue;
                }
            };

            // Scan lines for failure phrases, focusing on **Result:** context lines.
            for line in content.lines() {
                let lower = line.to_lowercase();
                for phrase in FAILURE_PHRASES {
                    if lower.contains(phrase) {
                        // Normalize: use the phrase as the pattern key.
                        let pattern = phrase.to_string();
                        let example: String = line.chars().take(120).collect();
                        let entry = pattern_map.entry(pattern).or_insert((0, *file_date, example.clone()));
                        entry.0 += 1;
                        if *file_date > entry.1 {
                            entry.1 = *file_date;
                            entry.2 = example;
                        }
                        // Only count first matching phrase per line to avoid inflating counts.
                        break;
                    }
                }
            }
        }

        // Sort by occurrence count descending, return top 3.
        let mut patterns: Vec<FailurePattern> = pattern_map
            .into_iter()
            .map(|(pattern, (occurrences, last_date, example))| FailurePattern {
                pattern,
                occurrences,
                last_seen: last_date.to_string(),
                example,
            })
            .collect();
        patterns.sort_by(|a, b| b.occurrences.cmp(&a.occurrences));
        patterns.truncate(3);

        Ok(patterns)
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────

/// Compute the `LatencyTrend` by comparing `current_p95` to `prior_p95`.
pub fn compute_trend(current_p95: u64, prior_p95: Option<u64>) -> LatencyTrend {
    let Some(prior) = prior_p95 else {
        return LatencyTrend::Stable;
    };
    if prior == 0 {
        return LatencyTrend::Stable;
    }
    let ratio = current_p95 as f64 / prior as f64;
    if ratio > DEGRADING_RATIO {
        LatencyTrend::Degrading
    } else if ratio < IMPROVING_RATIO {
        LatencyTrend::Improving
    } else {
        LatencyTrend::Stable
    }
}

/// Generate up to 3 rule-based actionable suggestions from assessment data.
pub fn generate_suggestions(
    latency: &LatencyAssessment,
    errors: &ErrorAssessment,
    tool_usage: &ToolUsageAssessment,
) -> Vec<String> {
    let mut suggestions = Vec::new();

    // Insufficient data check.
    if latency.sample_count < MIN_EVENTS_FOR_DATA {
        suggestions.push(format!(
            "Insufficient data for assessment (only {} cold-start events in window; need at least {MIN_EVENTS_FOR_DATA})",
            latency.sample_count
        ));
        return suggestions;
    }

    // P95 > 30s → suggest timeout / context reduction.
    if latency.p95_ms > P95_SLOW_MS {
        suggestions.push(format!(
            "P95 cold-start latency is {}ms — consider increasing Claude timeout or reducing context size",
            latency.p95_ms
        ));
    }

    // High error rate → name the top failing tool.
    if errors.error_rate_pct > ERROR_RATE_HIGH_PCT {
        if let Some((top_tool, _count)) = errors.top_failing_tools.first() {
            suggestions.push(format!(
                "Tool `{top_tool}` is failing frequently (overall error rate {:.1}%) — check credentials/config",
                errors.error_rate_pct
            ));
        } else {
            suggestions.push(format!(
                "Error rate is {:.1}% — review tool configurations",
                errors.error_rate_pct
            ));
        }
    }

    // Any slow tool → suggest caching or timeout reduction.
    if let Some((slow_tool, avg_ms)) = tool_usage
        .slowest
        .iter()
        .find(|(_, avg)| *avg > TOOL_SLOW_MS)
    {
        suggestions.push(format!(
            "Tool `{slow_tool}` has avg duration {avg_ms:.0}ms — consider caching or timeout reduction"
        ));
    }

    // Latency degrading → remind to investigate.
    if suggestions.is_empty() && latency.trend == LatencyTrend::Degrading {
        suggestions.push(format!(
            "P95 latency degraded from {}ms → {}ms — investigate recent changes",
            latency.prior_p95_ms.unwrap_or(0),
            latency.p95_ms
        ));
    }

    suggestions.truncate(3);
    suggestions
}

/// Format a `SelfAssessmentEntry` as the tool output text shown to Claude.
pub fn format_assessment(entry: &SelfAssessmentEntry) -> String {
    let week_str = entry.generated_at.format("%Y-%m-%d").to_string();

    let latency_line = format!(
        "  Latency: P50={}ms P95={}ms P99={}ms ({} samples) — {}",
        entry.latency.p50_ms,
        entry.latency.p95_ms,
        entry.latency.p99_ms,
        entry.latency.sample_count,
        entry.latency.trend,
    );

    let top_failing = if entry.errors.top_failing_tools.is_empty() {
        "none".to_string()
    } else {
        entry.errors.top_failing_tools
            .iter()
            .map(|(t, n)| format!("{t} ({n})"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let errors_line = format!(
        "  Errors: {:.1}% error rate, top failing: {top_failing}",
        entry.errors.error_rate_pct
    );

    let top_tools = entry
        .tool_usage
        .most_used
        .iter()
        .map(|(t, n)| format!("{t} ({n} calls)"))
        .collect::<Vec<_>>()
        .join(", ");
    let tools_line = format!("  Top tools: {top_tools}");

    let patterns_line = if entry.failure_patterns.is_empty() {
        "  Failure patterns: none".to_string()
    } else {
        let p = entry.failure_patterns
            .iter()
            .map(|fp| format!("{} ({})", fp.pattern, fp.occurrences))
            .collect::<Vec<_>>()
            .join(", ");
        format!("  Failure patterns: {p}")
    };

    let suggestions = entry.suggestions.iter().enumerate()
        .map(|(i, s)| format!("    {}. {s}", i + 1))
        .collect::<Vec<_>>()
        .join("\n");
    let suggestions_block = if suggestions.is_empty() {
        "  Suggestions:\n    (none — performance looks good)".to_string()
    } else {
        format!("  Suggestions:\n{suggestions}")
    };

    format!(
        "Self-Assessment (week of {week_str}):\n{latency_line}\n{errors_line}\n{tools_line}\n{patterns_line}\n{suggestions_block}"
    )
}

/// Collect diary `.md` files whose filename date falls within `[start, end]`.
///
/// Diary files are named `YYYY-MM-DD.md` (or similar date-prefixed patterns).
fn collect_diary_files(
    diary_base: &Path,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<Vec<(NaiveDate, PathBuf)>> {
    let mut result = Vec::new();

    let read_dir = match std::fs::read_dir(diary_base) {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(result),
        Err(e) => return Err(e).with_context(|| format!("read_dir {}", diary_base.display())),
    };

    for entry in read_dir.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        // Parse date from filename stem.
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            // Support "YYYY-MM-DD" at start of stem (may have suffix like "-entry").
            let date_part = &stem[..stem.len().min(10)];
            if let Ok(date) = NaiveDate::parse_from_str(date_part, "%Y-%m-%d") {
                if date >= start && date <= end {
                    result.push((date, path));
                }
            }
        }
    }

    result.sort_by_key(|(d, _)| *d);
    Ok(result)
}

// ── Status summary for /status command ───────────────────────────────────

/// Format a compact self-assessment summary for the `/status` bot command.
pub fn format_status_summary(entry: &SelfAssessmentEntry) -> String {
    let age = {
        let secs = (Utc::now() - entry.generated_at).num_seconds().max(0) as u64;
        if secs < 3600 {
            format!("{}m ago", secs / 60)
        } else if secs < 86400 {
            format!("{}h ago", secs / 3600)
        } else {
            format!("{}d ago", secs / 86400)
        }
    };
    let top_suggestion = entry.suggestions.first().map(|s| s.as_str()).unwrap_or("No suggestions.");
    format!(
        "Generated: {age}\nLatency trend: {}\nError rate: {:.1}%\nTop suggestion: {top_suggestion}",
        entry.latency.trend,
        entry.errors.error_rate_pct,
    )
}

/// Format the `[Self-Assessment]` morning briefing injection block.
pub fn format_briefing_section(entry: &SelfAssessmentEntry) -> String {
    let top_suggestions: Vec<&str> = entry.suggestions.iter().map(|s| s.as_str()).take(3).collect();
    let suggestions_text = if top_suggestions.is_empty() {
        "  (none)".to_string()
    } else {
        top_suggestions
            .iter()
            .enumerate()
            .map(|(i, s)| format!("  {}. {s}", i + 1))
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        "[Self-Assessment]\nLatency trend: {} (P95={}ms)\nError rate: {:.1}%\nSuggestions:\n{suggestions_text}",
        entry.latency.trend,
        entry.latency.p95_ms,
        entry.errors.error_rate_pct,
    )
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_entry(p95_ms: u64, error_rate: f64) -> SelfAssessmentEntry {
        SelfAssessmentEntry {
            id: Uuid::new_v4().to_string(),
            generated_at: Utc::now(),
            window_days: 7,
            latency: LatencyAssessment {
                p50_ms: p95_ms / 2,
                p95_ms,
                p99_ms: p95_ms + 1000,
                sample_count: 20,
                trend: LatencyTrend::Stable,
                prior_p95_ms: None,
            },
            errors: ErrorAssessment {
                total_invocations: 100,
                error_rate_pct: error_rate,
                top_failing_tools: vec![],
            },
            tool_usage: ToolUsageAssessment {
                total_invocations: 100,
                most_used: vec![("read_memory".to_string(), 50)],
                slowest: vec![],
            },
            failure_patterns: vec![],
            suggestions: vec![],
        }
    }

    // ── SelfAssessmentStore ───────────────────────────────────────────

    #[test]
    fn store_append_and_get_latest() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("state/self-assessment.jsonl");
        let store = SelfAssessmentStore::new(&path).unwrap();

        let entry = make_entry(5000, 2.0);
        store.append(&entry).unwrap();

        let latest = store.get_latest().unwrap().unwrap();
        assert_eq!(latest.id, entry.id);
    }

    #[test]
    fn store_get_recent_newest_first() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("state/self-assessment.jsonl");
        let store = SelfAssessmentStore::new(&path).unwrap();

        let e1 = make_entry(3000, 1.0);
        let e2 = make_entry(4000, 2.0);
        let e3 = make_entry(5000, 3.0);
        store.append(&e1).unwrap();
        store.append(&e2).unwrap();
        store.append(&e3).unwrap();

        let recent = store.get_recent(10).unwrap();
        assert_eq!(recent.len(), 3);
        // Newest (e3) should be first.
        assert_eq!(recent[0].id, e3.id);
        assert_eq!(recent[1].id, e2.id);
        assert_eq!(recent[2].id, e1.id);
    }

    #[test]
    fn store_cap_at_52_entries() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("state/self-assessment.jsonl");
        let store = SelfAssessmentStore::new(&path).unwrap();

        for _ in 0..55 {
            store.append(&make_entry(1000, 1.0)).unwrap();
        }

        let recent = store.get_recent(100).unwrap();
        assert_eq!(recent.len(), 52, "store must be capped at 52 entries");
    }

    #[test]
    fn store_get_latest_empty_returns_none() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("state/self-assessment.jsonl");
        let store = SelfAssessmentStore::new(&path).unwrap();
        assert!(store.get_latest().unwrap().is_none());
    }

    // ── LatencyTrend ──────────────────────────────────────────────────

    #[test]
    fn trend_degrading() {
        let trend = compute_trend(13000, Some(10000)); // 1.3x > 1.2x
        assert_eq!(trend, LatencyTrend::Degrading);
    }

    #[test]
    fn trend_improving() {
        let trend = compute_trend(7000, Some(10000)); // 0.7x < 0.8x
        assert_eq!(trend, LatencyTrend::Improving);
    }

    #[test]
    fn trend_stable() {
        let trend = compute_trend(10000, Some(10000)); // 1.0x
        assert_eq!(trend, LatencyTrend::Stable);
    }

    #[test]
    fn trend_stable_boundary_high() {
        let trend = compute_trend(12000, Some(10000)); // exactly 1.2x → Stable
        assert_eq!(trend, LatencyTrend::Stable);
    }

    #[test]
    fn trend_stable_boundary_low() {
        let trend = compute_trend(8000, Some(10000)); // exactly 0.8x → Stable
        assert_eq!(trend, LatencyTrend::Stable);
    }

    #[test]
    fn trend_no_prior_is_stable() {
        let trend = compute_trend(9999, None);
        assert_eq!(trend, LatencyTrend::Stable);
    }

    // ── Suggestion generation ─────────────────────────────────────────

    #[test]
    fn suggestion_insufficient_data() {
        let lat = LatencyAssessment {
            p50_ms: 0, p95_ms: 0, p99_ms: 0, sample_count: 3,
            trend: LatencyTrend::Stable, prior_p95_ms: None,
        };
        let err = ErrorAssessment { total_invocations: 0, error_rate_pct: 0.0, top_failing_tools: vec![] };
        let usage = ToolUsageAssessment { total_invocations: 0, most_used: vec![], slowest: vec![] };
        let sugg = generate_suggestions(&lat, &err, &usage);
        assert_eq!(sugg.len(), 1);
        assert!(sugg[0].contains("Insufficient data"), "got: {}", sugg[0]);
    }

    #[test]
    fn suggestion_p95_slow() {
        let lat = LatencyAssessment {
            p50_ms: 15000, p95_ms: 35000, p99_ms: 40000, sample_count: 20,
            trend: LatencyTrend::Stable, prior_p95_ms: None,
        };
        let err = ErrorAssessment { total_invocations: 100, error_rate_pct: 0.0, top_failing_tools: vec![] };
        let usage = ToolUsageAssessment { total_invocations: 100, most_used: vec![], slowest: vec![] };
        let sugg = generate_suggestions(&lat, &err, &usage);
        assert!(sugg.iter().any(|s| s.contains("P95") && s.contains("Claude timeout")));
    }

    #[test]
    fn suggestion_high_error_rate() {
        let lat = LatencyAssessment {
            p50_ms: 2000, p95_ms: 5000, p99_ms: 6000, sample_count: 20,
            trend: LatencyTrend::Stable, prior_p95_ms: None,
        };
        let err = ErrorAssessment {
            total_invocations: 100,
            error_rate_pct: 15.0,
            top_failing_tools: vec![("jira_search".to_string(), 8)],
        };
        let usage = ToolUsageAssessment { total_invocations: 100, most_used: vec![], slowest: vec![] };
        let sugg = generate_suggestions(&lat, &err, &usage);
        assert!(sugg.iter().any(|s| s.contains("jira_search") && s.contains("failing frequently")));
    }

    #[test]
    fn suggestion_slow_tool() {
        let lat = LatencyAssessment {
            p50_ms: 2000, p95_ms: 5000, p99_ms: 6000, sample_count: 20,
            trend: LatencyTrend::Stable, prior_p95_ms: None,
        };
        let err = ErrorAssessment { total_invocations: 100, error_rate_pct: 0.0, top_failing_tools: vec![] };
        let usage = ToolUsageAssessment {
            total_invocations: 100,
            most_used: vec![],
            slowest: vec![("outlook_calendar".to_string(), 7000.0)],
        };
        let sugg = generate_suggestions(&lat, &err, &usage);
        assert!(sugg.iter().any(|s| s.contains("outlook_calendar") && s.contains("avg duration")));
    }

    // ── Diary scan ────────────────────────────────────────────────────

    #[test]
    fn diary_scan_extracts_patterns() {
        let tmp = TempDir::new().unwrap();
        let diary_dir = tmp.path().join("diary");
        std::fs::create_dir_all(&diary_dir).unwrap();

        // Write a diary file for today.
        let today = Utc::now().date_naive();
        let file_path = diary_dir.join(format!("{today}.md"));
        std::fs::write(
            &file_path,
            "## Entry\n**Result:** Failed to reach Jira: connection error\n## Entry2\n**Result:** auth failed for Outlook\n",
        )
        .unwrap();

        let cold_store = Arc::new(Mutex::new(
            ColdStartStore::new(&tmp.path().join("messages.db")).unwrap(),
        ));
        let msg_store = Arc::new(Mutex::new(
            crate::messages::MessageStore::init(&tmp.path().join("messages.db")).unwrap(),
        ));
        let assessment_path = tmp.path().join("state/self-assessment.jsonl");
        let assessment_store = Arc::new(SelfAssessmentStore::new(&assessment_path).unwrap());

        let engine = SelfAssessmentEngine::new(cold_store, msg_store, diary_dir.clone(), assessment_store);
        let patterns = engine.scan_diary(7).unwrap();

        // We should find at least "failed", "error", "auth" patterns.
        assert!(!patterns.is_empty(), "expected patterns to be found");
        assert!(patterns.len() <= 3, "should be capped at 3");
    }
}
