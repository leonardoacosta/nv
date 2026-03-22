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

fn default_health_port() -> u16 {
    8400
}

fn default_voice_max_chars() -> u32 {
    500
}

fn default_elevenlabs_model() -> String {
    "eleven_multilingual_v2".to_string()
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

// ── Config structs ──────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub agent: AgentConfig,
    pub telegram: Option<TelegramConfig>,
    pub discord: Option<DiscordConfig>,
    pub teams: Option<TeamsConfig>,
    pub email: Option<EmailConfig>,
    pub imessage: Option<IMessageConfig>,
    pub jira: Option<JiraConfig>,
    pub nexus: Option<NexusConfig>,
    pub daemon: Option<DaemonConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgentConfig {
    pub model: String,
    #[serde(default = "default_think")]
    pub think: bool,
    #[serde(default = "default_digest_interval")]
    pub digest_interval_minutes: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TelegramConfig {
    pub chat_id: i64,
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

#[derive(Debug, Clone, Deserialize)]
pub struct JiraConfig {
    pub instance: String,
    pub default_project: String,
    /// Shared secret for validating inbound Jira webhooks.
    pub webhook_secret: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NexusAgent {
    pub name: String,
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NexusConfig {
    pub agents: Vec<NexusAgent>,
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
        let config: Config = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse config from {}", path.display()))?;
        Ok(config)
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
    pub jira_api_token: Option<String>,
    pub jira_username: Option<String>,
    pub elevenlabs_api_key: Option<String>,
}

impl Secrets {
    /// Read secrets from environment variables.
    ///
    /// All keys are optional. The Claude CLI handles its own authentication
    /// via OAuth, so ANTHROPIC_API_KEY is not required.
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            anthropic_api_key: std::env::var("ANTHROPIC_API_KEY").ok(),
            telegram_bot_token: std::env::var("TELEGRAM_BOT_TOKEN").ok(),
            discord_bot_token: std::env::var("DISCORD_BOT_TOKEN").ok(),
            bluebubbles_password: std::env::var("BLUEBUBBLES_PASSWORD").ok(),
            ms_graph_client_id: std::env::var("MS_GRAPH_CLIENT_ID").ok(),
            ms_graph_client_secret: std::env::var("MS_GRAPH_CLIENT_SECRET").ok(),
            jira_api_token: std::env::var("JIRA_API_TOKEN").ok(),
            jira_username: std::env::var("JIRA_USERNAME").ok(),
            elevenlabs_api_key: std::env::var("ELEVENLABS_API_KEY").ok(),
        })
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

[nexus]
[[nexus.agents]]
name = "builder"
host = "127.0.0.1"
port = 9000

[daemon]
tts_url = "http://localhost:5500/tts"
health_port = 9090
voice_enabled = true
voice_max_chars = 300
elevenlabs_voice_id = "pNInz6obpgDQGcFmaJgB"
elevenlabs_model = "eleven_turbo_v2_5"
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

        let jira = config.jira.unwrap();
        assert_eq!(jira.instance, "myteam.atlassian.net");
        assert_eq!(jira.default_project, "PROJ");
        assert_eq!(
            jira.webhook_secret.as_deref(),
            Some("super-secret-webhook-token-32chars!")
        );

        let nexus = config.nexus.unwrap();
        assert_eq!(nexus.agents.len(), 1);
        assert_eq!(nexus.agents[0].name, "builder");
        assert_eq!(nexus.agents[0].port, 9000);

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
        assert!(config.nexus.is_none());
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
}
