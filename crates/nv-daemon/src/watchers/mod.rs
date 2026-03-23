//! Proactive watchers — cron-triggered tasks that evaluate alert rules and
//! create obligations when conditions are met.
//!
//! Each watcher corresponds to an `AlertRuleType`:
//!
//! | Watcher           | Rule type        | Service checked              |
//! |-------------------|------------------|------------------------------|
//! | `deploy_watcher`  | deploy_failure   | Vercel REST API              |
//! | `sentry_watcher`  | sentry_spike     | Sentry REST API              |
//! | `stale_ticket`    | stale_ticket     | Local beads JSONL            |
//! | `ha_watcher`      | ha_anomaly       | Home Assistant REST API      |
//!
//! ## Spawn
//!
//! `spawn_watchers` starts a single background task that fires all enabled
//! watchers on a configurable interval (default 300 seconds / 5 minutes).
//! Each watcher is called concurrently within a `tokio::join!`-style loop.
//!
//! ## Obligation creation
//!
//! When a rule fires, the watcher calls `ObligationStore::create` directly
//! (bypassing the trigger channel). The obligation is visible immediately in
//! the dashboard and in the digest.

pub mod deploy_watcher;
pub mod ha_watcher;
pub mod sentry_watcher;
pub mod stale_ticket_watcher;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::alert_rules::{AlertRule, AlertRuleStore, AlertRuleType, RuleEvaluator};
use crate::obligation_store::ObligationStore;

// ── Spawn ──────────────────────────────────────────────────────────────

/// Spawn the watcher background task.
///
/// On each tick (every `interval_secs`), loads enabled rules from the DB and
/// runs the matching watcher in parallel. Each watcher that fires creates an
/// obligation and updates `last_triggered_at` on the rule.
///
/// Non-fatal: watcher errors are logged as warnings, not propagated.
pub fn spawn_watchers(
    db_path: PathBuf,
    obligation_store: Arc<Mutex<ObligationStore>>,
    interval_secs: u64,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let interval = Duration::from_secs(interval_secs.max(60));
        let mut ticker = tokio::time::interval(interval);
        // Skip the immediate first tick — let the daemon settle on startup.
        ticker.tick().await;

        loop {
            ticker.tick().await;
            tracing::debug!("watcher cycle: evaluating alert rules");

            run_watcher_cycle(&db_path, &obligation_store).await;
        }
    })
}

/// Run one full watcher cycle: load enabled rules, evaluate each one, store obligations.
pub async fn run_watcher_cycle(
    db_path: &std::path::Path,
    obligation_store: &Arc<Mutex<ObligationStore>>,
) {
    // Open rule store — we open a new connection each cycle to avoid
    // holding a long-lived lock across async boundaries.
    let rule_store = match AlertRuleStore::new(db_path) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "watcher cycle: failed to open alert rule store");
            return;
        }
    };

    let rules = match rule_store.list_enabled() {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, "watcher cycle: failed to load enabled rules");
            return;
        }
    };

    if rules.is_empty() {
        tracing::debug!("watcher cycle: no enabled rules");
        return;
    }

    // Evaluate rules concurrently via spawned tasks.
    let mut handles = Vec::with_capacity(rules.len());
    for rule in rules {
        let db_path = db_path.to_path_buf();
        let obligation_store = Arc::clone(obligation_store);

        let handle = tokio::spawn(async move {
            evaluate_rule(&rule, &db_path, &obligation_store).await;
        });
        handles.push(handle);
    }

    // Wait for all watchers — errors are already logged inside.
    for handle in handles {
        if let Err(e) = handle.await {
            tracing::warn!(error = %e, "watcher task panicked");
        }
    }
}

/// Evaluate a single rule and store an obligation if it fires.
async fn evaluate_rule(
    rule: &AlertRule,
    db_path: &std::path::Path,
    obligation_store: &Arc<Mutex<ObligationStore>>,
) {
    let maybe_new_ob = match rule.rule_type {
        AlertRuleType::DeployFailure => {
            deploy_watcher::DeployWatcher.evaluate(rule).await
        }
        AlertRuleType::SentrySpike => {
            sentry_watcher::SentryWatcher.evaluate(rule).await
        }
        AlertRuleType::StaleTicket => {
            stale_ticket_watcher::StaleTicketWatcher.evaluate(rule).await
        }
        AlertRuleType::HaAnomaly => {
            ha_watcher::HaWatcher.evaluate(rule).await
        }
    };

    let Some(new_ob) = maybe_new_ob else {
        tracing::debug!(rule = %rule.name, "rule evaluated: no obligation");
        return;
    };

    // Store the obligation
    let store_result = match obligation_store.lock() {
        Ok(store) => store.create(new_ob),
        Err(e) => {
            tracing::warn!(rule = %rule.name, error = %e, "obligation store mutex poisoned");
            return;
        }
    };

    match store_result {
        Ok(ob) => {
            tracing::info!(
                rule = %rule.name,
                obligation_id = %ob.id,
                priority = ob.priority,
                "watcher created obligation"
            );
        }
        Err(e) => {
            tracing::warn!(rule = %rule.name, error = %e, "failed to store watcher obligation");
            return;
        }
    }

    // Touch last_triggered_at — open a fresh store connection for this write.
    if let Ok(rs) = AlertRuleStore::new(db_path) {
        if let Err(e) = rs.touch_triggered(&rule.name) {
            tracing::warn!(rule = %rule.name, error = %e, "failed to touch last_triggered_at");
        }
    }
}
