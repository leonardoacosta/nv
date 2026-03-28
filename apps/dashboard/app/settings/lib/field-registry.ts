/**
 * Field metadata registry for all known Nova daemon config keys.
 * Maps flat config key paths to display metadata used by FieldRow.
 */

export interface FieldMeta {
  description: string;
  placeholder?: string;
  patternHint?: string;
  default?: string | number | boolean;
  unit?: string;
  min?: number;
  max?: number;
  pattern?: string;
  restartRequired?: boolean;
  /** Which settings section this field belongs to */
  group?: "daemon" | "channels" | "integrations" | "memory";
}

/**
 * Static registry mapping config key paths to their display metadata.
 * Keys are dot-separated paths matching the flattened config object.
 */
export const FIELD_REGISTRY: Record<string, FieldMeta> = {
  // ── Daemon ──────────────────────────────────────────────────────────────
  "daemon.log_level": {
    description: "Verbosity of daemon log output. Use 'info' for normal operation, 'debug' for troubleshooting.",
    placeholder: "info",
    default: "info",
    patternHint: "One of: error, warn, info, debug, trace",
    pattern: "^(error|warn|info|debug|trace)$",
    group: "daemon",
  },
  "daemon.interval_ms": {
    description: "How often the daemon polls for new work, in milliseconds. Lower values reduce latency but increase CPU usage.",
    placeholder: "5000",
    default: 5000,
    unit: "ms",
    min: 100,
    max: 60000,
    restartRequired: true,
    group: "daemon",
  },
  "daemon.port": {
    description: "HTTP port the Nova daemon API server listens on.",
    placeholder: "8400",
    default: 8400,
    min: 1024,
    max: 65535,
    restartRequired: true,
    group: "daemon",
  },
  "daemon.host": {
    description: "Host address the daemon binds to. Use 0.0.0.0 to listen on all interfaces.",
    placeholder: "127.0.0.1",
    default: "127.0.0.1",
    restartRequired: true,
    group: "daemon",
  },
  "daemon.debug": {
    description: "Enable extra debug output and verbose error messages in the daemon.",
    default: false,
    group: "daemon",
  },
  "agent.model": {
    description: "The LLM model identifier used for Nova's main reasoning loop.",
    placeholder: "claude-opus-4-5",
    group: "daemon",
  },
  "agent.max_tokens": {
    description: "Maximum number of output tokens per agent response. Higher values allow longer replies but cost more.",
    placeholder: "8192",
    default: 8192,
    unit: "tokens",
    min: 256,
    max: 200000,
    group: "daemon",
  },
  "agent.system_prompt_path": {
    description: "Path to the system prompt file used to configure Nova's personality and capabilities.",
    placeholder: "/home/user/.config/nova/system.md",
    group: "daemon",
  },
  "proactive_watcher.enabled": {
    description: "When enabled, Nova periodically checks for pending obligations and messages that need attention.",
    default: true,
    restartRequired: true,
    group: "daemon",
  },
  "proactive_watcher.interval_minutes": {
    description: "How often the proactive watcher runs its check cycle, in minutes.",
    placeholder: "15",
    default: 15,
    unit: "min",
    min: 1,
    max: 1440,
    group: "daemon",
  },
  "proactive_watcher.quiet_start": {
    description: "Start of the quiet period (24h format). The proactive watcher will not run during quiet hours.",
    placeholder: "22:00",
    default: "22:00",
    patternHint: "Format: HH:MM (24-hour)",
    pattern: "^([01]?[0-9]|2[0-3]):[0-5][0-9]$",
    group: "daemon",
  },
  "proactive_watcher.quiet_end": {
    description: "End of the quiet period (24h format). Proactive checks resume after this time.",
    placeholder: "08:00",
    default: "08:00",
    patternHint: "Format: HH:MM (24-hour)",
    pattern: "^([01]?[0-9]|2[0-3]):[0-5][0-9]$",
    group: "daemon",
  },
  "server.port": {
    description: "Port for the Nova dashboard/API server.",
    placeholder: "3000",
    default: 3000,
    min: 1024,
    max: 65535,
    restartRequired: true,
    group: "daemon",
  },
  "log_level": {
    description: "Top-level log verbosity override.",
    placeholder: "info",
    default: "info",
    pattern: "^(error|warn|info|debug|trace)$",
    patternHint: "One of: error, warn, info, debug, trace",
    group: "daemon",
  },
  "debug": {
    description: "Global debug mode. Overrides section-level debug settings.",
    default: false,
    group: "daemon",
  },

  // ── Channels ─────────────────────────────────────────────────────────────
  "telegram.bot_token": {
    description: "Telegram Bot API token from @BotFather. Required to send and receive Telegram messages.",
    placeholder: "1234567890:ABC...",
    group: "channels",
    restartRequired: true,
  },
  "telegram.enabled": {
    description: "Enable or disable the Telegram channel integration.",
    default: true,
    restartRequired: true,
    group: "channels",
  },
  "telegram.allowed_user_ids": {
    description: "Comma-separated list of Telegram user IDs allowed to interact with Nova. Leave empty to allow all.",
    placeholder: "123456789,987654321",
    group: "channels",
  },
  "telegram.webhook_url": {
    description: "Public HTTPS URL for the Telegram webhook endpoint. Required for webhook mode (leave empty for polling).",
    placeholder: "https://example.com/api/telegram/webhook",
    group: "channels",
  },
  "discord.bot_token": {
    description: "Discord bot token from the Discord Developer Portal. Required for Discord integration.",
    placeholder: "OTk...",
    group: "channels",
    restartRequired: true,
  },
  "discord.enabled": {
    description: "Enable or disable the Discord channel integration.",
    default: true,
    restartRequired: true,
    group: "channels",
  },
  "discord.guild_id": {
    description: "Discord server (guild) ID to restrict Nova's activity to. Leave empty for all servers.",
    placeholder: "1234567890123456789",
    group: "channels",
  },
  "teams.app_id": {
    description: "Microsoft Teams app/bot ID from Azure Active Directory app registration.",
    placeholder: "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx",
    group: "channels",
  },
  "teams.app_password": {
    description: "Microsoft Teams bot password/client secret from Azure app registration.",
    group: "channels",
    restartRequired: true,
  },
  "teams.enabled": {
    description: "Enable or disable the Microsoft Teams channel integration.",
    default: true,
    restartRequired: true,
    group: "channels",
  },
  "websocket.enabled": {
    description: "Enable the WebSocket channel for real-time browser-based chat.",
    default: true,
    restartRequired: true,
    group: "channels",
  },
  "websocket.port": {
    description: "Port the WebSocket server listens on.",
    placeholder: "8401",
    default: 8401,
    min: 1024,
    max: 65535,
    restartRequired: true,
    group: "channels",
  },
  "notifications.enabled": {
    description: "Enable desktop or push notifications when Nova receives messages.",
    default: false,
    group: "channels",
  },

  // ── Integrations ──────────────────────────────────────────────────────────
  "anthropic.api_key": {
    description: "Anthropic API key for Claude model access. Required for all AI reasoning tasks.",
    placeholder: "sk-ant-...",
    group: "integrations",
    restartRequired: true,
  },
  "openai.api_key": {
    description: "OpenAI API key. Used as a fallback model provider or for specific tools.",
    placeholder: "sk-...",
    group: "integrations",
  },
  "elevenlabs.api_key": {
    description: "ElevenLabs API key for text-to-speech voice synthesis.",
    placeholder: "...",
    group: "integrations",
  },
  "elevenlabs.voice_id": {
    description: "ElevenLabs voice ID to use for TTS output. Find voice IDs in your ElevenLabs dashboard.",
    placeholder: "21m00Tcm4TlvDq8ikWAM",
    group: "integrations",
  },
  "github.token": {
    description: "GitHub personal access token for repository access, issue tracking, and PR management.",
    placeholder: "ghp_...",
    group: "integrations",
  },
  "stripe.secret_key": {
    description: "Stripe secret API key for payment processing and subscription management.",
    placeholder: "sk_live_...",
    group: "integrations",
  },
  "resend.api_key": {
    description: "Resend API key for transactional email delivery.",
    placeholder: "re_...",
    group: "integrations",
  },
  "sentry.dsn": {
    description: "Sentry DSN for error tracking and performance monitoring.",
    placeholder: "https://...@sentry.io/...",
    group: "integrations",
  },
  "sentry.auth_token": {
    description: "Sentry auth token for API access and source map uploads.",
    placeholder: "sntrys_...",
    group: "integrations",
  },
  "posthog.api_key": {
    description: "PostHog project API key for product analytics and feature flags.",
    placeholder: "phc_...",
    group: "integrations",
  },
  "posthog.host": {
    description: "PostHog instance URL. Use app.posthog.com for cloud or your self-hosted URL.",
    placeholder: "https://app.posthog.com",
    default: "https://app.posthog.com",
    group: "integrations",
  },

  // ── Memory ────────────────────────────────────────────────────────────────
  "memory.enabled": {
    description: "Enable Nova's persistent memory system for storing context across sessions.",
    default: true,
    restartRequired: true,
    group: "memory",
  },
  "memory.max_topics": {
    description: "Maximum number of memory topics Nova will maintain. Older topics are archived when the limit is reached.",
    placeholder: "100",
    default: 100,
    unit: "topics",
    min: 10,
    max: 10000,
    group: "memory",
  },
  "memory.ttl_days": {
    description: "How many days memory entries are retained before expiry. Set to 0 to keep indefinitely.",
    placeholder: "90",
    default: 90,
    unit: "days",
    min: 0,
    max: 3650,
    group: "memory",
  },
  "personas.default": {
    description: "The default persona Nova uses when no channel-specific persona is configured.",
    placeholder: "nova",
    default: "nova",
    group: "memory",
  },
  "context.max_tokens": {
    description: "Maximum tokens to include in Nova's context window from memory and recent messages.",
    placeholder: "32000",
    default: 32000,
    unit: "tokens",
    min: 1000,
    max: 200000,
    group: "memory",
  },
  "storage.path": {
    description: "Directory path for Nova's local file storage. Used for memory persistence and config files.",
    placeholder: "/home/user/.local/share/nova",
    restartRequired: true,
    group: "memory",
  },
  "db.url": {
    description: "PostgreSQL connection URL for the Nova database. Supports standard Postgres connection strings.",
    placeholder: "postgres://user:pass@localhost:5432/nova",
    restartRequired: true,
    group: "memory",
  },
  "database.url": {
    description: "PostgreSQL connection URL (alias for db.url).",
    placeholder: "postgres://user:pass@localhost:5432/nova",
    restartRequired: true,
    group: "memory",
  },
  "cache.ttl_seconds": {
    description: "How long query results are cached in memory before being invalidated.",
    placeholder: "60",
    default: 60,
    unit: "s",
    min: 0,
    max: 86400,
    group: "memory",
  },

  // ── Materialize ───────────────────────────────────────────────────────────
  "materialize.enabled": {
    description: "Enable automatic materialization of contacts and projects from memory topics on a schedule.",
    default: true,
    group: "daemon",
  },
  "materialize.cron_hour": {
    description: "Hour of the day (0-23) when the automatic materialization job runs.",
    placeholder: "4",
    default: 4,
    unit: "h",
    min: 0,
    max: 23,
    group: "daemon",
  },
};

/** Look up metadata for a config key, falling back to an empty object. */
export function getFieldMeta(key: string): FieldMeta {
  return FIELD_REGISTRY[key] ?? {};
}
