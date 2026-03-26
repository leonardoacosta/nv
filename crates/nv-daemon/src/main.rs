mod account;
mod anthropic;
mod contact_store;
mod error_recovery;
mod agent;
mod self_assessment;
mod aggregation;
mod briefing_store;
mod cc_sessions;
mod cold_start_store;
mod alert_rules;
mod bash;
mod dashboard_client;
mod callbacks;
mod channels;
mod claude;
mod conversation;
mod diary;
mod digest;
mod health;
mod http;
mod memory;
mod messages;
mod nexus;
mod obligation_detector;
mod obligation_research;
mod persona;
mod team_agent;
mod obligation_store;
mod orchestrator;
mod query;
mod reminders;
mod proactive_watcher;
mod scheduler;
mod speech_to_text;
mod shutdown;
#[allow(dead_code)]
mod state;
#[allow(dead_code)]
mod tailscale;
mod tool_cache;
mod tools;
mod tts;
mod health_poller;
mod server_health_store;
mod watchers;
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

use channels::telegram::{run_poll_loop, TelegramChannel};
use claude::ClaudeClient;
use health::{ChannelStatus, HealthState};

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
///
/// Also monitors cgroup PID usage to warn before hitting TasksMax.
fn spawn_watchdog(_health: Arc<HealthState>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            // Always send the watchdog ping — the process is alive and functional.
            // A degraded health status (e.g. nexus agent down) does not mean the
            // daemon should be killed; the nexus watchdog handles reconnection
            // independently.  Withholding the ping caused a crash loop when any
            // channel was disconnected (systemd SIGABRT after WatchdogSec=120).
            if let Err(e) =
                sd_notify::notify(false, &[sd_notify::NotifyState::Watchdog])
            {
                tracing::debug!(error = %e, "sd_notify WATCHDOG failed");
            }

            // Check cgroup PID usage — warn if approaching limit
            if let (Ok(current), Ok(max)) = (
                tokio::fs::read_to_string("/proc/self/cgroup")
                    .await
                    .ok()
                    .and_then(|cg| {
                        let slice = cg.lines().next()?.strip_prefix("0::")?;
                        Some(format!("/sys/fs/cgroup{slice}/pids.current"))
                    })
                    .map(std::fs::read_to_string)
                    .unwrap_or(Err(std::io::Error::other("no cgroup"))),
                tokio::fs::read_to_string("/proc/self/cgroup")
                    .await
                    .ok()
                    .and_then(|cg| {
                        let slice = cg.lines().next()?.strip_prefix("0::")?;
                        Some(format!("/sys/fs/cgroup{slice}/pids.max"))
                    })
                    .map(std::fs::read_to_string)
                    .unwrap_or(Err(std::io::Error::other("no cgroup"))),
            ) {
                let cur: u64 = current.trim().parse().unwrap_or(0);
                let mx: u64 = max.trim().parse().unwrap_or(u64::MAX);
                if mx != u64::MAX && cur > mx * 80 / 100 {
                    tracing::warn!(
                        pids_current = cur,
                        pids_max = mx,
                        pct = cur * 100 / mx,
                        "watchdog: cgroup PID usage above 80%"
                    );
                }
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
    let diary_writer_inner = diary::DiaryWriter::new(&nv_base.join("diary"));
    diary_writer_inner.init()?;
    tracing::info!("diary initialized");
    let diary_writer = std::sync::Arc::new(std::sync::Mutex::new(diary_writer_inner));

    // Initialize message store
    let message_store = Arc::new(std::sync::Mutex::new(
        messages::MessageStore::init(&nv_base.join("messages.db"))?,
    ));
    tracing::info!("message store initialized");

    // Initialize obligation store (shares messages.db — migration run by MessageStore above)
    let obligation_store = match obligation_store::ObligationStore::new(&nv_base.join("messages.db")) {
        Ok(store) => {
            tracing::info!("obligation store initialized");
            Some(std::sync::Arc::new(std::sync::Mutex::new(store)))
        }
        Err(e) => {
            tracing::warn!(error = %e, "failed to initialize obligation store — obligation tools disabled");
            None
        }
    };

    // Initialize reminder store (shares messages.db path)
    let reminder_store = match reminders::ReminderStore::new(&nv_base.join("messages.db")) {
        Ok(store) => {
            tracing::info!("reminder store initialized");
            Some(std::sync::Arc::new(std::sync::Mutex::new(store)))
        }
        Err(e) => {
            tracing::warn!(error = %e, "failed to initialize reminder store — reminder tools disabled");
            None
        }
    };

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

        let tg_timezone = config
            .daemon
            .as_ref()
            .map(|d| d.timezone.clone())
            .unwrap_or_else(|| "America/Chicago".to_string());

        let mut tg_channel =
            TelegramChannel::new(bot_token, tg_config.chat_id, poll_tx)
                .with_authorized_user_id(tg_config.authorized_user_id)
                .with_reminder_stores(
                    reminder_store.clone(),
                    obligation_store.clone(),
                    tg_timezone,
                );

        // Validate bot token at startup
        use nv_core::channel::Channel;
        tg_channel.connect().await?;

        health_state
            .update_channel("telegram", ChannelStatus::Connected)
            .await;

        // Create an Arc-wrapped channel for the registry (for sending outbound messages)
        let tg_for_registry = Arc::new(
            TelegramChannel::new(
                bot_token,
                tg_config.chat_id,
                // Dummy sender -- not used for outbound, only the client matters
                mpsc::channel::<Trigger>(1).0,
            )
            .with_authorized_user_id(tg_config.authorized_user_id),
        );
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
            channels::discord::DiscordChannel::new(bot_token, discord_config.clone(), poll_tx);

        // Connect to Discord gateway
        use nv_core::channel::Channel as _;
        discord_channel.connect().await?;

        health_state
            .update_channel("discord", ChannelStatus::Connected)
            .await;

        // Create an Arc-wrapped channel for the registry (for sending outbound messages)
        let discord_for_registry = Arc::new(channels::discord::DiscordChannel::new(
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
            channels::discord::run_poll_loop(discord_channel).await;
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
                channels::imessage::IMessageChannel::new(imessage_config.clone(), bb_password, poll_tx);

            // Validate connectivity at startup
            use nv_core::channel::Channel as _;
            imessage_channel.connect().await?;

            health_state
                .update_channel("imessage", ChannelStatus::Connected)
                .await;

            // Create an Arc-wrapped channel for the registry (for sending outbound messages)
            let imessage_for_registry = Arc::new(channels::imessage::IMessageChannel::new(
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
                channels::imessage::run_poll_loop(imessage_channel).await;
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
        std::sync::Arc<tokio::sync::Mutex<std::collections::VecDeque<channels::teams::types::ChatMessage>>>,
    > = None;
    let mut teams_client_for_http: Option<std::sync::Arc<channels::teams::client::TeamsClient>> = None;
    let mut teams_client_state_for_http: Option<String> = None;

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

        let mut teams_channel = channels::teams::TeamsChannel::new(
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

        // Capture clientState secret for webhook validation in the HTTP handler
        teams_client_state_for_http = Some(teams_channel.client_state.clone());

        // Create an Arc-wrapped TeamsClient for the HTTP webhook handler
        let auth_for_http =
            std::sync::Arc::new(channels::teams::oauth::MsGraphAuth::new(
                &teams_config.tenant_id,
                client_id,
                client_secret,
            ));
        teams_client_for_http = Some(std::sync::Arc::new(channels::teams::client::TeamsClient::new(
            auth_for_http,
        )));

        // Create an Arc-wrapped channel for the registry (for sending outbound messages)
        let teams_for_registry = Arc::new(channels::teams::TeamsChannel::new(
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
            channels::teams::run_poll_loop(teams_channel).await;
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

                let email_auth = Arc::new(channels::teams::oauth::MsGraphAuth::new(
                    &tenant_id,
                    &client_id,
                    &client_secret,
                ));
                let email_client = channels::email::client::EmailClient::new(Arc::clone(&email_auth));

                let mut email_channel =
                    channels::email::EmailChannel::new(email_client, email_config.clone(), poll_tx);

                // Connect (authenticate + initialize last_seen)
                use nv_core::channel::Channel as _;
                email_channel.connect().await?;

                health_state
                    .update_channel("email", ChannelStatus::Connected)
                    .await;

                // Create an Arc-wrapped channel for the registry (for sending outbound messages)
                let email_for_registry_auth = Arc::new(channels::teams::oauth::MsGraphAuth::new(
                    &tenant_id,
                    &client_id,
                    &client_secret,
                ));
                let email_for_registry_client =
                    channels::email::client::EmailClient::new(email_for_registry_auth);
                let email_for_registry = Arc::new(channels::email::EmailChannel::new(
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
                    channels::email::run_poll_loop(email_channel).await;
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
        match tools::jira::JiraRegistry::new(jira_config, &secrets) {
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

    // Create TeamAgentDispatcher if configured.
    let team_agent_dispatcher: Option<team_agent::TeamAgentDispatcher> =
        if let Some(ta_config) = &config.team_agents {
            let dispatcher = team_agent::TeamAgentDispatcher::new(ta_config);
            tracing::info!(
                machines = ta_config.machines.len(),
                cc_binary = %ta_config.cc_binary,
                "TeamAgentDispatcher initialized"
            );
            health_state
                .update_channel("team_agents", ChannelStatus::Connected)
                .await;
            Some(dispatcher)
        } else {
            tracing::info!("team_agents not configured -- team agents disabled");
            None
        };

    // Build CcSessionManager from team_agents config (if configured).
    // Created early so it can be shared with the HTTP server and SharedDeps.
    let cc_session_manager: Option<cc_sessions::CcSessionManager> =
        if let Some(ta_config) = &config.team_agents {
            let dispatcher = team_agent::TeamAgentDispatcher::new(ta_config);
            let mgr = cc_sessions::CcSessionManager::new(dispatcher);
            mgr.spawn_health_monitor();
            tracing::info!("CcSessionManager initialized with health monitor");
            Some(mgr)
        } else {
            None
        };

    // Initialize schedule store (user-defined recurring schedules)
    let schedule_store = match tools::schedule::ScheduleStore::new(&nv_base) {
        Ok(store) => {
            tracing::info!("schedule store initialized");
            Some(std::sync::Arc::new(std::sync::Mutex::new(store)))
        }
        Err(e) => {
            tracing::warn!(error = %e, "failed to initialize schedule store — schedule tools disabled");
            None
        }
    };

    // Initialize cold-start store (shares messages.db — table created lazily by ColdStartStore::new)
    let cold_start_store = match cold_start_store::ColdStartStore::new(&nv_base.join("messages.db")) {
        Ok(store) => {
            tracing::info!("cold-start store initialized");
            Some(std::sync::Arc::new(std::sync::Mutex::new(store)))
        }
        Err(e) => {
            tracing::warn!(error = %e, "failed to initialize cold-start store — cold-start logging disabled");
            None
        }
    };

    // Initialize self-assessment store and engine.
    let (self_assessment_store_arc, self_assessment_engine_arc) = {
        let sa_path = nv_base.join("state/self-assessment.jsonl");
        match self_assessment::SelfAssessmentStore::new(&sa_path) {
            Ok(store) => {
                let store_arc = std::sync::Arc::new(store);
                tracing::info!("self-assessment store initialized");
                if let Some(ref cs_arc) = cold_start_store {
                    let engine = self_assessment::SelfAssessmentEngine::new(
                        std::sync::Arc::clone(cs_arc),
                        std::sync::Arc::clone(&message_store),
                        nv_base.join("diary"),
                        std::sync::Arc::clone(&store_arc),
                    );
                    tracing::info!("self-assessment engine initialized");
                    (Some(store_arc), Some(std::sync::Arc::new(engine)))
                } else {
                    tracing::warn!("self-assessment engine skipped — cold-start store unavailable");
                    (Some(store_arc), None)
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to initialize self-assessment store — self-assessment disabled");
                (None, None)
            }
        }
    };

    // Spawn proactive watchers if alert_rules are configured
    if let Some(ref ar_config) = config.alert_rules {
        // Seed configured rules into the DB (idempotent — INSERT OR IGNORE).
        let db_path_for_seed = nv_base.join("messages.db");
        for entry in &ar_config.rules {
            let rule_type = match alert_rules::AlertRuleType::from_str(&entry.rule_type) {
                Ok(rt) => rt,
                Err(e) => {
                    tracing::warn!(
                        name = %entry.name,
                        rule_type = %entry.rule_type,
                        error = %e,
                        "alert_rules: unknown rule_type, skipping seed"
                    );
                    continue;
                }
            };
            // Open store per-entry to avoid holding a live connection across the seed loop.
            match alert_rules::AlertRuleStore::new(&db_path_for_seed) {
                Ok(store) => {
                    // Only insert if the rule doesn't exist yet (get_by_name returns None).
                    match store.get_by_name(&entry.name) {
                        Ok(None) => {
                            let id = uuid::Uuid::new_v4().to_string();
                            if let Err(e) = store.create(
                                &id,
                                &entry.name,
                                rule_type,
                                entry.config.as_deref(),
                                entry.enabled,
                            ) {
                                tracing::warn!(name = %entry.name, error = %e, "failed to seed alert rule");
                            } else {
                                tracing::info!(name = %entry.name, "alert rule seeded");
                            }
                        }
                        Ok(Some(_)) => {
                            tracing::debug!(name = %entry.name, "alert rule already exists, skipping seed");
                        }
                        Err(e) => {
                            tracing::warn!(name = %entry.name, error = %e, "failed to check existing alert rule");
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "failed to open alert rule store for seed");
                }
            }
        }

        // Spawn watcher loop only if obligation store is available
        if let Some(ref ob_store) = obligation_store {
            let watcher_db = nv_base.join("messages.db");
            let watcher_ob = Arc::clone(ob_store);
            let watcher_interval = ar_config.interval_secs;
            let watcher_handle = watchers::spawn_watchers(watcher_db, watcher_ob, watcher_interval);
            tracing::info!(
                interval_secs = ar_config.interval_secs,
                rules = ar_config.rules.len(),
                "proactive watchers started"
            );

            // Abort the watcher task on shutdown so in-flight cycles are cancelled cleanly.
            // We store the handle in a tokio task that awaits the shutdown signal, then aborts.
            tokio::spawn(async move {
                shutdown::wait_for_shutdown_signal().await;
                tracing::info!("shutdown: aborting watcher task");
                watcher_handle.abort();
            });
        } else {
            tracing::warn!("alert_rules configured but obligation store unavailable — watchers disabled");
        }
    }

    // Spawn 60s health poll loop (always active — no obligation store required).
    {
        let health_poll_db = nv_base.join("messages.db");
        let health_poll_ob = obligation_store.clone();

        if let Some(ob_store) = health_poll_ob {
            health_poller::spawn_health_poller(
                health_poll_db,
                ob_store,
            );
            tracing::info!("health poller started (60s interval)");
        } else {
            tracing::warn!("health poller: no obligation store — crash detection disabled");
        }
    }

    // Spawn the cron scheduler for periodic digests (skip during bootstrap)
    if agent::check_bootstrap_state() {
        let _scheduler_handle = scheduler::spawn_scheduler(
            trigger_tx.clone(),
            config.agent.digest_interval_minutes,
            &nv_base,
            schedule_store.clone(),
            Some(Arc::clone(&message_store)),
        );
        tracing::info!(
            interval_minutes = config.agent.digest_interval_minutes,
            "digest scheduler started"
        );
    } else {
        tracing::info!("bootstrap not complete — digest scheduler deferred");
    }

    // Spawn reminder scheduler (polls SQLite every 30s for due reminders)
    if let Some(ref store) = reminder_store {
        reminders::spawn_reminder_scheduler(store.clone(), channels.clone());
        tracing::info!("reminder scheduler started");
    }

    // Spawn proactive watcher (scans obligations for overdue/stale/approaching-deadline items)
    let pw_enabled = config
        .proactive_watcher
        .as_ref()
        .map(|c| c.enabled)
        .unwrap_or(true);
    if pw_enabled {
        let pw_config = config.proactive_watcher.clone().unwrap_or_default();
        let pw_interval = pw_config.interval_minutes;
        let _pw_handle = proactive_watcher::spawn_proactive_watcher(
            trigger_tx.clone(),
            pw_config,
            &nv_base,
        );
        tracing::info!(
            interval_minutes = pw_interval,
            "proactive watcher started"
        );
    } else {
        tracing::info!("proactive watcher disabled by config");
    }

    // Build Jira webhook state if configured
    let jira_webhook_state = config.jira.as_ref().map(|jira_config| {
        let secret = jira_config.webhook_secret().map(String::from);
        if secret.is_none() {
            tracing::info!("Jira configured but no webhook_secret — Jira webhooks will accept all requests");
        }
        let state = tools::jira::JiraWebhookState {
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

    // Initialize the morning briefing store.
    let briefing_store = Arc::new(briefing_store::BriefingStore::new(&nv_base));
    let briefing_store_for_http = Arc::clone(&briefing_store);
    tracing::info!("briefing store initialized");

    // Initialize contact store (shares messages.db).
    // Two Arc clones are created: one for the HTTP server, one for SharedDeps/workers.
    let contact_store_arc: Option<Arc<contact_store::ContactStore>> =
        match rusqlite::Connection::open(nv_base.join("messages.db")) {
            Ok(conn) => {
                conn.execute_batch("PRAGMA journal_mode=WAL;").ok();
                let cs = contact_store::ContactStore::new(Arc::new(std::sync::Mutex::new(conn)));
                tracing::info!("contact store initialized");
                Some(Arc::new(cs))
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to initialize contact store — contact tools disabled");
                None
            }
        };
    let contact_store_for_http = contact_store_arc.clone();
    let contact_store_for_workers = contact_store_arc;

    // Create the dashboard WebSocket event broadcast channel.
    // Capacity 256: enough for burst events; lagged clients are warned but not disconnected.
    let (http_event_tx, _http_event_rx) = tokio::sync::broadcast::channel::<http::DaemonEvent>(256);

    // Clone teams client before the async move so SharedDeps can hold its own reference.
    let teams_client_for_workers = teams_client_for_http.clone();
    // Clone cold-start store for the HTTP server (shared with workers via SharedDeps).
    let cold_start_store_for_http = cold_start_store.clone();
    // Clone CcSessionManager for the HTTP server (cheaply cloned via Arc).
    let cc_session_manager_for_http = cc_session_manager.clone();
    // Clone diary writer for the HTTP server (shared with workers via SharedDeps).
    let diary_for_http = Arc::clone(&diary_writer);
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
            teams_client_state_for_http,
            Some(briefing_store_for_http),
            cold_start_store_for_http,
            http_event_tx,
            cc_session_manager_for_http,
            contact_store_for_http,
            Some(diary_for_http),
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

    // Open the conversation DB connection (shares messages.db in WAL mode).
    //
    // WAL mode allows concurrent reads from other connections (message_store,
    // obligation_store, etc.) while the conversation store writes.
    let conversation_ttl_hours = config
        .daemon
        .as_ref()
        .map(|d| d.conversation_ttl_hours)
        .unwrap_or(24);

    let conversation_db = {
        let conn = rusqlite::Connection::open(nv_base.join("messages.db"))
            .expect("failed to open conversation DB");
        conn.execute_batch("PRAGMA journal_mode=WAL;")
            .expect("WAL pragma failed");
        // Run the conversations-table migration so the table exists when using
        // a fresh connection that hasn't gone through MessageStore::init().
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS conversations (
                channel     TEXT NOT NULL,
                thread_id   TEXT NOT NULL,
                turns_json  TEXT NOT NULL DEFAULT '[]',
                updated_at  TEXT NOT NULL DEFAULT (datetime('now')),
                PRIMARY KEY (channel, thread_id)
            );
            CREATE INDEX IF NOT EXISTS idx_conversations_updated ON conversations(updated_at);",
        )
        .expect("failed to ensure conversations table");
        std::sync::Arc::new(std::sync::Mutex::new(conn))
    };
    tracing::info!(ttl_hours = conversation_ttl_hours, "conversation DB initialized");

    // Build service registries from environment variables
    let stripe_registry = match tools::stripe::StripeClient::from_env() {
        Ok(client) => {
            tracing::info!("Stripe client initialized");
            Some(tools::ServiceRegistry::single(client))
        }
        Err(e) => {
            tracing::warn!(error = %e, "Stripe not configured — stripe tools use inline auth");
            None
        }
    };

    let vercel_registry = match tools::vercel::VercelClient::from_env() {
        Ok(client) => {
            tracing::info!("Vercel client initialized");
            Some(tools::ServiceRegistry::single(client))
        }
        Err(e) => {
            tracing::warn!(error = %e, "Vercel not configured — vercel tools use inline auth");
            None
        }
    };

    let sentry_registry = match tools::sentry::SentryClient::from_env() {
        Ok(client) => {
            tracing::info!("Sentry client initialized");
            Some(tools::ServiceRegistry::single(client))
        }
        Err(e) => {
            tracing::warn!(error = %e, "Sentry not configured — sentry tools use inline auth");
            None
        }
    };

    let resend_registry = match tools::resend::ResendClient::from_env() {
        Ok(client) => {
            tracing::info!("Resend client initialized");
            Some(tools::ServiceRegistry::single(client))
        }
        Err(e) => {
            tracing::warn!(error = %e, "Resend not configured — resend tools use inline auth");
            None
        }
    };

    let ha_registry = match tools::ha::HAClient::from_env() {
        Ok(client) => {
            tracing::info!("Home Assistant client initialized");
            Some(tools::ServiceRegistry::single(client))
        }
        Err(e) => {
            tracing::warn!(error = %e, "Home Assistant not configured — ha tools use inline auth");
            None
        }
    };

    let upstash_registry = match tools::upstash::UpstashClient::from_env() {
        Ok(client) => {
            tracing::info!("Upstash client initialized");
            Some(tools::ServiceRegistry::single(client))
        }
        Err(e) => {
            tracing::warn!(error = %e, "Upstash not configured — upstash tools use inline auth");
            None
        }
    };

    let ado_registry = match tools::ado::AdoClient::from_env() {
        Ok(client) => {
            tracing::info!("Azure DevOps client initialized");
            Some(tools::ServiceRegistry::single(client))
        }
        Err(e) => {
            tracing::warn!(error = %e, "ADO not configured — ado tools use inline auth");
            None
        }
    };

    let cloudflare_registry = match tools::cloudflare::CloudflareClient::from_env() {
        Ok(client) => {
            tracing::info!("Cloudflare client initialized");
            Some(tools::ServiceRegistry::single(client))
        }
        Err(e) => {
            tracing::warn!(error = %e, "Cloudflare not configured — cloudflare tools use inline auth");
            None
        }
    };

    let doppler_registry = match tools::doppler::DopplerClient::from_env() {
        Ok(client) => {
            tracing::info!("Doppler client initialized");
            Some(tools::ServiceRegistry::single(client))
        }
        Err(e) => {
            tracing::warn!(error = %e, "Doppler not configured — doppler tools use inline auth");
            None
        }
    };

    // Build optional DashboardClient if both url and secret are configured.
    let dashboard_client = match (
        config.daemon.as_ref().and_then(|d| d.dashboard_url.as_deref()),
        config.daemon.as_ref().and_then(|d| d.dashboard_secret.as_deref()),
    ) {
        (Some(url), Some(secret)) => {
            // Redact token: show first 4 chars only (e.g. "tok1..." or "****" if short).
            let redacted = if secret.len() > 4 {
                format!("{}...", &secret[..4])
            } else {
                "****".to_string()
            };
            tracing::info!(
                url = %url,
                token = %redacted,
                "dashboard forwarding enabled"
            );
            match dashboard_client::DashboardClient::new(url, secret) {
                Ok(dc) => {
                    let healthy = dc.ping().await;
                    tracing::info!(
                        url = %url,
                        healthy,
                        "dashboard reachability check complete"
                    );
                    Some(dc)
                }
                Err(e) => {
                    tracing::warn!(error = %e, "failed to build DashboardClient — forwarding disabled");
                    None
                }
            }
        }
        (Some(_), None) => {
            tracing::warn!("dashboard_url configured but dashboard_secret missing — forwarding disabled (cold-start only)");
            None
        }
        _ => {
            tracing::info!("dashboard forwarding not configured — cold-start only");
            None
        }
    };

    // Build shared dependencies for workers
    let shared_deps = Arc::new(worker::SharedDeps {
        memory: memory::Memory::new(&nv_base),
        state: state::State::new(&nv_base),
        message_store: Arc::clone(&message_store),
        conversation_db,
        conversation_ttl_hours,
        diary: Arc::clone(&diary_writer),
        jira_registry,
        team_agent_dispatcher,
        cc_session_manager,
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
        obligation_store,
        schedule_store,
        reminder_store,
        calendar_credentials: secrets.google_calendar_credentials.clone(),
        calendar_id: config
            .calendar
            .as_ref()
            .map(|c| c.calendar_id.clone())
            .unwrap_or_else(|| "primary".to_string()),
        timezone: config
            .daemon
            .as_ref()
            .map(|d| d.timezone.clone())
            .unwrap_or_else(|| "America/Chicago".to_string()),
        health_port: config
            .daemon
            .as_ref()
            .map(|d| d.health_port)
            .unwrap_or(8400),
        stripe_registry,
        vercel_registry,
        sentry_registry,
        resend_registry,
        ha_registry,
        upstash_registry,
        ado_registry,
        cloudflare_registry,
        doppler_registry,
        teams_client: teams_client_for_workers,
        claude_client: client.clone(),
        dashboard_url: config.daemon.as_ref().and_then(|d| d.dashboard_url.clone()),
        dashboard_client,
        briefing_store: Some(Arc::clone(&briefing_store)),
        cold_start_store,
        contact_store: contact_store_for_workers,
        tool_cache: crate::tool_cache::ToolResultCache::new(),
        proactive_watcher_config: config.proactive_watcher.clone(),
        obligation_research_config: config.obligation_research.clone(),
        self_assessment_store: self_assessment_store_arc,
        self_assessment_engine: self_assessment_engine_arc,
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
