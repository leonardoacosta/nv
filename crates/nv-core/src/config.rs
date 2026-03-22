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

// ── Config structs ──────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub agent: AgentConfig,
    pub telegram: Option<TelegramConfig>,
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
pub struct JiraConfig {
    pub instance: String,
    pub default_project: String,
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
    pub jira_api_token: Option<String>,
    pub jira_username: Option<String>,
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
            jira_api_token: std::env::var("JIRA_API_TOKEN").ok(),
            jira_username: std::env::var("JIRA_USERNAME").ok(),
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

[jira]
instance = "myteam.atlassian.net"
default_project = "PROJ"

[nexus]
[[nexus.agents]]
name = "builder"
host = "127.0.0.1"
port = 9000

[daemon]
tts_url = "http://localhost:5500/tts"
health_port = 9090
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.agent.model, "claude-sonnet-4-20250514");
        assert!(!config.agent.think);
        assert_eq!(config.agent.digest_interval_minutes, 30);

        let tg = config.telegram.unwrap();
        assert_eq!(tg.chat_id, 123_456_789);

        let jira = config.jira.unwrap();
        assert_eq!(jira.instance, "myteam.atlassian.net");
        assert_eq!(jira.default_project, "PROJ");

        let nexus = config.nexus.unwrap();
        assert_eq!(nexus.agents.len(), 1);
        assert_eq!(nexus.agents[0].name, "builder");
        assert_eq!(nexus.agents[0].port, 9000);

        let daemon = config.daemon.unwrap();
        assert_eq!(daemon.tts_url.as_deref(), Some("http://localhost:5500/tts"));
        assert_eq!(daemon.health_port, 9090);
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
