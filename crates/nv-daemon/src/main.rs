mod account;
mod agent;
mod aggregation;
mod bash;
mod callbacks;
mod claude;
mod conversation;
mod diary;
mod digest;
mod discord;
mod docker_tools;
mod email;
#[allow(dead_code)]
mod github;
mod ha_tools;
mod ado_tools;
mod plaid_tools;
mod health;
mod imessage;
mod http;
mod jira;
mod memory;
mod messages;
mod neon_tools;
mod nexus;
mod orchestrator;
mod posthog_tools;
mod query;
mod resend_tools;
mod scheduler;
mod sentry_tools;
mod shutdown;
mod stripe_tools;
mod upstash_tools;
#[allow(dead_code)]
mod state;
#[allow(dead_code)]
mod tailscale;
mod teams;
mod telegram;
mod tools;
mod tts;
mod vercel_tools;
mod voice_input;
mod worker;

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use nv_core::types::Trigger;
use nv_core::{Config, Secrets};
use tokio::sync::mpsc;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

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

    // Initialize diary
    let diary_writer = diary::DiaryWriter::new(&nv_base.join("diary"));
    diary_writer.init()?;
    tracing::info!("diary initialized");

    // Initialize message store
    let message_store = messages::MessageStore::init(&nv_base.join("messages.db"))?;
    tracing::info!("message store initialized");

    // Background: refresh account info cache (non-blocking)
    tokio::spawn(async {
        match account::query_account_info().await {
            Ok(info) => {
                tracing::info!(plan = %info.plan, username = %info.username, "account info refreshed");
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to query account info (will use cache or unknown)");
            }
        }
    });

    // Initialize voice/TTS support
    let voice_config_enabled = config
        .daemon
        .as_ref()
        .map(|d| d.voice_enabled)
        .unwrap_or(false);

    let voice_enabled = Arc::new(AtomicBool::new(voice_config_enabled));

    let tts_client = if voice_config_enabled {
        // Check ffmpeg availability
        if !tts::check_ffmpeg().await {
            tracing::warn!("voice_enabled=true but ffmpeg not found in PATH — voice disabled");
            voice_enabled.store(false, std::sync::atomic::Ordering::Relaxed);
            None
        } else if let (Some(api_key), Some(voice_id)) = (
            &secrets.elevenlabs_api_key,
            config.daemon.as_ref().and_then(|d| d.elevenlabs_voice_id.as_deref()),
        ) {
            let model = config
                .daemon
                .as_ref()
                .map(|d| d.elevenlabs_model.as_str())
                .unwrap_or("eleven_multilingual_v2");
            tracing::info!(voice_id, model, "TTS client initialized");
            Some(Arc::new(tts::TtsClient::new(api_key, voice_id, model)))
        } else {
            tracing::warn!(
                "voice_enabled=true but ELEVENLABS_API_KEY or elevenlabs_voice_id missing — voice disabled"
            );
            voice_enabled.store(false, std::sync::atomic::Ordering::Relaxed);
            None
        }
    } else {
        tracing::debug!("voice disabled by config");
        None
    };

    let voice_max_chars = config
        .daemon
        .as_ref()
        .map(|d| d.voice_max_chars)
        .unwrap_or(500);

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

        let poll_voice_enabled = voice_enabled.clone();
        tokio::spawn(async move {
            run_poll_loop(tg_channel, poll_voice_enabled).await;
        });
        tracing::info!("Telegram channel started");
    }

    // Start Discord channel if configured
    if let (Some(discord_config), Some(bot_token)) =
        (&config.discord, &secrets.discord_bot_token)
    {
        let (poll_tx, mut poll_rx) = mpsc::channel::<Trigger>(256);

        let mut discord_channel =
            discord::DiscordChannel::new(bot_token, discord_config.clone(), poll_tx);

        // Connect to Discord gateway
        use nv_core::channel::Channel as _;
        discord_channel.connect().await?;

        health_state
            .update_channel("discord", ChannelStatus::Connected)
            .await;

        // Create an Arc-wrapped channel for the registry (for sending outbound messages)
        let discord_for_registry = Arc::new(discord::DiscordChannel::new(
            bot_token,
            discord_config.clone(),
            // Dummy sender — not used for outbound, only the REST client matters
            mpsc::channel::<Trigger>(1).0,
        ));
        channels.insert("discord".into(), discord_for_registry);

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
            discord::run_poll_loop(discord_channel).await;
        });
        tracing::info!("Discord channel started");
    }

    // Start iMessage channel if configured
    if let (Some(imessage_config), Some(bb_password)) =
        (&config.imessage, &secrets.bluebubbles_password)
    {
        if imessage_config.enabled {
            let (poll_tx, mut poll_rx) = mpsc::channel::<Trigger>(256);

            let mut imessage_channel =
                imessage::IMessageChannel::new(imessage_config.clone(), bb_password, poll_tx);

            // Validate connectivity at startup
            use nv_core::channel::Channel as _;
            imessage_channel.connect().await?;

            health_state
                .update_channel("imessage", ChannelStatus::Connected)
                .await;

            // Create an Arc-wrapped channel for the registry (for sending outbound messages)
            let imessage_for_registry = Arc::new(imessage::IMessageChannel::new(
                imessage_config.clone(),
                bb_password,
                // Dummy sender — not used for outbound, only the BB client matters
                mpsc::channel::<Trigger>(1).0,
            ));
            channels.insert("imessage".into(), imessage_for_registry);

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
                imessage::run_poll_loop(imessage_channel).await;
            });
            tracing::info!(
                url = %imessage_config.bluebubbles_url,
                poll_secs = imessage_config.poll_interval_secs,
                "iMessage channel started"
            );
        }
    }

    // Start Teams channel if configured
    let mut teams_message_buffer: Option<
        std::sync::Arc<tokio::sync::Mutex<std::collections::VecDeque<teams::types::ChatMessage>>>,
    > = None;
    let mut teams_client_for_http: Option<std::sync::Arc<teams::client::TeamsClient>> = None;

    if let (Some(teams_config), Some(client_id), Some(client_secret)) = (
        &config.teams,
        &secrets.ms_graph_client_id,
        &secrets.ms_graph_client_secret,
    ) {
        let webhook_url = teams_config
            .webhook_url
            .clone()
            .unwrap_or_else(|| {
                let port = config
                    .daemon
                    .as_ref()
                    .map(|d| d.health_port)
                    .unwrap_or(8400);
                format!("http://127.0.0.1:{port}/webhooks/teams")
            });

        let (poll_tx, mut poll_rx) = mpsc::channel::<Trigger>(256);

        let mut teams_channel = teams::TeamsChannel::new(
            &teams_config.tenant_id,
            client_id,
            client_secret,
            teams_config.clone(),
            poll_tx,
            webhook_url,
        );

        // Connect to MS Graph (OAuth + subscription registration)
        use nv_core::channel::Channel as _;
        teams_channel.connect().await?;

        health_state
            .update_channel("teams", ChannelStatus::Connected)
            .await;

        // Share the message buffer with the HTTP server for webhook delivery
        let buffer = std::sync::Arc::clone(&teams_channel.message_buffer);
        teams_message_buffer = Some(buffer);

        // Create an Arc-wrapped TeamsClient for the HTTP webhook handler
        let auth_for_http =
            std::sync::Arc::new(teams::oauth::MsGraphAuth::new(
                &teams_config.tenant_id,
                client_id,
                client_secret,
            ));
        teams_client_for_http = Some(std::sync::Arc::new(teams::client::TeamsClient::new(
            auth_for_http,
        )));

        // Create an Arc-wrapped channel for the registry (for sending outbound messages)
        let teams_for_registry = Arc::new(teams::TeamsChannel::new(
            &teams_config.tenant_id,
            client_id,
            client_secret,
            teams_config.clone(),
            // Dummy sender — not used for outbound, only the REST client matters
            mpsc::channel::<Trigger>(1).0,
            String::new(),
        ));
        channels.insert("teams".into(), teams_for_registry);

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
            teams::run_poll_loop(teams_channel).await;
        });
        tracing::info!("Teams channel started");
    }

    // Start Email channel if configured
    // Email reuses MS Graph OAuth — shares auth with Teams if both are configured.
    if let Some(email_config) = &config.email {
        if email_config.enabled {
            let (client_id, client_secret) = match (
                &secrets.ms_graph_client_id,
                &secrets.ms_graph_client_secret,
            ) {
                (Some(id), Some(secret)) => (id.clone(), secret.clone()),
                _ => {
                    tracing::warn!(
                        "Email enabled but MS_GRAPH_CLIENT_ID or MS_GRAPH_CLIENT_SECRET missing — email disabled"
                    );
                    // Skip email setup but don't fail
                    (String::new(), String::new())
                }
            };

            if !client_id.is_empty() {
                // Resolve tenant_id: reuse from teams config if available, else from env
                let tenant_id = config
                    .teams
                    .as_ref()
                    .map(|t| t.tenant_id.clone())
                    .or_else(|| std::env::var("MS_GRAPH_TENANT_ID").ok())
                    .unwrap_or_else(|| {
                        tracing::warn!("No tenant_id for email — using 'common'");
                        "common".to_string()
                    });

                let (poll_tx, mut poll_rx) = mpsc::channel::<Trigger>(256);

                let email_auth = Arc::new(teams::oauth::MsGraphAuth::new(
                    &tenant_id,
                    &client_id,
                    &client_secret,
                ));
                let email_client = email::client::EmailClient::new(Arc::clone(&email_auth));

                let mut email_channel =
                    email::EmailChannel::new(email_client, email_config.clone(), poll_tx);

                // Connect (authenticate + initialize last_seen)
                use nv_core::channel::Channel as _;
                email_channel.connect().await?;

                health_state
                    .update_channel("email", ChannelStatus::Connected)
                    .await;

                // Create an Arc-wrapped channel for the registry (for sending outbound messages)
                let email_for_registry_auth = Arc::new(teams::oauth::MsGraphAuth::new(
                    &tenant_id,
                    &client_id,
                    &client_secret,
                ));
                let email_for_registry_client =
                    email::client::EmailClient::new(email_for_registry_auth);
                let email_for_registry = Arc::new(email::EmailChannel::new(
                    email_for_registry_client,
                    email_config.clone(),
                    // Dummy sender — not used for outbound, only the REST client matters
                    mpsc::channel::<Trigger>(1).0,
                ));
                channels.insert("email".into(), email_for_registry);

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
                    email::run_poll_loop(email_channel).await;
                });
                tracing::info!(
                    folders = ?email_config.folder_ids,
                    poll_secs = email_config.poll_interval_secs,
                    "Email channel started"
                );
            }
        }
    }

    // Build Jira registry if configured
    let jira_registry = if let Some(jira_config) = &config.jira {
        match jira::JiraRegistry::new(jira_config, &secrets) {
            Ok(registry) => registry,
            Err(e) => {
                tracing::error!(error = %e, "Failed to build Jira registry — jira tools disabled");
                None
            }
        }
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

    // Spawn the cron scheduler for periodic digests (skip during bootstrap)
    if agent::check_bootstrap_state() {
        let _scheduler_handle = scheduler::spawn_scheduler(
            trigger_tx.clone(),
            config.agent.digest_interval_minutes,
            &nv_base,
        );
        tracing::info!(
            interval_minutes = config.agent.digest_interval_minutes,
            "digest scheduler started"
        );
    } else {
        tracing::info!("bootstrap not complete — digest scheduler deferred");
    }

    // Build Jira webhook state if configured
    let jira_webhook_state = config.jira.as_ref().map(|jira_config| {
        let secret = jira_config.webhook_secret().map(String::from);
        if secret.is_none() {
            tracing::info!("Jira configured but no webhook_secret — Jira webhooks will accept all requests");
        }
        let state = jira::JiraWebhookState {
            trigger_tx: trigger_tx.clone(),
            webhook_secret: secret,
            memory_base_path: nv_base.join("memory"),
        };
        tracing::info!(
            instance = %jira_config.primary_instance(),
            has_secret = state.webhook_secret.is_some(),
            "Jira webhook endpoint configured"
        );
        Arc::new(state)
    });

    // Spawn the HTTP server for CLI triggers (POST /digest, GET /health, GET /stats, etc.)
    let health_port = config
        .daemon
        .as_ref()
        .map(|d| d.health_port)
        .unwrap_or(8400);
    let http_tx = trigger_tx.clone();
    let http_health = Arc::clone(&health_state);
    let stats_db_path = nv_base.join("messages.db");
    let http_weekly_budget = config.agent.weekly_budget_usd;
    tokio::spawn(async move {
        if let Err(e) = http::run_http_server(
            health_port,
            http_tx,
            http_health,
            stats_db_path,
            teams_message_buffer,
            teams_client_for_http,
            jira_webhook_state,
            http_weekly_budget,
        )
        .await
        {
            tracing::error!(error = %e, "HTTP server failed");
        }
    });
    tracing::info!(port = health_port, "HTTP server started");

    // Drop the original sender so the channel closes when all listener senders are dropped
    drop(trigger_tx);

    // Create Claude CLI client (uses OAuth via claude CLI, no API key needed)
    let client = ClaudeClient::new(
        secrets.anthropic_api_key.clone().unwrap_or_default(),
        config.agent.model.clone(),
        4096, // max_tokens
    );

    // All components are running -- signal systemd that we're ready
    notify_ready();
    tracing::info!("sd_notify READY sent");

    // Spawn the watchdog task for systemd health monitoring
    spawn_watchdog(Arc::clone(&health_state));

    // Create worker event channel for progress tracking
    let (worker_event_tx, worker_event_rx) =
        mpsc::unbounded_channel::<worker::WorkerEvent>();

    // Initialize conversation store
    let conversation_store = conversation::ConversationStore::new();
    tracing::info!("conversation store initialized");

    // Build shared dependencies for workers
    let shared_deps = Arc::new(worker::SharedDeps {
        memory: memory::Memory::new(&nv_base),
        state: state::State::new(&nv_base),
        message_store: Arc::new(std::sync::Mutex::new(message_store)),
        conversation_store: Arc::new(std::sync::Mutex::new(conversation_store)),
        diary: Arc::new(std::sync::Mutex::new(diary_writer)),
        jira_registry,
        nexus_client,
        channels: channels.clone(),
        nv_base_path: nv_base,
        voice_enabled: voice_enabled.clone(),
        tts_client,
        voice_max_chars,
        project_registry: config.projects.clone(),
        event_tx: worker_event_tx,
        weekly_budget_usd: config.agent.weekly_budget_usd,
        alert_threshold_pct: config.agent.alert_threshold_pct,
        worker_timeout_secs: config
            .daemon
            .as_ref()
            .map(|d| d.worker_timeout_secs)
            .unwrap_or(300),
    });

    // Extract Telegram client and chat_id for reactions
    let (tg_reaction_client, tg_reaction_chat_id) = if let Some(tg) = channels.get("telegram") {
        if let Some(tg_channel) = tg.as_any().downcast_ref::<TelegramChannel>() {
            (Some(tg_channel.client.clone()), Some(tg_channel.chat_id))
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };

    // Create worker pool
    let max_workers = config.agent.max_workers;
    let worker_pool = worker::WorkerPool::new(
        max_workers,
        Arc::clone(&shared_deps),
        client,
        tg_reaction_client.clone(),
        tg_reaction_chat_id,
    );

    tracing::info!(max_workers, "worker pool created");

    // Parse quiet hours from config
    let (quiet_start, quiet_end) = config
        .daemon
        .as_ref()
        .map(|d| {
            let start = d.quiet_start.as_deref().and_then(|s| {
                chrono::NaiveTime::parse_from_str(s, "%H:%M")
                    .map_err(|e| tracing::warn!(error = %e, value = s, "invalid quiet_start"))
                    .ok()
            });
            let end = d.quiet_end.as_deref().and_then(|s| {
                chrono::NaiveTime::parse_from_str(s, "%H:%M")
                    .map_err(|e| tracing::warn!(error = %e, value = s, "invalid quiet_end"))
                    .ok()
            });
            (start, end)
        })
        .unwrap_or((None, None));

    // Create and run the orchestrator
    let orchestrator = orchestrator::Orchestrator::new(
        trigger_rx,
        worker_pool,
        channels,
        shared_deps,
        tg_reaction_client,
        tg_reaction_chat_id,
        worker_event_rx,
        quiet_start,
        quiet_end,
    );

    tracing::info!("starting orchestrator");

    // Run the orchestrator alongside the shutdown signal listener.
    // The orchestrator exits when its trigger channel closes (all senders dropped).
    // On shutdown signal, we log the event -- the orchestrator will naturally
    // drain and stop since we already dropped the original trigger_tx.
    tokio::select! {
        result = orchestrator.run() => {
            if let Err(e) = result {
                tracing::error!(error = %e, "orchestrator failed");
            }
        }
        () = shutdown::wait_for_shutdown_signal() => {
            tracing::info!("shutdown signal received, draining...");
            // The orchestrator will stop once all trigger senders are dropped
            // and the channel is drained. Give it a moment to finish.
        }
    }

    // Notify systemd we're stopping
    notify_stopping();
    tracing::info!("NV daemon stopped cleanly");

    Ok(())
}
