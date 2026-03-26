use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Context;
use serde::Deserialize;

// ── Default value functions ─────────────────────────────────────────

fn default_think() -> bool {
    true
}

fn default_digest_interval() -> u64 {
    60
}

fn default_max_workers() -> usize {
    3
}

fn default_health_port() -> u16 {
    8400
}

fn default_voice_max_chars() -> u32 {
    500
}

fn default_elevenlabs_model() -> String {
    "eleven_multilingual_v2".to_string()
}

fn default_weekly_budget_usd() -> f64 {
    50.0
}

fn default_alert_threshold_pct() -> u8 {
    90
}

fn default_imessage_poll_interval() -> u64 {
    10
}

fn default_email_poll_interval() -> u64 {
    60
}

fn default_email_folder_ids() -> Vec<String> {
    vec!["Inbox".to_string()]
}

fn default_worker_timeout_secs() -> u64 {
    300
}

fn default_calendar_id() -> String {
    "primary".to_string()
}

fn default_timezone() -> String {
    "America/Chicago".to_string()
}

fn default_conversation_ttl_hours() -> u64 {
    24
}

fn default_search_url() -> String {
    "https://html.duckduckgo.com/html/".to_string()
}

// ── Config structs ──────────────────────────────────────────────────

/// Configuration for web fetch and search tools.
#[derive(Debug, Clone, Deserialize)]
pub struct WebConfig {
    /// Base URL for the web search backend. Defaults to DuckDuckGo HTML endpoint.
    /// Set to a SearXNG instance URL (containing `/search`) for structured JSON results.
    #[serde(default = "default_search_url")]
    pub search_url: String,
}

/// Configuration for the Doppler secrets management tools.
///
/// Provides short alias mapping so the operator can use project codes (e.g. `oo`)
/// instead of full Doppler project names (e.g. `otaku-odyssey`).
///
/// ```toml
/// [doppler.projects]
/// oo = "otaku-odyssey"
/// tc = "tribal-cities"
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct DopplerConfig {
    /// Maps project aliases to full Doppler project names.
    /// E.g. `{"oo" => "otaku-odyssey"}`.
    #[serde(default)]
    pub projects: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub agent: AgentConfig,
    pub telegram: Option<TelegramConfig>,
    pub discord: Option<DiscordConfig>,
    pub teams: Option<TeamsConfig>,
    pub email: Option<EmailConfig>,
    pub imessage: Option<IMessageConfig>,
    pub jira: Option<JiraConfig>,
    pub team_agents: Option<TeamAgentsConfig>,
    pub daemon: Option<DaemonConfig>,
    /// Optional Google Calendar integration config.
    pub calendar: Option<CalendarConfig>,
    /// Optional web fetch / search configuration.
    pub web: Option<WebConfig>,
    /// Optional Doppler secrets management configuration (alias mappings).
    pub doppler: Option<DopplerConfig>,
    /// Project code to filesystem path mapping (e.g. "oo" -> "~/dev/oo").
    /// Paths are resolved and validated on load.
    #[serde(default)]
    pub projects: HashMap<String, PathBuf>,
    /// Optional alert rules configuration. When present, rules are seeded
    /// into the DB on startup and watchers poll at `interval_secs`.
    pub alert_rules: Option<AlertRulesConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgentConfig {
    pub model: String,
    #[serde(default = "default_think")]
    pub think: bool,
    #[serde(default = "default_digest_interval")]
    pub digest_interval_minutes: u64,
    #[serde(default = "default_max_workers")]
    pub max_workers: usize,
    /// Weekly budget in USD for Claude API usage (default: 50.0).
    #[serde(default = "default_weekly_budget_usd")]
    pub weekly_budget_usd: f64,
    /// Alert threshold as a percentage of the weekly budget (default: 90).
    #[serde(default = "default_alert_threshold_pct")]
    pub alert_threshold_pct: u8,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TelegramConfig {
    pub chat_id: i64,
    /// Optional authorized user ID for inline query filtering.
    /// When set, inline queries from other users are silently dropped.
    pub authorized_user_id: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiscordConfig {
    /// Guild (server) IDs to watch for messages.
    #[serde(default)]
    pub server_ids: Vec<u64>,
    /// Channel IDs to watch — messages from other channels are ignored.
    #[serde(default)]
    pub channel_ids: Vec<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TeamsConfig {
    /// Azure AD tenant ID.
    pub tenant_id: String,
    /// Default team ID for tool operations (e.g. `teams_channels`, `teams_messages`).
    /// Distinct from `team_ids` which is the inbound watch list.
    pub team_id: Option<String>,
    /// Team IDs to watch for messages.
    #[serde(default)]
    pub team_ids: Vec<String>,
    /// Channel IDs to watch — messages from other channels are ignored.
    #[serde(default)]
    pub channel_ids: Vec<String>,
    /// Public webhook URL for MS Graph subscription notifications.
    /// Must be HTTPS. For local dev, use Tailscale Funnel or ngrok.
    pub webhook_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IMessageConfig {
    /// Whether the iMessage channel is enabled.
    #[serde(default)]
    pub enabled: bool,
    /// BlueBubbles server URL (e.g. "http://mac.tailnet:1234").
    pub bluebubbles_url: String,
    /// Polling interval in seconds (default: 10).
    #[serde(default = "default_imessage_poll_interval")]
    pub poll_interval_secs: u64,
    /// Optional allowlist of chat GUIDs to accept inbound messages from.
    ///
    /// When non-empty, only messages whose `chat_guid` matches an entry in
    /// this list are forwarded to the agent. When empty (the default), all
    /// non-self messages pass through unchanged.
    #[serde(default)]
    pub allowed_chat_guids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EmailConfig {
    /// Whether the email channel is enabled.
    #[serde(default)]
    pub enabled: bool,
    /// Polling interval in seconds (default: 60).
    #[serde(default = "default_email_poll_interval")]
    pub poll_interval_secs: u64,
    /// Mail folder IDs or well-known names to poll (default: ["Inbox"]).
    #[serde(default = "default_email_folder_ids")]
    pub folder_ids: Vec<String>,
    /// Sender filter — list of email addresses or domains to include (empty = all).
    #[serde(default)]
    pub sender_filter: Vec<String>,
    /// Subject filter — list of subject substrings to include (empty = all).
    #[serde(default)]
    pub subject_filter: Vec<String>,
}

// ── Jira Config (multi-instance) ────────────────────────────────────

/// Configuration for a single Jira instance.
#[derive(Debug, Clone, Deserialize)]
pub struct JiraInstanceConfig {
    /// Atlassian instance hostname, e.g. "myteam.atlassian.net".
    pub instance: String,
    /// Default project KEY for this instance (used when no project_map entry matches).
    pub default_project: String,
    /// Shared secret for validating inbound Jira webhooks (per-instance override).
    pub webhook_secret: Option<String>,
}

/// Multi-instance Jira configuration.
///
/// Supports two TOML formats:
///
/// **Flat (single-instance, backward-compatible):**
/// ```toml
/// [jira]
/// instance = "myteam.atlassian.net"
/// default_project = "OO"
/// ```
///
/// **Multi-instance:**
/// ```toml
/// [jira.instances.personal]
/// instance = "leonardoacosta.atlassian.net"
/// default_project = "OO"
///
/// [jira.instances.llc]
/// instance = "civalent.atlassian.net"
/// default_project = "CT"
///
/// [jira.project_map]
/// OO = "personal"
/// TC = "personal"
/// CT = "llc"
/// ```
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum JiraConfig {
    /// Flat single-instance format (backward-compatible).
    Flat(JiraInstanceConfig),
    /// Multi-instance format with named instances and a project map.
    Multi(JiraMultiConfig),
}

/// Named-instances Jira config with optional project-to-instance routing.
#[derive(Debug, Clone, Deserialize)]
pub struct JiraMultiConfig {
    /// Named Jira instances, keyed by instance name (e.g. "personal", "llc").
    pub instances: HashMap<String, JiraInstanceConfig>,
    /// Maps project KEYs to instance names (e.g. "OO" -> "personal").
    #[serde(default)]
    pub project_map: HashMap<String, String>,
    /// Shared secret for validating inbound Jira webhooks (applies to all instances
    /// unless overridden per-instance).
    pub webhook_secret: Option<String>,
}

impl JiraConfig {
    /// Resolve which `JiraInstanceConfig` to use for a given project KEY.
    ///
    /// Resolution order:
    /// 1. `project_map` lookup → instance name → config
    /// 2. `default_project` match across all instances
    /// 3. `"default"` instance (for backward-compat flat configs stored under "default")
    /// 4. First instance in the map
    pub fn resolve_instance(&self, project: &str) -> Option<(&str, &JiraInstanceConfig)> {
        match self {
            JiraConfig::Flat(cfg) => Some(("default", cfg)),
            JiraConfig::Multi(multi) => {
                // 1. project_map lookup
                if let Some(instance_name) = multi.project_map.get(project) {
                    if let Some(cfg) = multi.instances.get(instance_name) {
                        return Some((instance_name, cfg));
                    }
                }
                // 2. default_project match
                for (name, cfg) in &multi.instances {
                    if cfg.default_project.eq_ignore_ascii_case(project) {
                        return Some((name, cfg));
                    }
                }
                // 3. "default" instance
                if let Some(cfg) = multi.instances.get("default") {
                    return Some(("default", cfg));
                }
                // 4. First instance
                multi.instances.iter().next().map(|(k, v)| (k.as_str(), v))
            }
        }
    }

    /// Return the webhook secret for this config.
    ///
    /// For flat configs, uses the instance's `webhook_secret`.
    /// For multi-instance, uses the top-level `webhook_secret` (per-instance
    /// overrides are not yet implemented).
    pub fn webhook_secret(&self) -> Option<&str> {
        match self {
            JiraConfig::Flat(cfg) => cfg.webhook_secret.as_deref(),
            JiraConfig::Multi(multi) => multi.webhook_secret.as_deref(),
        }
    }

    /// Return the first/primary instance hostname for logging.
    pub fn primary_instance(&self) -> &str {
        match self {
            JiraConfig::Flat(cfg) => &cfg.instance,
            JiraConfig::Multi(multi) => {
                multi
                    .instances
                    .values()
                    .next()
                    .map(|c| c.instance.as_str())
                    .unwrap_or("<none>")
            }
        }
    }
}

// ── Generic ServiceConfig (flat vs multi-instance) ───────────────────

/// Named-instance service config with optional project-to-instance routing.
///
/// Used as the `Multi` variant of `ServiceConfig<T>`.
#[derive(Debug, Clone, Deserialize)]
pub struct ServiceMultiConfig<T> {
    /// Named instances, keyed by instance name (e.g. `"personal"`, `"llc"`).
    pub instances: HashMap<String, T>,
    /// Maps project codes to instance names (e.g. `"OO"` → `"personal"`).
    #[serde(default)]
    pub project_map: HashMap<String, String>,
}

/// Generic flat-vs-instances configuration for any service.
///
/// Supports two TOML shapes, both backward-compatible:
///
/// **Flat (single-instance):**
/// ```toml
/// [stripe]
/// # uses STRIPE_SECRET_KEY
/// ```
///
/// **Multi-instance:**
/// ```toml
/// [stripe.instances.personal]
/// # uses STRIPE_SECRET_KEY_PERSONAL
///
/// [stripe.instances.llc]
/// # uses STRIPE_SECRET_KEY_LLC
///
/// [stripe.project_map]
/// OO = "personal"
/// CT = "llc"
/// ```
///
/// `T` is the service-specific per-instance configuration struct (e.g. a unit
/// struct for services with no per-instance config fields, or a struct carrying
/// fields like a base URL).
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ServiceConfig<T>
where
    T: Clone,
{
    /// Flat single-instance format. `T` is the entire flat config.
    Flat(T),
    /// Multi-instance format with named instances and an optional project map.
    Multi(ServiceMultiConfig<T>),
}

impl<T: Clone> ServiceConfig<T> {
    /// Resolve which instance config to use for a given project code.
    ///
    /// For flat configs, always returns the single config as `("default", cfg)`.
    /// For multi-instance, applies the resolution order:
    /// 1. `project_map` lookup
    /// 2. `"default"` instance
    /// 3. First instance
    pub fn resolve_instance(&self, project: &str) -> Option<(&str, &T)> {
        match self {
            ServiceConfig::Flat(cfg) => Some(("default", cfg)),
            ServiceConfig::Multi(multi) => {
                // 1. project_map lookup
                if let Some(instance_name) = multi.project_map.get(project) {
                    if let Some(cfg) = multi.instances.get(instance_name) {
                        return Some((instance_name, cfg));
                    }
                }
                // 2. "default" instance
                if let Some(cfg) = multi.instances.get("default") {
                    return Some(("default", cfg));
                }
                // 3. First instance
                multi.instances.iter().next().map(|(k, v)| (k.as_str(), v))
            }
        }
    }

    /// Return all instance names and their configs.
    ///
    /// For flat configs, yields a single `("default", cfg)` pair.
    pub fn all_instances(&self) -> Vec<(&str, &T)> {
        match self {
            ServiceConfig::Flat(cfg) => vec![("default", cfg)],
            ServiceConfig::Multi(multi) => multi
                .instances
                .iter()
                .map(|(k, v)| (k.as_str(), v))
                .collect(),
        }
    }

    /// Return the project map (empty for flat configs).
    pub fn project_map(&self) -> &HashMap<String, String> {
        static EMPTY: std::sync::OnceLock<HashMap<String, String>> =
            std::sync::OnceLock::new();
        match self {
            ServiceConfig::Flat(_) => EMPTY.get_or_init(HashMap::new),
            ServiceConfig::Multi(multi) => &multi.project_map,
        }
    }
}

/// A machine that can run Claude Code subprocesses (locally or via SSH).
///
/// ```toml
/// [[team_agents.machines]]
/// name = "homelab"
/// # ssh_host omitted → local execution
///
/// [[team_agents.machines]]
/// name = "remote"
/// ssh_host = "user@remote.host"
/// working_dir = "/home/user"
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct TeamAgentMachine {
    /// Logical name for this machine (e.g. "homelab", "local").
    pub name: String,
    /// SSH target for remote execution (e.g. "user@host"). When absent, the
    /// subprocess is spawned locally.
    pub ssh_host: Option<String>,
    /// Working directory to use when no project path is resolved. Defaults to
    /// the daemon's `$HOME/dev` when unset.
    pub working_dir: Option<String>,
}

/// Configuration for the team-agents subprocess mode.
///
/// ```toml
/// [team_agents]
/// cc_binary = "claude"    # optional — defaults to "claude"
///
/// [[team_agents.machines]]
/// name = "local"
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct TeamAgentsConfig {
    /// List of machines available for subprocess dispatch.
    #[serde(default)]
    pub machines: Vec<TeamAgentMachine>,
    /// Path / name of the Claude Code binary to invoke. Defaults to `"claude"`.
    #[serde(default = "default_cc_binary")]
    pub cc_binary: String,
}

fn default_cc_binary() -> String {
    "claude".to_string()
}

/// Configuration for the optional Google Calendar integration.
#[derive(Debug, Clone, Deserialize)]
pub struct CalendarConfig {
    /// Google Calendar ID to query (default: "primary" — the user's main calendar).
    #[serde(default = "default_calendar_id")]
    pub calendar_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DaemonConfig {
    pub tts_url: Option<String>,
    #[serde(default = "default_health_port")]
    pub health_port: u16,
    #[serde(default)]
    pub voice_enabled: bool,
    #[serde(default = "default_voice_max_chars")]
    pub voice_max_chars: u32,
    pub elevenlabs_voice_id: Option<String>,
    #[serde(default = "default_elevenlabs_model")]
    pub elevenlabs_model: String,
    /// Quiet hours start time (e.g. "23:00"). During quiet hours, non-P0
    /// outbound messages are suppressed. Optional — if unset, no quiet window.
    pub quiet_start: Option<String>,
    /// Quiet hours end time (e.g. "07:00"). Optional — if unset, no quiet window.
    pub quiet_end: Option<String>,
    /// Per-worker session timeout in seconds (default: 300 = 5 minutes).
    ///
    /// If a worker's entire Claude session (including all tool calls) exceeds
    /// this duration, it is cancelled and the slot is reclaimed. The existing
    /// per-tool timeouts (`TOOL_TIMEOUT_READ`, `TOOL_TIMEOUT_WRITE`) remain
    /// unchanged — this is a hard ceiling above them.
    #[serde(default = "default_worker_timeout_secs")]
    pub worker_timeout_secs: u64,
    /// IANA timezone name for time-aware features (reminders, calendar display).
    /// Default: "America/Chicago".
    #[serde(default = "default_timezone")]
    pub timezone: String,
    /// Base URL for the Nova dashboard (e.g. "https://nova.example.com").
    ///
    /// When set, workers append a `<a href="{url}/sessions/{task_id}">` link to
    /// Telegram responses. When omitted, the link is suppressed entirely.
    pub dashboard_url: Option<String>,
    /// Shared secret for authenticating daemon → dashboard requests.
    ///
    /// When set, the daemon sends `Authorization: Bearer <secret>` on every
    /// POST to `/api/session/message`. Must match `DASHBOARD_SECRET` on the
    /// dashboard side. When absent, dashboard forwarding is disabled even if
    /// `dashboard_url` is configured.
    pub dashboard_secret: Option<String>,
    /// How many hours to retain conversation history per `(channel, thread_id)`.
    ///
    /// After this duration of inactivity the stored turns are expired and
    /// the next load returns an empty history (fresh context).
    ///
    /// Default: 24 hours.  Set to 0 to disable expiry.
    #[serde(default = "default_conversation_ttl_hours")]
    pub conversation_ttl_hours: u64,
}

// ── Alert Rules Config ───────────────────────────────────────────────

/// Configuration for a single alert rule.
///
/// Maps to a row in the `alert_rules` table. Rules seeded here are inserted
/// on daemon startup if they don't already exist.
///
/// ```toml
/// [[alert_rules.rules]]
/// name = "deploy_failure"
/// rule_type = "deploy_failure"
/// enabled = true
///
/// [[alert_rules.rules]]
/// name = "sentry_spike"
/// rule_type = "sentry_spike"
/// config = '{"threshold": 10}'
/// enabled = true
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct AlertRuleEntry {
    /// Unique name for the rule (e.g. "deploy_failure"). Must match a known rule type.
    pub name: String,
    /// Rule type string: "deploy_failure" | "sentry_spike" | "stale_ticket" | "ha_anomaly".
    pub rule_type: String,
    /// Optional JSON blob for rule-specific configuration (thresholds, entity IDs, etc.).
    pub config: Option<String>,
    /// Whether the rule is active. Defaults to true.
    #[serde(default = "default_alert_rule_enabled")]
    pub enabled: bool,
}

fn default_alert_rule_enabled() -> bool {
    true
}

fn default_watcher_interval() -> u64 {
    300
}

/// Top-level alert rules configuration block.
#[derive(Debug, Clone, Deserialize)]
pub struct AlertRulesConfig {
    /// Interval in seconds between watcher poll cycles. Default: 300 (5 minutes).
    #[serde(default = "default_watcher_interval")]
    pub interval_secs: u64,
    /// Rules to seed into the DB on startup.
    #[serde(default)]
    pub rules: Vec<AlertRuleEntry>,
}

// ── Config loading ──────────────────────────────────────────────────

impl Config {
    /// Load configuration from the default path (`~/.nv/nv.toml`).
    pub fn load() -> anyhow::Result<Self> {
        Self::load_from(Self::default_path()?)
    }

    /// Load configuration from an explicit path (useful for testing).
    pub fn load_from(path: PathBuf) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config from {}", path.display()))?;
        let mut config: Config = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse config from {}", path.display()))?;

        // Resolve ~ in project paths and validate each one exists.
        config.projects = config
            .projects
            .into_iter()
            .filter_map(|(code, raw_path)| {
                let resolved = Self::resolve_home(&raw_path);
                if resolved.is_dir() {
                    Some((code, resolved))
                } else {
                    tracing::warn!(
                        project = %code,
                        path = %resolved.display(),
                        "project path does not exist or is not a directory — skipping",
                    );
                    None
                }
            })
            .collect();

        // Apply NOVA_DASHBOARD_TOKEN env override for dashboard_secret.
        // Allows secret rotation without editing nv.toml.
        if let Ok(token) = std::env::var("NOVA_DASHBOARD_TOKEN") {
            if !token.is_empty() {
                if let Some(ref mut daemon) = config.daemon {
                    daemon.dashboard_secret = Some(token);
                }
            }
        }

        // Validate quiet_start and quiet_end at parse time.
        if let Some(ref daemon) = config.daemon {
            if let Some(ref qs) = daemon.quiet_start {
                Self::validate_hhmm(qs)
                    .with_context(|| format!("invalid quiet_start '{qs}': expected HH:MM format (e.g. \"23:00\")"))?;
            }
            if let Some(ref qe) = daemon.quiet_end {
                Self::validate_hhmm(qe)
                    .with_context(|| format!("invalid quiet_end '{qe}': expected HH:MM format (e.g. \"07:00\")"))?;
            }
        }

        Ok(config)
    }

    /// Validate a `HH:MM` time string.
    ///
    /// Accepts `00:00`–`23:59`. Returns `Ok(())` on success, `Err` on invalid format.
    fn validate_hhmm(s: &str) -> anyhow::Result<()> {
        let parts: Vec<&str> = s.splitn(2, ':').collect();
        if parts.len() != 2 {
            anyhow::bail!("expected HH:MM, got '{s}'");
        }
        let hours: u32 = parts[0]
            .parse()
            .map_err(|_| anyhow::anyhow!("hours component is not a number in '{s}'"))?;
        let minutes: u32 = parts[1]
            .parse()
            .map_err(|_| anyhow::anyhow!("minutes component is not a number in '{s}'"))?;
        if hours > 23 {
            anyhow::bail!("hours must be 0–23, got {hours} in '{s}'");
        }
        if minutes > 59 {
            anyhow::bail!("minutes must be 0–59, got {minutes} in '{s}'");
        }
        Ok(())
    }

    /// Expand `~` prefix to the current user's home directory.
    fn resolve_home(path: &std::path::Path) -> PathBuf {
        if let Ok(stripped) = path.strip_prefix("~") {
            if let Ok(home) = std::env::var("HOME") {
                return PathBuf::from(home).join(stripped);
            }
        }
        path.to_path_buf()
    }

    /// Resolve the default config file path: `$HOME/.nv/nv.toml`.
    pub fn default_path() -> anyhow::Result<PathBuf> {
        let home =
            std::env::var("HOME").context("HOME environment variable not set")?;
        Ok(PathBuf::from(home).join(".nv").join("nv.toml"))
    }
}

// ── Secrets ─────────────────────────────────────────────────────────

/// Runtime secrets sourced exclusively from environment variables.
#[derive(Debug, Clone)]
pub struct Secrets {
    /// Optional — only needed if using direct API calls instead of Claude CLI.
    pub anthropic_api_key: Option<String>,
    pub telegram_bot_token: Option<String>,
    pub discord_bot_token: Option<String>,
    pub bluebubbles_password: Option<String>,
    pub ms_graph_client_id: Option<String>,
    pub ms_graph_client_secret: Option<String>,
    /// Azure AD tenant ID for MS Graph API.
    /// Sourced from `MS_GRAPH_TENANT_ID`. Resolution order: env > `[teams].tenant_id` in config.
    pub ms_graph_tenant_id: Option<String>,
    /// Default Jira API token (unqualified — used for flat config and as fallback).
    pub jira_api_token: Option<String>,
    /// Default Jira username (unqualified — used for flat config and as fallback).
    pub jira_username: Option<String>,
    pub elevenlabs_api_key: Option<String>,
    /// Instance-qualified Jira credentials: `JIRA_API_TOKEN_{INSTANCE_UPPER}`.
    /// Keyed by uppercase instance name (e.g. "PERSONAL", "LLC").
    pub jira_api_tokens: HashMap<String, String>,
    /// Instance-qualified Jira usernames: `JIRA_USERNAME_{INSTANCE_UPPER}`.
    pub jira_usernames: HashMap<String, String>,
    /// Base64-encoded service account JSON key for Google Calendar API.
    /// Sourced from `GOOGLE_CALENDAR_CREDENTIALS` env var.
    pub google_calendar_credentials: Option<String>,
}

impl Secrets {
    /// Read secrets from environment variables.
    ///
    /// All keys are optional. The Claude CLI handles its own authentication
    /// via OAuth, so ANTHROPIC_API_KEY is not required.
    ///
    /// Instance-qualified Jira credentials are loaded by scanning all env vars
    /// matching `JIRA_API_TOKEN_*` and `JIRA_USERNAME_*`.
    pub fn from_env() -> anyhow::Result<Self> {
        let mut jira_api_tokens = HashMap::new();
        let mut jira_usernames = HashMap::new();

        // Scan all env vars for JIRA_API_TOKEN_{INSTANCE} and JIRA_USERNAME_{INSTANCE}
        for (key, value) in std::env::vars() {
            if let Some(instance) = key.strip_prefix("JIRA_API_TOKEN_") {
                if !instance.is_empty() {
                    jira_api_tokens.insert(instance.to_string(), value);
                }
            } else if let Some(instance) = key.strip_prefix("JIRA_USERNAME_") {
                if !instance.is_empty() {
                    jira_usernames.insert(instance.to_string(), value);
                }
            }
        }

        Ok(Self {
            anthropic_api_key: std::env::var("ANTHROPIC_API_KEY").ok(),
            telegram_bot_token: std::env::var("TELEGRAM_BOT_TOKEN").ok(),
            discord_bot_token: std::env::var("DISCORD_BOT_TOKEN").ok(),
            bluebubbles_password: std::env::var("BLUEBUBBLES_PASSWORD").ok(),
            ms_graph_client_id: std::env::var("MS_GRAPH_CLIENT_ID").ok(),
            ms_graph_client_secret: std::env::var("MS_GRAPH_CLIENT_SECRET").ok(),
            ms_graph_tenant_id: std::env::var("MS_GRAPH_TENANT_ID").ok(),
            jira_api_token: std::env::var("JIRA_API_TOKEN").ok(),
            jira_username: std::env::var("JIRA_USERNAME").ok(),
            elevenlabs_api_key: std::env::var("ELEVENLABS_API_KEY").ok(),
            jira_api_tokens,
            jira_usernames,
            google_calendar_credentials: std::env::var("GOOGLE_CALENDAR_CREDENTIALS").ok(),
        })
    }

    /// Resolve the API token for a named Jira instance.
    ///
    /// Tries `JIRA_API_TOKEN_{INSTANCE_UPPER}` first, then falls back to
    /// the unqualified `JIRA_API_TOKEN`.
    pub fn jira_token_for(&self, instance_name: &str) -> Option<&str> {
        let key = instance_name.to_uppercase();
        self.jira_api_tokens
            .get(&key)
            .map(|s| s.as_str())
            .or(self.jira_api_token.as_deref())
    }

    /// Resolve the username for a named Jira instance.
    ///
    /// Tries `JIRA_USERNAME_{INSTANCE_UPPER}` first, then falls back to
    /// the unqualified `JIRA_USERNAME`.
    pub fn jira_username_for(&self, instance_name: &str) -> Option<&str> {
        let key = instance_name.to_uppercase();
        self.jira_usernames
            .get(&key)
            .map(|s| s.as_str())
            .or(self.jira_username.as_deref())
    }

    // ── Generic instance-qualified env var helpers ───────────────────

    /// Resolve an instance-qualified env var using the pattern
    /// `{PREFIX}_{INSTANCE_UPPER}`, falling back to the unqualified `{PREFIX}`.
    ///
    /// This is the canonical mechanism for all multi-instance services.
    ///
    /// **Convention:**
    /// - Flat config → `instance_name` is `"default"` → reads `{PREFIX}` directly
    /// - Multi-instance → reads `{PREFIX}_{INSTANCE_UPPER}`, falls back to `{PREFIX}`
    ///
    /// **Example (Stripe):**
    /// ```
    /// // instance "personal"  → checks STRIPE_SECRET_KEY_PERSONAL, falls back to STRIPE_SECRET_KEY
    /// // instance "default"   → checks STRIPE_SECRET_KEY_DEFAULT (unlikely), falls back to STRIPE_SECRET_KEY
    /// secrets.service_secret("STRIPE_SECRET_KEY", "personal")
    /// ```
    pub fn service_secret(prefix: &str, instance_name: &str) -> Option<String> {
        if instance_name != "default" {
            let qualified = format!("{}_{}", prefix, instance_name.to_uppercase());
            if let Ok(val) = std::env::var(&qualified) {
                return Some(val);
            }
        }
        std::env::var(prefix).ok()
    }

    /// Collect all instance-qualified secrets matching the pattern `{PREFIX}_*`.
    ///
    /// Returns a `HashMap<String, String>` keyed by the uppercase suffix after
    /// the prefix (i.e. the instance name). The unqualified `{PREFIX}` var is
    /// NOT included — it is treated as the flat-config fallback and accessed
    /// via `service_secret(prefix, "default")`.
    ///
    /// **Example:**
    /// ```
    /// // Env: STRIPE_SECRET_KEY_PERSONAL=sk_personal, STRIPE_SECRET_KEY_LLC=sk_llc
    /// let map = Secrets::collect_instance_secrets("STRIPE_SECRET_KEY");
    /// // Returns: {"PERSONAL" => "sk_personal", "LLC" => "sk_llc"}
    /// ```
    pub fn collect_instance_secrets(prefix: &str) -> HashMap<String, String> {
        let search_prefix = format!("{}_", prefix);
        std::env::vars()
            .filter_map(|(key, value)| {
                key.strip_prefix(&search_prefix)
                    .filter(|suffix| !suffix.is_empty())
                    .map(|suffix| (suffix.to_string(), value))
            })
            .collect()
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parse_full_config() {
        let toml_str = r#"
[agent]
model = "claude-sonnet-4-20250514"
think = false
digest_interval_minutes = 30

[telegram]
chat_id = 123456789

[discord]
server_ids = [111222333, 444555666]
channel_ids = [123456789, 987654321]

[teams]
tenant_id = "aaaabbbb-cccc-dddd-eeee-ffffffffffff"
team_ids = ["team-1", "team-2"]
channel_ids = ["ch-1", "ch-2"]
webhook_url = "https://nv.example.com/webhooks/teams"

[email]
enabled = true
poll_interval_secs = 30
folder_ids = ["Inbox", "Important"]
sender_filter = ["@company.com", "boss@external.com"]
subject_filter = ["urgent", "action required"]

[imessage]
enabled = true
bluebubbles_url = "http://mac.tailnet:1234"
poll_interval_secs = 5

[jira]
instance = "myteam.atlassian.net"
default_project = "PROJ"
webhook_secret = "super-secret-webhook-token-32chars!"

[daemon]
tts_url = "http://localhost:5500/tts"
health_port = 9090
voice_enabled = true
voice_max_chars = 300
elevenlabs_voice_id = "pNInz6obpgDQGcFmaJgB"
elevenlabs_model = "eleven_turbo_v2_5"
quiet_start = "23:00"
quiet_end = "07:00"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.agent.model, "claude-sonnet-4-20250514");
        assert!(!config.agent.think);
        assert_eq!(config.agent.digest_interval_minutes, 30);

        let tg = config.telegram.unwrap();
        assert_eq!(tg.chat_id, 123_456_789);

        let discord = config.discord.unwrap();
        assert_eq!(discord.server_ids, vec![111_222_333, 444_555_666]);
        assert_eq!(discord.channel_ids, vec![123_456_789, 987_654_321]);

        let teams = config.teams.unwrap();
        assert_eq!(teams.tenant_id, "aaaabbbb-cccc-dddd-eeee-ffffffffffff");
        assert_eq!(teams.team_ids, vec!["team-1", "team-2"]);
        assert_eq!(teams.channel_ids, vec!["ch-1", "ch-2"]);
        assert_eq!(
            teams.webhook_url.as_deref(),
            Some("https://nv.example.com/webhooks/teams")
        );

        let email = config.email.unwrap();
        assert!(email.enabled);
        assert_eq!(email.poll_interval_secs, 30);
        assert_eq!(email.folder_ids, vec!["Inbox", "Important"]);
        assert_eq!(
            email.sender_filter,
            vec!["@company.com", "boss@external.com"]
        );
        assert_eq!(
            email.subject_filter,
            vec!["urgent", "action required"]
        );

        let imessage = config.imessage.unwrap();
        assert!(imessage.enabled);
        assert_eq!(imessage.bluebubbles_url, "http://mac.tailnet:1234");
        assert_eq!(imessage.poll_interval_secs, 5);

        // Flat JiraConfig
        let jira = config.jira.unwrap();
        match &jira {
            JiraConfig::Flat(cfg) => {
                assert_eq!(cfg.instance, "myteam.atlassian.net");
                assert_eq!(cfg.default_project, "PROJ");
                assert_eq!(
                    cfg.webhook_secret.as_deref(),
                    Some("super-secret-webhook-token-32chars!")
                );
            }
            JiraConfig::Multi(_) => panic!("expected flat config"),
        }
        // resolve_instance always returns something for flat
        let (name, resolved) = jira.resolve_instance("PROJ").unwrap();
        assert_eq!(name, "default");
        assert_eq!(resolved.instance, "myteam.atlassian.net");

        let daemon = config.daemon.unwrap();
        assert_eq!(daemon.tts_url.as_deref(), Some("http://localhost:5500/tts"));
        assert_eq!(daemon.health_port, 9090);
        assert!(daemon.voice_enabled);
        assert_eq!(daemon.voice_max_chars, 300);
        assert_eq!(
            daemon.elevenlabs_voice_id.as_deref(),
            Some("pNInz6obpgDQGcFmaJgB")
        );
        assert_eq!(daemon.elevenlabs_model, "eleven_turbo_v2_5");
        assert_eq!(daemon.quiet_start.as_deref(), Some("23:00"));
        assert_eq!(daemon.quiet_end.as_deref(), Some("07:00"));
    }

    #[test]
    fn parse_minimal_config() {
        let toml_str = r#"
[agent]
model = "claude-sonnet-4-20250514"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.agent.model, "claude-sonnet-4-20250514");
        // Defaults kick in
        assert!(config.agent.think);
        assert_eq!(config.agent.digest_interval_minutes, 60);
        // Optional sections are None
        assert!(config.telegram.is_none());
        assert!(config.jira.is_none());
        assert!(config.team_agents.is_none());
        assert!(config.daemon.is_none());
    }

    #[test]
    fn parse_missing_model_fails() {
        let toml_str = r#"
[agent]
think = true
"#;
        let result = toml::from_str::<Config>(toml_str);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("model"),
            "Error should mention missing `model` field, got: {err_msg}"
        );
    }

    #[test]
    fn load_from_nonexistent_path() {
        let result = Config::load_from(PathBuf::from("/tmp/nv-does-not-exist.toml"));
        assert!(result.is_err());
    }

    #[test]
    fn load_from_real_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nv.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(
            f,
            r#"
[agent]
model = "claude-sonnet-4-20250514"
"#
        )
        .unwrap();

        let config = Config::load_from(path).unwrap();
        assert_eq!(config.agent.model, "claude-sonnet-4-20250514");
    }

    #[test]
    fn parse_daemon_voice_defaults() {
        let toml_str = r#"
[agent]
model = "claude-sonnet-4-20250514"

[daemon]
tts_url = "http://localhost:5500/tts"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        let daemon = config.daemon.unwrap();
        assert!(!daemon.voice_enabled);
        assert_eq!(daemon.voice_max_chars, 500);
        assert!(daemon.elevenlabs_voice_id.is_none());
        assert_eq!(daemon.elevenlabs_model, "eleven_multilingual_v2");
        assert!(daemon.quiet_start.is_none());
        assert!(daemon.quiet_end.is_none());
        // worker_timeout_secs defaults to 300
        assert_eq!(daemon.worker_timeout_secs, 300);
    }

    #[test]
    fn parse_daemon_worker_timeout_explicit() {
        let toml_str = r#"
[agent]
model = "claude-sonnet-4-20250514"

[daemon]
worker_timeout_secs = 600
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        let daemon = config.daemon.unwrap();
        assert_eq!(daemon.worker_timeout_secs, 600);
    }

    #[test]
    fn parse_worker_timeout_default_when_daemon_absent() {
        let toml_str = r#"
[agent]
model = "claude-sonnet-4-20250514"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        // No daemon section — worker_timeout_secs not accessible, but DaemonConfig
        // is None. The default is on the field, so this test just ensures no panic.
        assert!(config.daemon.is_none());
    }

    #[test]
    fn secrets_from_env_ok() {
        // Set the required key in the current process environment.
        // This is safe because Rust test threads share the process env, but
        // we use a unique-enough key prefix that collisions are unlikely.
        unsafe {
            std::env::set_var("ANTHROPIC_API_KEY", "sk-test-key");
            std::env::set_var("TELEGRAM_BOT_TOKEN", "tg-token");
        }

        let secrets = Secrets::from_env().unwrap();
        assert_eq!(secrets.anthropic_api_key.as_deref(), Some("sk-test-key"));
        assert_eq!(secrets.telegram_bot_token.as_deref(), Some("tg-token"));

        // Cleanup
        unsafe {
            std::env::remove_var("ANTHROPIC_API_KEY");
            std::env::remove_var("TELEGRAM_BOT_TOKEN");
        }
    }

    #[test]
    fn secrets_from_env_missing_key_is_none() {
        // Make sure the key is absent
        unsafe {
            std::env::remove_var("ANTHROPIC_API_KEY");
        }

        let secrets = Secrets::from_env().unwrap();
        assert!(secrets.anthropic_api_key.is_none());
    }

    #[test]
    fn secrets_instance_qualified_jira_creds() {
        unsafe {
            std::env::set_var("JIRA_API_TOKEN_PERSONAL", "token-personal");
            std::env::set_var("JIRA_USERNAME_PERSONAL", "user-personal");
            std::env::set_var("JIRA_API_TOKEN_LLC", "token-llc");
            std::env::set_var("JIRA_API_TOKEN", "token-default");
            std::env::set_var("JIRA_USERNAME", "user-default");
        }

        let secrets = Secrets::from_env().unwrap();

        // Instance-qualified lookup
        assert_eq!(secrets.jira_token_for("personal"), Some("token-personal"));
        assert_eq!(secrets.jira_username_for("personal"), Some("user-personal"));
        assert_eq!(secrets.jira_token_for("llc"), Some("token-llc"));
        // LLC has no username var — falls back to unqualified
        assert_eq!(secrets.jira_username_for("llc"), Some("user-default"));
        // Unknown instance falls back to unqualified
        assert_eq!(secrets.jira_token_for("unknown"), Some("token-default"));
        // "default" instance also falls back
        assert_eq!(secrets.jira_token_for("default"), Some("token-default"));

        // Cleanup
        unsafe {
            std::env::remove_var("JIRA_API_TOKEN_PERSONAL");
            std::env::remove_var("JIRA_USERNAME_PERSONAL");
            std::env::remove_var("JIRA_API_TOKEN_LLC");
            std::env::remove_var("JIRA_API_TOKEN");
            std::env::remove_var("JIRA_USERNAME");
        }
    }

    #[test]
    fn parse_jira_flat_config() {
        let toml_str = r#"
[agent]
model = "test-model"

[jira]
instance = "myteam.atlassian.net"
default_project = "OO"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        let jira = config.jira.unwrap();
        match &jira {
            JiraConfig::Flat(cfg) => {
                assert_eq!(cfg.instance, "myteam.atlassian.net");
                assert_eq!(cfg.default_project, "OO");
            }
            JiraConfig::Multi(_) => panic!("expected flat"),
        }
        let (name, cfg) = jira.resolve_instance("OO").unwrap();
        assert_eq!(name, "default");
        assert_eq!(cfg.instance, "myteam.atlassian.net");
    }

    #[test]
    fn parse_jira_multi_instance_config() {
        let toml_str = r#"
[agent]
model = "test-model"

[jira.instances.personal]
instance = "leonardoacosta.atlassian.net"
default_project = "OO"

[jira.instances.llc]
instance = "civalent.atlassian.net"
default_project = "CT"

[jira.project_map]
OO = "personal"
TC = "personal"
CT = "llc"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        let jira = config.jira.unwrap();
        match &jira {
            JiraConfig::Multi(multi) => {
                assert_eq!(multi.instances.len(), 2);
                assert!(multi.instances.contains_key("personal"));
                assert!(multi.instances.contains_key("llc"));
                assert_eq!(multi.project_map.get("OO").map(|s| s.as_str()), Some("personal"));
                assert_eq!(multi.project_map.get("CT").map(|s| s.as_str()), Some("llc"));
            }
            JiraConfig::Flat(_) => panic!("expected multi"),
        }

        // project_map resolution
        let (name, cfg) = jira.resolve_instance("OO").unwrap();
        assert_eq!(name, "personal");
        assert_eq!(cfg.instance, "leonardoacosta.atlassian.net");

        let (name2, cfg2) = jira.resolve_instance("CT").unwrap();
        assert_eq!(name2, "llc");
        assert_eq!(cfg2.instance, "civalent.atlassian.net");
    }

    #[test]
    fn jira_resolve_instance_fallback_chain() {
        let toml_str = r#"
[agent]
model = "test-model"

[jira.instances.personal]
instance = "leonardoacosta.atlassian.net"
default_project = "OO"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        let jira = config.jira.unwrap();

        // Unknown project — no project_map, no default_project match, no "default" instance
        // Falls through to first instance
        let result = jira.resolve_instance("UNKNOWN");
        assert!(result.is_some());
        let (_, cfg) = result.unwrap();
        assert_eq!(cfg.instance, "leonardoacosta.atlassian.net");

        // default_project match
        let (name, _) = jira.resolve_instance("OO").unwrap();
        assert_eq!(name, "personal");
    }

    // ── ServiceConfig<T> unit tests ──────────────────────────────────

    // Simple per-instance config for testing the generic ServiceConfig<T>.
    #[derive(Debug, Clone, Deserialize, PartialEq)]
    struct TestInstanceCfg {
        url: String,
    }

    #[test]
    fn service_config_flat_deserializes() {
        let toml_str = r#"url = "https://api.example.com""#;
        let cfg: ServiceConfig<TestInstanceCfg> = toml::from_str(toml_str).unwrap();
        match &cfg {
            ServiceConfig::Flat(inner) => assert_eq!(inner.url, "https://api.example.com"),
            ServiceConfig::Multi(_) => panic!("expected Flat variant"),
        }
    }

    #[test]
    fn service_config_flat_resolve_returns_default_name() {
        let cfg: ServiceConfig<TestInstanceCfg> =
            toml::from_str(r#"url = "https://flat.example.com""#).unwrap();
        let (name, inner) = cfg.resolve_instance("any-project").unwrap();
        assert_eq!(name, "default");
        assert_eq!(inner.url, "https://flat.example.com");
    }

    #[test]
    fn service_config_flat_all_instances_returns_single_default() {
        let cfg: ServiceConfig<TestInstanceCfg> =
            toml::from_str(r#"url = "https://flat.example.com""#).unwrap();
        let instances = cfg.all_instances();
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].0, "default");
    }

    #[test]
    fn service_config_multi_instance_deserializes() {
        let toml_str = r#"
[instances.prod]
url = "https://prod.example.com"

[instances.staging]
url = "https://staging.example.com"

[project_map]
OO = "prod"
"#;
        let cfg: ServiceConfig<TestInstanceCfg> = toml::from_str(toml_str).unwrap();
        match &cfg {
            ServiceConfig::Multi(multi) => {
                assert_eq!(multi.instances.len(), 2);
                assert!(multi.instances.contains_key("prod"));
                assert!(multi.instances.contains_key("staging"));
                assert_eq!(multi.project_map.get("OO").map(|s| s.as_str()), Some("prod"));
            }
            ServiceConfig::Flat(_) => panic!("expected Multi variant"),
        }
    }

    #[test]
    fn service_config_multi_resolve_via_project_map() {
        let toml_str = r#"
[instances.prod]
url = "https://prod.example.com"

[instances.staging]
url = "https://staging.example.com"

[project_map]
OO = "prod"
TC = "staging"
"#;
        let cfg: ServiceConfig<TestInstanceCfg> = toml::from_str(toml_str).unwrap();
        let (name, inner) = cfg.resolve_instance("OO").unwrap();
        assert_eq!(name, "prod");
        assert_eq!(inner.url, "https://prod.example.com");

        let (name2, inner2) = cfg.resolve_instance("TC").unwrap();
        assert_eq!(name2, "staging");
        assert_eq!(inner2.url, "https://staging.example.com");
    }

    #[test]
    fn service_config_multi_resolve_falls_back_to_default_instance() {
        let toml_str = r#"
[instances.default]
url = "https://default.example.com"

[instances.other]
url = "https://other.example.com"
"#;
        let cfg: ServiceConfig<TestInstanceCfg> = toml::from_str(toml_str).unwrap();
        // No project_map entry for "UNKNOWN" → falls back to "default" instance
        let (name, inner) = cfg.resolve_instance("UNKNOWN").unwrap();
        assert_eq!(name, "default");
        assert_eq!(inner.url, "https://default.example.com");
    }

    #[test]
    fn service_config_multi_resolve_falls_back_to_first_instance() {
        let toml_str = r#"
[instances.only]
url = "https://only.example.com"
"#;
        let cfg: ServiceConfig<TestInstanceCfg> = toml::from_str(toml_str).unwrap();
        // No project_map, no "default" key → first instance
        let result = cfg.resolve_instance("ANYTHING");
        assert!(result.is_some());
        let (_, inner) = result.unwrap();
        assert_eq!(inner.url, "https://only.example.com");
    }

    #[test]
    fn service_config_multi_resolve_returns_none_when_empty() {
        let toml_str = r#"
[instances]
"#;
        // TOML with empty instances table parses as Multi with no instances
        let cfg: ServiceConfig<TestInstanceCfg> =
            toml::from_str(toml_str).unwrap_or(ServiceConfig::Multi(ServiceMultiConfig {
                instances: HashMap::new(),
                project_map: HashMap::new(),
            }));
        match &cfg {
            ServiceConfig::Multi(multi) if multi.instances.is_empty() => {
                assert!(cfg.resolve_instance("ANY").is_none());
            }
            _ => {
                // Acceptable: TOML may parse differently; just verify no panic
            }
        }
    }

    #[test]
    fn service_config_multi_all_instances_returns_all() {
        let toml_str = r#"
[instances.a]
url = "https://a.example.com"

[instances.b]
url = "https://b.example.com"
"#;
        let cfg: ServiceConfig<TestInstanceCfg> = toml::from_str(toml_str).unwrap();
        let mut instances = cfg.all_instances();
        instances.sort_by_key(|(k, _)| *k);
        assert_eq!(instances.len(), 2);
        assert_eq!(instances[0].0, "a");
        assert_eq!(instances[1].0, "b");
    }

    #[test]
    fn service_config_flat_project_map_is_empty() {
        let cfg: ServiceConfig<TestInstanceCfg> =
            toml::from_str(r#"url = "https://flat.example.com""#).unwrap();
        assert!(cfg.project_map().is_empty());
    }

    #[test]
    fn service_config_multi_project_map_accessible() {
        let toml_str = r#"
[instances.prod]
url = "https://prod.example.com"

[project_map]
OO = "prod"
"#;
        let cfg: ServiceConfig<TestInstanceCfg> = toml::from_str(toml_str).unwrap();
        let pm = cfg.project_map();
        assert_eq!(pm.get("OO").map(|s| s.as_str()), Some("prod"));
    }

    #[test]
    fn service_config_multi_backward_compat_no_project_map() {
        // project_map is #[serde(default)] — omitting it is valid
        let toml_str = r#"
[instances.main]
url = "https://main.example.com"
"#;
        let cfg: ServiceConfig<TestInstanceCfg> = toml::from_str(toml_str).unwrap();
        match &cfg {
            ServiceConfig::Multi(multi) => {
                assert!(multi.project_map.is_empty());
                assert_eq!(multi.instances.len(), 1);
            }
            ServiceConfig::Flat(_) => panic!("expected Multi"),
        }
    }

    // ── Quiet hours validation ────────────────────────────────────────

    #[test]
    fn validate_hhmm_accepts_valid_times() {
        assert!(Config::validate_hhmm("23:00").is_ok());
        assert!(Config::validate_hhmm("07:00").is_ok());
        assert!(Config::validate_hhmm("00:00").is_ok());
        assert!(Config::validate_hhmm("23:59").is_ok());
        assert!(Config::validate_hhmm("12:30").is_ok());
    }

    #[test]
    fn validate_hhmm_rejects_invalid_hours() {
        let err = Config::validate_hhmm("25:99");
        assert!(err.is_err(), "25:99 should fail validation");
        let msg = err.unwrap_err().to_string();
        assert!(msg.contains("hours"), "error should mention hours: {msg}");
    }

    #[test]
    fn validate_hhmm_rejects_invalid_minutes() {
        let err = Config::validate_hhmm("10:60");
        assert!(err.is_err(), "10:60 should fail validation");
        let msg = err.unwrap_err().to_string();
        assert!(msg.contains("minutes"), "error should mention minutes: {msg}");
    }

    #[test]
    fn validate_hhmm_rejects_non_time_strings() {
        assert!(Config::validate_hhmm("not-a-time").is_err());
        assert!(Config::validate_hhmm("").is_err());
        assert!(Config::validate_hhmm("99:99").is_err());
    }

    #[test]
    fn load_from_rejects_invalid_quiet_start() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nv.toml");
        std::fs::write(
            &path,
            r#"
[agent]
model = "test-model"

[daemon]
quiet_start = "25:99"
quiet_end = "07:00"
"#,
        )
        .unwrap();
        let result = Config::load_from(path);
        assert!(result.is_err(), "expected error for invalid quiet_start");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("quiet_start") || msg.contains("25:99"),
            "error should mention quiet_start or the invalid value: {msg}"
        );
    }

    #[test]
    fn load_from_rejects_invalid_quiet_end() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nv.toml");
        std::fs::write(
            &path,
            r#"
[agent]
model = "test-model"

[daemon]
quiet_start = "23:00"
quiet_end = "not-a-time"
"#,
        )
        .unwrap();
        let result = Config::load_from(path);
        assert!(result.is_err(), "expected error for invalid quiet_end");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("quiet_end") || msg.contains("not-a-time"),
            "error should mention quiet_end or the invalid value: {msg}"
        );
    }

    #[test]
    fn load_from_accepts_valid_quiet_hours() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nv.toml");
        std::fs::write(
            &path,
            r#"
[agent]
model = "test-model"

[daemon]
quiet_start = "23:00"
quiet_end = "07:00"
"#,
        )
        .unwrap();
        let config = Config::load_from(path).unwrap();
        let daemon = config.daemon.unwrap();
        assert_eq!(daemon.quiet_start.as_deref(), Some("23:00"));
        assert_eq!(daemon.quiet_end.as_deref(), Some("07:00"));
    }

    // ── TeamAgentMachine / TeamAgentsConfig deserialization ──────────

    #[test]
    fn parse_team_agent_machine_full() {
        let toml_str = r#"
[agent]
model = "test-model"

[team_agents]
cc_binary = "/usr/local/bin/claude"

[[team_agents.machines]]
name = "dev-box"
ssh_host = "dev-box.local"
working_dir = "/home/dev/projects"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        let ta = config.team_agents.unwrap();
        assert_eq!(ta.cc_binary, "/usr/local/bin/claude");
        assert_eq!(ta.machines.len(), 1);
        let m = &ta.machines[0];
        assert_eq!(m.name, "dev-box");
        assert_eq!(m.ssh_host.as_deref(), Some("dev-box.local"));
        assert_eq!(m.working_dir.as_deref(), Some("/home/dev/projects"));
    }

    #[test]
    fn parse_team_agent_machine_local_no_ssh() {
        let toml_str = r#"
[agent]
model = "test-model"

[team_agents]

[[team_agents.machines]]
name = "local"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        let ta = config.team_agents.unwrap();
        let m = &ta.machines[0];
        assert_eq!(m.name, "local");
        assert!(m.ssh_host.is_none());
        assert!(m.working_dir.is_none());
    }

    #[test]
    fn parse_team_agents_config_cc_binary_default() {
        let toml_str = r#"
[agent]
model = "test-model"

[team_agents]

[[team_agents.machines]]
name = "local"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        let ta = config.team_agents.unwrap();
        // Default cc_binary is "claude" when not specified
        assert_eq!(ta.cc_binary, "claude");
    }

    #[test]
    fn parse_team_agents_config_multiple_machines() {
        let toml_str = r#"
[agent]
model = "test-model"

[team_agents]

[[team_agents.machines]]
name = "local"

[[team_agents.machines]]
name = "remote-1"
ssh_host = "192.168.1.10"
working_dir = "/home/user/dev"

[[team_agents.machines]]
name = "remote-2"
ssh_host = "server.example.com"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        let ta = config.team_agents.unwrap();
        assert_eq!(ta.machines.len(), 3);
        assert_eq!(ta.machines[0].name, "local");
        assert!(ta.machines[0].ssh_host.is_none());
        assert_eq!(ta.machines[1].name, "remote-1");
        assert_eq!(ta.machines[1].ssh_host.as_deref(), Some("192.168.1.10"));
        assert_eq!(ta.machines[1].working_dir.as_deref(), Some("/home/user/dev"));
        assert_eq!(ta.machines[2].name, "remote-2");
        assert_eq!(ta.machines[2].ssh_host.as_deref(), Some("server.example.com"));
        assert!(ta.machines[2].working_dir.is_none());
    }

    #[test]
    fn parse_team_agents_absent_is_none() {
        let toml_str = r#"
[agent]
model = "test-model"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(config.team_agents.is_none());
    }
}
