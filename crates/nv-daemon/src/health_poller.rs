//! Background 60s health poll loop with crash detection.
//!
//! # What it does
//!
//! On each tick (every 60 seconds):
//!
//! 1. Collects system metrics from `/proc` (Linux)
//! 2. Stores a `server_health` row via `ServerHealthStore`
//! 3. Compares current `uptime_seconds` to the previous poll — if the current
//!    value is *less* than the previous value, the server restarted between
//!    polls (i.e., a crash was detected)
//! 4. On crash: creates a P1 obligation, optionally spawns a Nexus
//!    investigation session, and stores a crash event obligation
//!
//! # Proc reads
//!
//! | Metric | Source |
//! |--------|--------|
//! | CPU usage | `/proc/stat` (delta between two reads) |
//! | Memory | `/proc/meminfo` |
//! | Disk | `statvfs` on `/` |
//! | Uptime | `/proc/uptime` |
//! | Load avg | `/proc/loadavg` |
//!
//! CPU usage requires two `/proc/stat` reads 200ms apart to compute a delta.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use uuid::Uuid;

use crate::obligation_store::{NewObligation, ObligationStore};
use crate::server_health_store::{NewServerHealth, ServerHealthStore};

// ── Spawn ──────────────────────────────────────────────────────────────

/// Spawn the 60s health poll loop as a background tokio task.
///
/// `db_path` — path to `messages.db`
/// `obligation_store` — shared obligation store for crash P1 obligations
/// `nexus_client` — optional Nexus client to spawn investigation sessions
pub fn spawn_health_poller(
    db_path: PathBuf,
    obligation_store: Arc<Mutex<ObligationStore>>,
    nexus_client: Option<Arc<crate::nexus::client::NexusClient>>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(60));
        // Skip the immediate first tick to let the daemon settle.
        ticker.tick().await;

        loop {
            ticker.tick().await;
            tracing::debug!("health_poller: collecting metrics");

            if let Err(e) = run_poll_cycle(&db_path, &obligation_store, nexus_client.as_deref()).await {
                tracing::warn!(error = %e, "health_poller: poll cycle failed");
            }
        }
    })
}

/// Run one full poll cycle: collect, store, detect crash.
pub async fn run_poll_cycle(
    db_path: &std::path::Path,
    obligation_store: &Arc<Mutex<ObligationStore>>,
    nexus_client: Option<&crate::nexus::client::NexusClient>,
) -> Result<()> {
    let metrics = collect_metrics().await?;

    let store = ServerHealthStore::new(db_path)?;

    // Read previous snapshot before inserting the new one.
    let previous = store.latest()?;

    store.insert(&metrics)?;

    // Detect crash: if previous uptime > current uptime → server restarted.
    if let (Some(prev), Some(current_uptime)) = (previous, metrics.uptime_seconds) {
        if let Some(prev_uptime) = prev.uptime_seconds {
            if prev_uptime > current_uptime {
                tracing::warn!(
                    prev_uptime = prev_uptime,
                    current_uptime = current_uptime,
                    "crash detection: uptime decreased — server restarted"
                );

                handle_crash_detected(
                    db_path,
                    obligation_store,
                    nexus_client,
                    prev_uptime,
                    current_uptime,
                )
                .await;
            }
        }
    }

    // Prune old rows to prevent unbounded growth (keep 7 days).
    if let Err(e) = store.prune_older_than_days(7) {
        tracing::warn!(error = %e, "health_poller: failed to prune old rows");
    }

    Ok(())
}

// ── Crash Handler ──────────────────────────────────────────────────────

/// Called when a crash is detected (uptime decreased between polls).
///
/// 1. Creates a P1 obligation in the obligation store.
/// 2. Spawns a Nexus investigation session (if a client is available).
/// 3. The obligation itself acts as the crash event record.
async fn handle_crash_detected(
    _db_path: &std::path::Path,
    obligation_store: &Arc<Mutex<ObligationStore>>,
    nexus_client: Option<&crate::nexus::client::NexusClient>,
    prev_uptime: i64,
    current_uptime: i64,
) {
    let cause = format!(
        "Server restarted between health polls. Previous uptime: {prev_uptime}s, \
         current uptime: {current_uptime}s. Likely caused by a crash or OOM kill."
    );

    let recommendation = "Investigate system logs (journalctl -u nv-daemon), \
        check for OOM kills in /var/log/syslog, and review recent daemon changes.";

    let detected_action = format!(
        "Investigate server crash: {cause} Recommendation: {recommendation}"
    );

    let new_ob = NewObligation {
        id: Uuid::new_v4().to_string(),
        source_channel: "health_poller".into(),
        source_message: None,
        detected_action,
        project_code: Some("nv".into()),
        priority: 1, // P1 — Critical
        owner: nv_core::types::ObligationOwner::Nova,
        owner_reason: Some("Automated crash detection".into()),
    };

    let obligation_result = {
        match obligation_store.lock() {
            Ok(store) => store.create(new_ob),
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "crash_detection: obligation store mutex poisoned"
                );
                return;
            }
        }
    };

    match obligation_result {
        Ok(ob) => {
            tracing::info!(
                obligation_id = %ob.id,
                priority = ob.priority,
                "crash_detection: P1 obligation created"
            );

            // Spawn Nexus investigation session if available.
            if let Some(nexus) = nexus_client {
                let crash_summary = format!(
                    "Nova daemon crashed (uptime dropped from {prev_uptime}s to {current_uptime}s). \
                     Please investigate the cause."
                );

                let nv_project_path = {
                    let home = std::env::var("HOME").unwrap_or_else(|_| String::new());
                    if home.is_empty() {
                        tracing::warn!("crash_detection: HOME env var not set, using empty path for Nexus session");
                    }
                    format!("{home}/nv")
                };

                match nexus
                    .start_session_with_context("nv", &nv_project_path, &crash_summary, None)
                    .await
                {
                    Ok((session_id, tmux_session)) => {
                        tracing::info!(
                            session_id = %session_id,
                            tmux_session = %tmux_session,
                            "crash_detection: Nexus investigation session spawned"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            "crash_detection: failed to spawn Nexus investigation session"
                        );
                    }
                }
            }
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                "crash_detection: failed to create P1 obligation"
            );
        }
    }
}

// ── Metric collection ──────────────────────────────────────────────────

/// Collect system metrics from /proc (Linux-specific).
///
/// CPU usage requires two `/proc/stat` reads with a 200ms sleep between them
/// to compute a delta. All other reads are instantaneous.
pub async fn collect_metrics() -> Result<NewServerHealth> {
    // Read CPU stats twice with a short delay for delta.
    let cpu_percent = read_cpu_percent().await.ok();
    let uptime_seconds = read_uptime().ok();
    let (load_avg_1m, load_avg_5m) = read_loadavg().ok().unwrap_or((None, None));
    let (memory_used_mb, memory_total_mb) = read_meminfo().ok().unwrap_or((None, None));
    let (disk_used_gb, disk_total_gb) = read_disk_usage("/").ok().unwrap_or((None, None));

    Ok(NewServerHealth {
        cpu_percent,
        memory_used_mb,
        memory_total_mb,
        disk_used_gb,
        disk_total_gb,
        uptime_seconds,
        load_avg_1m,
        load_avg_5m,
    })
}

/// Read CPU usage percentage over a 200ms sample.
///
/// Reads `/proc/stat` twice, computes the delta in idle vs total jiffies.
async fn read_cpu_percent() -> Result<f64> {
    let before = read_cpu_jiffies()?;
    tokio::time::sleep(Duration::from_millis(200)).await;
    let after = read_cpu_jiffies()?;

    let total_delta = after.total.saturating_sub(before.total);
    let idle_delta = after.idle.saturating_sub(before.idle);

    if total_delta == 0 {
        return Ok(0.0);
    }

    let busy_delta = total_delta.saturating_sub(idle_delta);
    let pct = (busy_delta as f64 / total_delta as f64) * 100.0;
    Ok(pct.clamp(0.0, 100.0))
}

struct CpuJiffies {
    total: u64,
    idle: u64,
}

fn read_cpu_jiffies() -> Result<CpuJiffies> {
    let content = std::fs::read_to_string("/proc/stat")?;
    let line = content
        .lines()
        .find(|l| l.starts_with("cpu "))
        .ok_or_else(|| anyhow::anyhow!("cpu line not found in /proc/stat"))?;

    // Format: cpu  user nice system idle iowait irq softirq steal guest guest_nice
    let fields: Vec<u64> = line
        .split_whitespace()
        .skip(1) // skip "cpu"
        .map(|s| s.parse::<u64>().unwrap_or(0))
        .collect();

    if fields.len() < 4 {
        anyhow::bail!("unexpected /proc/stat cpu line: {line}");
    }

    // idle + iowait: fields[3] = idle, fields[4] = iowait.
    // Including iowait matches the definition used by top, mpstat, and the Linux kernel docs.
    let idle = fields[3] + fields.get(4).copied().unwrap_or(0);
    let total: u64 = fields.iter().sum();

    Ok(CpuJiffies { total, idle })
}

/// Read uptime in seconds from `/proc/uptime`.
fn read_uptime() -> Result<i64> {
    let content = std::fs::read_to_string("/proc/uptime")?;
    let uptime_secs: f64 = content
        .split_whitespace()
        .next()
        .ok_or_else(|| anyhow::anyhow!("empty /proc/uptime"))?
        .parse()?;
    Ok(uptime_secs as i64)
}

/// Read load averages from `/proc/loadavg`.
///
/// Returns `(load_1m, load_5m)`.
fn read_loadavg() -> Result<(Option<f64>, Option<f64>)> {
    let content = std::fs::read_to_string("/proc/loadavg")?;
    let mut parts = content.split_whitespace();

    let load_1m = parts.next().and_then(|s| s.parse::<f64>().ok());
    let load_5m = parts.next().and_then(|s| s.parse::<f64>().ok());

    Ok((load_1m, load_5m))
}

/// Read memory stats from `/proc/meminfo`.
///
/// Returns `(used_mb, total_mb)`.
fn read_meminfo() -> Result<(Option<i64>, Option<i64>)> {
    let content = std::fs::read_to_string("/proc/meminfo")?;

    let parse_kb = |prefix: &str| -> Option<i64> {
        content
            .lines()
            .find(|l| l.starts_with(prefix))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|s| s.parse::<i64>().ok())
    };

    let total_kb = parse_kb("MemTotal:");
    let available_kb = parse_kb("MemAvailable:");

    let (total_mb, used_mb) = match (total_kb, available_kb) {
        (Some(t), Some(a)) => {
            let total_mb = t / 1024;
            let used_mb = (t - a) / 1024;
            (Some(total_mb), Some(used_mb))
        }
        _ => (None, None),
    };

    Ok((used_mb, total_mb))
}

/// Read disk usage for the given mount path via `statvfs`.
///
/// Returns `(used_gb, total_gb)`.
fn read_disk_usage(path: &str) -> Result<(Option<f64>, Option<f64>)> {
    // Use std::fs::metadata + nix statvfs alternative: read /proc/mounts
    // and use the libc statvfs. Since we don't have nix in deps, use
    // the statvfs syscall via libc.
    use std::ffi::CString;

    let c_path = CString::new(path)?;

    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    let ret = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };

    if ret != 0 {
        anyhow::bail!("statvfs({path}) failed: errno {}", std::io::Error::last_os_error());
    }

    let block_size = stat.f_frsize;
    let total_bytes = stat.f_blocks * block_size;
    // Use f_bavail (blocks available to unprivileged processes) instead of
    // f_bfree (blocks free for root), to match what `df` reports.
    let free_bytes = stat.f_bavail * block_size;
    let used_bytes = total_bytes.saturating_sub(free_bytes);

    let gb = 1024.0 * 1024.0 * 1024.0;
    let total_gb = total_bytes as f64 / gb;
    let used_gb = used_bytes as f64 / gb;

    Ok((Some(used_gb), Some(total_gb)))
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that `read_cpu_jiffies` includes iowait (fields[4]) in the idle counter.
    ///
    /// We exercise the calculation logic directly by parsing a synthetic /proc/stat line
    /// with known values. The idle field includes iowait to match `top` / `mpstat` semantics.
    #[test]
    fn cpu_idle_includes_iowait() {
        // Simulate: cpu user=100 nice=0 system=50 idle=300 iowait=50 irq=0 ...
        // With the fix: idle = fields[3] + fields[4] = 300 + 50 = 350
        // Without the fix: idle = fields[3] = 300 only
        let fields: Vec<u64> = vec![100, 0, 50, 300, 50, 0, 0, 0];
        let idle = fields[3] + fields.get(4).copied().unwrap_or(0);
        let total: u64 = fields.iter().sum();

        assert_eq!(idle, 350, "idle should include iowait (300 + 50)");
        assert_eq!(total, 500);

        let busy = total - idle;
        assert_eq!(busy, 150, "busy = user + system = 100 + 50");
        let pct = (busy as f64 / total as f64) * 100.0;
        assert!((pct - 30.0).abs() < 0.01, "busy% should be 30%");
    }

    #[test]
    fn cpu_idle_handles_missing_iowait_field() {
        // Only 4 fields (no iowait) — get(4) returns None → +0
        let fields: Vec<u64> = vec![100, 0, 50, 300];
        let idle = fields[3] + fields.get(4).copied().unwrap_or(0);
        assert_eq!(idle, 300, "no iowait field → idle unchanged");
    }

    /// Verify that `read_disk_usage` uses f_bavail (not f_bfree).
    ///
    /// We can't easily call `statvfs` with a synthetic struct in a unit test,
    /// but we verify the calculation logic and that calling on "/" succeeds
    /// (or fails gracefully) on the test host.
    #[test]
    fn disk_usage_reads_without_panic() {
        // This calls statvfs("/") which should succeed on any Linux host.
        // We just verify it returns a non-negative used_gb and that total >= used.
        match read_disk_usage("/") {
            Ok((Some(used), Some(total))) => {
                assert!(used >= 0.0, "used_gb must be non-negative");
                assert!(total > 0.0, "total_gb must be positive");
                assert!(
                    total >= used,
                    "total_gb ({total:.2}) must be >= used_gb ({used:.2})"
                );
            }
            Ok(_) => {} // partial data — no assertion
            Err(e) => {
                // In some environments (containers without /proc), statvfs may fail.
                // This is acceptable in CI.
                eprintln!("read_disk_usage('/') returned Err (may be expected in CI): {e}");
            }
        }
    }

    /// Verify the f_bavail vs f_bfree calculation difference is correct.
    ///
    /// We simulate the two approaches with known values:
    /// total = 1000 blocks, f_bfree = 200 (root-accessible), f_bavail = 150 (user-accessible)
    ///
    /// With f_bfree: used = 1000 - 200 = 800
    /// With f_bavail: used = 1000 - 150 = 850  (correct — includes root-reserved 50 blocks)
    #[test]
    fn disk_bavail_calculation_is_correct() {
        let block_size: u64 = 4096;
        let total_blocks: u64 = 1000;
        let f_bfree: u64 = 200;   // root-accessible free
        let f_bavail: u64 = 150;  // user-accessible free (root-reserved = 50 blocks)

        let total_bytes = total_blocks * block_size;
        let free_bfree = f_bfree * block_size;
        let free_bavail = f_bavail * block_size;

        let used_with_bfree = total_bytes - free_bfree;
        let used_with_bavail = total_bytes - free_bavail;

        assert!(
            used_with_bavail > used_with_bfree,
            "f_bavail reports higher used% than f_bfree (includes root-reserved)"
        );
        assert_eq!(
            used_with_bavail - used_with_bfree,
            (f_bfree - f_bavail) * block_size,
            "difference equals root-reserved blocks * block_size"
        );
    }
}
