# core-types-and-config

## Summary

Define the core type system in nv-core: Config struct (TOML deserialization), message types (InboundMessage, OutboundMessage), Trigger enum, Channel trait, AgentResponse, PendingAction. Config loads from `~/.nv/nv.toml` with env var overrides for secrets.

## Motivation

Every subsequent spec depends on these types. The Channel trait defines the adapter contract for Telegram (spec-3), Discord, Teams, etc. The Trigger enum is the unified event model for the agent loop (spec-4). Config loading is needed by every crate at runtime.

## Design

### Config (config.rs)

Full config struct matching the TOML schema from the PRD. Secrets come from environment variables, not the TOML file.

```rust
use serde::Deserialize;

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
```

### Config Loading

Config loads from `~/.nv/nv.toml`. Secrets are resolved from environment variables at runtime, never stored in the TOML file.

```rust
impl Config {
    pub fn load() -> anyhow::Result<Self> {
        Self::load_from(Self::default_path()?)
    }

    pub fn load_from(path: PathBuf) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config from {}", path.display()))?;
        let config: Config = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse config from {}", path.display()))?;
        Ok(config)
    }

    pub fn default_path() -> anyhow::Result<PathBuf> {
        let home = std::env::var("HOME")
            .context("HOME environment variable not set")?;
        Ok(PathBuf::from(home).join(".nv").join("nv.toml"))
    }
}
```

### Secrets Resolution

A `Secrets` struct holds values sourced exclusively from environment variables. This keeps secrets out of the config file and makes Doppler integration straightforward.

```rust
#[derive(Debug, Clone)]
pub struct Secrets {
    pub anthropic_api_key: String,
    pub telegram_bot_token: Option<String>,
    pub jira_api_token: Option<String>,
    pub jira_username: Option<String>,
}

impl Secrets {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            anthropic_api_key: std::env::var("ANTHROPIC_API_KEY")
                .context("ANTHROPIC_API_KEY must be set")?,
            telegram_bot_token: std::env::var("TELEGRAM_BOT_TOKEN").ok(),
            jira_api_token: std::env::var("JIRA_API_TOKEN").ok(),
            jira_username: std::env::var("JIRA_USERNAME").ok(),
        })
    }
}
```

### InboundMessage (types.rs)

Unified inbound message from any channel. The `metadata` field carries channel-specific data (e.g., Telegram `message_id`, callback query data).

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundMessage {
    pub id: String,
    pub channel: String,
    pub sender: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub thread_id: Option<String>,
    pub metadata: serde_json::Value,
}
```

### OutboundMessage (types.rs)

Message to send through a channel. Keyboard field supports inline keyboards (Telegram) or reactions (Discord).

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundMessage {
    pub channel: String,
    pub content: String,
    pub reply_to: Option<String>,
    pub keyboard: Option<InlineKeyboard>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineKeyboard {
    pub rows: Vec<Vec<InlineButton>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineButton {
    pub text: String,
    pub callback_data: String,
}
```

### Trigger Enum (types.rs)

The unified event type that all channel listeners and schedulers push into the `mpsc` channel. The agent loop receives these and processes sequentially.

```rust
#[derive(Debug)]
pub enum Trigger {
    /// Inbound message from any channel
    Message(InboundMessage),
    /// Scheduled cron event (digest, cleanup)
    Cron(CronEvent),
    /// Event from a Nexus agent session
    NexusEvent(SessionEvent),
    /// Command from the CLI
    CliCommand(CliRequest),
}

#[derive(Debug, Clone)]
pub enum CronEvent {
    Digest,
    MemoryCleanup,
}

#[derive(Debug, Clone)]
pub struct SessionEvent {
    pub agent_name: String,
    pub session_id: String,
    pub event_type: SessionEventType,
    pub details: Option<String>,
}

#[derive(Debug, Clone)]
pub enum SessionEventType {
    Started,
    Completed,
    Failed,
    Progress,
}

#[derive(Debug, Clone)]
pub struct CliRequest {
    pub command: CliCommand,
    pub response_tx: Option<tokio::sync::oneshot::Sender<String>>,
}

#[derive(Debug, Clone)]
pub enum CliCommand {
    Status,
    Ask(String),
    DigestNow,
}
```

### Channel Trait (channel.rs)

The adapter contract. Each channel (Telegram, Discord, Teams, etc.) implements this trait. The `poll_messages` method returns a batch of messages since last poll. Channels are spawned as tokio tasks.

```rust
use async_trait::async_trait;
use crate::types::{InboundMessage, OutboundMessage};

#[async_trait]
pub trait Channel: Send + Sync {
    /// Human-readable channel name (e.g., "telegram", "discord")
    fn name(&self) -> &str;

    /// Establish connection to the channel service
    async fn connect(&mut self) -> anyhow::Result<()>;

    /// Poll for new messages since last check. Returns empty vec if none.
    async fn poll_messages(&self) -> anyhow::Result<Vec<InboundMessage>>;

    /// Send a message through this channel
    async fn send_message(&self, msg: OutboundMessage) -> anyhow::Result<()>;

    /// Gracefully disconnect from the channel service
    async fn disconnect(&mut self) -> anyhow::Result<()>;
}
```

### AgentResponse (types.rs)

What the agent loop produces after processing a trigger through Claude. Determines routing: replies go to channels, actions go to pending confirmation, digests go to Telegram.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentResponse {
    /// Direct reply to a channel message
    Reply {
        channel: String,
        content: String,
        reply_to: Option<String>,
        keyboard: Option<InlineKeyboard>,
    },
    /// Action requiring confirmation (Jira create, transition, etc.)
    Action(PendingAction),
    /// Proactive digest to send to Telegram
    Digest {
        content: String,
        suggested_actions: Vec<PendingAction>,
    },
    /// Answer to a CLI query
    QueryAnswer(String),
    /// No response needed (message was informational)
    NoOp,
}
```

### PendingAction (types.rs)

An action drafted by Claude that requires Leo's confirmation via Telegram inline keyboard before execution.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingAction {
    pub id: uuid::Uuid,
    pub description: String,
    pub action_type: ActionType,
    pub payload: serde_json::Value,
    pub status: ActionStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionType {
    JiraCreate,
    JiraTransition,
    JiraAssign,
    JiraComment,
    ChannelReply,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ActionStatus {
    Pending,
    Approved,
    Rejected,
    Executed,
    Failed,
}
```

### Module Re-exports (lib.rs)

Clean re-exports so downstream crates can use `nv_core::Config` instead of `nv_core::config::Config`.

```rust
pub mod config;
pub mod types;
pub mod channel;

pub use config::{Config, Secrets};
pub use types::*;
pub use channel::Channel;
```

## Verification

- `cargo build` succeeds for all workspace members
- `cargo test` passes — unit tests for:
  - Config parsing from valid TOML string
  - Config parsing fails gracefully on missing required fields
  - InboundMessage serialization/deserialization round-trip
  - OutboundMessage with keyboard serialization
  - PendingAction with ActionStatus transitions
  - Secrets::from_env() with and without env vars set
