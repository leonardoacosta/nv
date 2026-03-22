mod agent;
mod claude;
mod digest;
mod health;
mod http;
mod jira;
mod memory;
mod nexus;
mod query;
mod scheduler;
mod shutdown;
#[allow(dead_code)]
mod state;
mod telegram;
mod tools;

use std::collections::HashMap;
use std::sync::Arc;

use nv_core::types::Trigger;
use nv_core::{Config, Secrets};
use tokio::sync::mpsc;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

use agent::AgentLoop;
use claude::ClaudeClient;
use health::{ChannelStatus, HealthState};
use telegram::{run_poll_loop, TelegramChannel};

// ── Log Rotation ────────────────────────────────────────────────────

/// Initialize tracing with both stdout and daily rolling file appender.
///
/// Returns the non-blocking writer guard which must be held for the
/// lifetime of the program (dropping it flushes and stops the writer).
fn init_tracing(log_dir: &std::path::Path) -> tracing_appender::non_blocking::WorkerGuard {
    // Create log directory if it doesn't exist
    if let Err(e) = std::fs::create_dir_all(log_dir) {
        eprintln!("WARNING: failed to create log directory {}: {e}", log_dir.display());
    }

    let file_appender = tracing_appender::rolling::daily(log_dir, "nv");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stdout))
        .with(tracing_subscriber::fmt::layer().with_writer(non_blocking).with_ansi(false))
        .init();

    guard
}

/// Remove old log files, keeping only the most recent `keep` files.
fn cleanup_old_logs(log_dir: &std::path::Path, keep: usize) {
    let entries = match std::fs::read_dir(log_dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    let mut log_files: Vec<(std::path::PathBuf, std::time::SystemTime)> = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_str()
                .map(|n| n.starts_with("nv."))
                .unwrap_or(false)
        })
        .filter_map(|e| {
            e.metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .map(|t| (e.path(), t))
        })
        .collect();

    if log_files.len() <= keep {
        return;
    }

    // Sort newest first
    log_files.sort_by(|a, b| b.1.cmp(&a.1));

    let to_remove = &log_files[keep..];
    let count = to_remove.len();
    for (path, _) in to_remove {
        if let Err(e) = std::fs::remove_file(path) {
            tracing::warn!(path = %path.display(), error = %e, "failed to remove old log file");
        }
    }

    if count > 0 {
        tracing::info!(removed = count, kept = keep, "cleaned up old log files");
    }
}

// ── sd-notify Helpers ───────────────────────────────────────────────

/// Send sd_notify READY=1 (non-fatal if not running under systemd).
fn notify_ready() {
    if let Err(e) = sd_notify::notify(false, &[sd_notify::NotifyState::Ready]) {
        tracing::debug!(error = %e, "sd_notify READY failed (not running under systemd?)");
    }
}

/// Send sd_notify STOPPING=1.
fn notify_stopping() {
    if let Err(e) = sd_notify::notify(false, &[sd_notify::NotifyState::Stopping]) {
        tracing::debug!(error = %e, "sd_notify STOPPING failed");
    }
}

/// Spawn a watchdog task that pings systemd every 30 seconds.
fn spawn_watchdog(health: Arc<HealthState>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            let resp = health.to_health_response().await;
            if resp.status == "ok" {
                if let Err(e) =
                    sd_notify::notify(false, &[sd_notify::NotifyState::Watchdog])
                {
                    tracing::debug!(error = %e, "sd_notify WATCHDOG failed");
                }
            } else {
                tracing::warn!("health check failed, skipping watchdog ping");
            }
        }
    });
}

// ── Main ────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Resolve paths early
    let home = std::env::var("HOME").expect("HOME not set");
    let nv_base = std::path::PathBuf::from(&home).join(".nv");
    let log_dir = nv_base.join("logs");

    // Initialize tracing with file + stdout
    let _log_guard = init_tracing(&log_dir);

    // Clean up old log files (keep 5)
    cleanup_old_logs(&log_dir, 5);

    tracing::info!("NV daemon starting");

    let config = Config::load()?;
    let secrets = Secrets::from_env()?;

    // Initialize memory and state directories
    let mem = memory::Memory::new(&nv_base);
    mem.init()?;
    let st = state::State::new(&nv_base);
    st.init()?;

    // Create shared health state
    let health_state = Arc::new(HealthState::new());

    // Create unbounded trigger channel -- listeners produce, agent loop consumes
    let (trigger_tx, trigger_rx) = mpsc::unbounded_channel::<Trigger>();

    // Channel registry for outbound message routing
    let mut channels: HashMap<String, Arc<dyn nv_core::channel::Channel>> = HashMap::new();

    // Start Telegram channel if configured
    if let (Some(tg_config), Some(bot_token)) = (&config.telegram, &secrets.telegram_bot_token) {
        // Create a separate sender for the poll loop (bounded for backpressure on poll side)
        let (poll_tx, mut poll_rx) = mpsc::channel::<Trigger>(256);

        let mut tg_channel =
            TelegramChannel::new(bot_token, tg_config.chat_id, poll_tx);

        // Validate bot token at startup
        use nv_core::channel::Channel;
        tg_channel.connect().await?;

        health_state
            .update_channel("telegram", ChannelStatus::Connected)
            .await;

        // Create an Arc-wrapped channel for the registry (for sending outbound messages)
        let tg_for_registry = Arc::new(TelegramChannel::new(
            bot_token,
            tg_config.chat_id,
            // Dummy sender -- not used for outbound, only the client matters
            mpsc::channel::<Trigger>(1).0,
        ));
        channels.insert("telegram".into(), tg_for_registry);

        // Forward triggers from poll channel to the unbounded agent channel
        let agent_tx = trigger_tx.clone();
        tokio::spawn(async move {
            while let Some(trigger) = poll_rx.recv().await {
                if agent_tx.send(trigger).is_err() {
                    tracing::error!("agent trigger channel closed");
                    break;
                }
            }
        });

        tokio::spawn(async move {
            run_poll_loop(tg_channel).await;
        });
        tracing::info!("Telegram channel started");
    }

    // Create Jira client if configured
    let jira_client = if let (Some(jira_config), Some(username), Some(token)) = (
        &config.jira,
        &secrets.jira_username,
        &secrets.jira_api_token,
    ) {
        let instance_url = format!("https://{}", jira_config.instance);
        tracing::info!(instance = %jira_config.instance, "Jira client configured");
        Some(jira::JiraClient::new(&instance_url, username, token))
    } else {
        tracing::warn!("Jira not configured -- jira tools disabled");
        None
    };

    // Create and connect NexusClient if configured
    let nexus_client = if let Some(nexus_config) = &config.nexus {
        let client = nexus::client::NexusClient::new(&nexus_config.agents);
        client.connect_all().await;

        // Update health state for each Nexus agent
        for agent in &nexus_config.agents {
            let status = if client.is_connected().await {
                ChannelStatus::Connected
            } else {
                ChannelStatus::Disconnected
            };
            health_state
                .update_channel(format!("nexus_{}", agent.name), status)
                .await;
        }

        if client.is_connected().await {
            // Spawn event stream listeners for all connected agents
            nexus::stream::spawn_event_streams(&client.agents, trigger_tx.clone());
            tracing::info!("Nexus event streams started");
        } else {
            tracing::warn!("Nexus configured but no agents reachable");
        }

        Some(client)
    } else {
        tracing::info!("Nexus not configured -- nexus tools disabled");
        None
    };

    // Spawn the cron scheduler for periodic digests
    let _scheduler_handle = scheduler::spawn_scheduler(
        trigger_tx.clone(),
        config.agent.digest_interval_minutes,
        &nv_base,
    );
    tracing::info!(
        interval_minutes = config.agent.digest_interval_minutes,
        "digest scheduler started"
    );

    // Spawn the HTTP server for CLI triggers (POST /digest, GET /health, etc.)
    let health_port = config
        .daemon
        .as_ref()
        .map(|d| d.health_port)
        .unwrap_or(8400);
    let http_tx = trigger_tx.clone();
    let http_health = Arc::clone(&health_state);
    tokio::spawn(async move {
        if let Err(e) = http::run_http_server(health_port, http_tx, http_health).await {
            tracing::error!(error = %e, "HTTP server failed");
        }
    });
    tracing::info!(port = health_port, "HTTP server started");

    // Drop the original sender so the channel closes when all listener senders are dropped
    drop(trigger_tx);

    // Create Claude API client
    let client = ClaudeClient::new(
        secrets.anthropic_api_key.clone(),
        config.agent.model.clone(),
        4096, // max_tokens
    );

    // All components are running -- signal systemd that we're ready
    notify_ready();
    tracing::info!("sd_notify READY sent");

    // Spawn the watchdog task for systemd health monitoring
    spawn_watchdog(Arc::clone(&health_state));

    // Create and run the agent loop
    let agent = AgentLoop::new(
        config.agent.clone(),
        client,
        trigger_rx,
        channels,
        nv_base,
        jira_client,
        nexus_client,
    );

    tracing::info!("starting agent loop");

    // Run the agent loop alongside the shutdown signal listener.
    // The agent loop exits when its trigger channel closes (all senders dropped).
    // On shutdown signal, we log the event -- the agent loop will naturally
    // drain and stop since we already dropped the original trigger_tx.
    tokio::select! {
        result = agent.run() => {
            if let Err(e) = result {
                tracing::error!(error = %e, "agent loop failed");
            }
        }
        () = shutdown::wait_for_shutdown_signal() => {
            tracing::info!("shutdown signal received, draining...");
            // The agent loop will stop once all trigger senders are dropped
            // and the channel is drained. Give it a moment to finish.
        }
    }

    // Notify systemd we're stopping
    notify_stopping();
    tracing::info!("NV daemon stopped cleanly");

    Ok(())
}
