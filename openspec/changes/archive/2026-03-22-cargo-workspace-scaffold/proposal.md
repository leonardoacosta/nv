# cargo-workspace-scaffold

## Summary

Scaffold the Cargo workspace with 3 crates (nv-core, nv-daemon, nv-cli). No logic — just compilable structure with shared dependencies.

## Motivation

Foundation for all subsequent specs. Establishes crate boundaries, dependency versions, and build configuration. Every other spec depends on this structure existing and compiling.

## Design

### Workspace Structure

```
nv/
├── Cargo.toml              # Workspace root
├── crates/
│   ├── nv-core/            # Lib: shared types, config, Channel trait
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   ├── nv-daemon/          # Bin: main daemon process
│   │   ├── Cargo.toml
│   │   └── src/main.rs
│   └── nv-cli/             # Bin: CLI tool
│       ├── Cargo.toml
│       └── src/main.rs
├── config/
│   └── nv.example.toml
└── deploy/
    └── nv.service
```

### Workspace Cargo.toml

Root `Cargo.toml` declares the workspace members and shared dependency versions. All crates reference dependencies via `workspace = true` to keep versions centralized.

```toml
[workspace]
resolver = "2"
members = ["crates/nv-core", "crates/nv-daemon", "crates/nv-cli"]

[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
thiserror = "2"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
chrono = { version = "0.4", features = ["serde"] }
reqwest = { version = "0.12", features = ["json"] }
clap = { version = "4", features = ["derive"] }
toml = "0.8"
uuid = { version = "1", features = ["v4", "serde"] }
async-trait = "0.1"
```

### nv-core (lib crate)

Minimal lib crate that re-exports nothing yet. Module stubs are declared but empty — spec-2 fills them in.

```rust
// src/lib.rs
pub mod config;
pub mod types;
pub mod channel;
```

Each module file (`config.rs`, `types.rs`, `channel.rs`) is created empty or with a single placeholder comment. This establishes the module structure without implementing any logic.

**Dependencies:** serde, serde_json, anyhow, thiserror, chrono, uuid, async-trait, toml, tracing (all via workspace).

### nv-daemon (bin crate)

Entry point with tokio runtime, tracing initialization, and graceful shutdown skeleton.

```rust
// src/main.rs
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    tracing::info!("NV daemon starting");

    // TODO: Load config from ~/.nv/nv.toml
    // TODO: Start channel listeners
    // TODO: Start agent loop

    tokio::signal::ctrl_c().await?;
    tracing::info!("NV daemon shutting down");

    Ok(())
}
```

**Dependencies:** nv-core (path), tokio, tracing, tracing-subscriber, anyhow (all via workspace).

### nv-cli (bin crate)

Clap-derive CLI with subcommand stubs. Each subcommand prints a placeholder message.

```rust
// src/main.rs
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "nv", about = "NV — Master Agent Harness CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show daemon and session status
    Status,
    /// Ask NV a question
    Ask {
        /// The question to ask
        query: String,
    },
    /// Manage NV configuration
    Config,
    /// Trigger or view digest
    Digest {
        /// Trigger immediate digest
        #[arg(long)]
        now: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Status => println!("not implemented yet"),
        Commands::Ask { query } => println!("not implemented yet: {query}"),
        Commands::Config => println!("not implemented yet"),
        Commands::Digest { now } => {
            if now {
                println!("not implemented yet: immediate digest");
            } else {
                println!("not implemented yet: show last digest");
            }
        }
    }
}
```

**Dependencies:** nv-core (path), clap (via workspace).

### config/nv.example.toml

Full example configuration covering all sections referenced in the PRD. Secrets are documented as env var references, never hardcoded.

```toml
# NV Configuration
# Copy to ~/.nv/nv.toml and fill in values.
# Secrets should be set via environment variables (Doppler).

[agent]
model = "claude-sonnet-4-6"
think = true
digest_interval_minutes = 60
# ANTHROPIC_API_KEY set via env var

[telegram]
chat_id = 123456789
# TELEGRAM_BOT_TOKEN set via env var

[jira]
instance = "leonardoacosta.atlassian.net"
default_project = "OO"
# JIRA_API_TOKEN set via env var
# JIRA_USERNAME set via env var

[nexus]
[[nexus.agents]]
name = "homelab"
host = "homelab"
port = 7400

[[nexus.agents]]
name = "macbook"
host = "macbook"
port = 7400

[daemon]
tts_url = "http://100.91.88.16:9999"
health_port = 8400
```

### deploy/nv.service

systemd unit file following the claude-daemon pattern. Restart on failure with 5s delay, watchdog at 60s.

```ini
[Unit]
Description=NV Master Agent Harness
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=nyaptor
ExecStart=/usr/local/bin/nv-daemon
Restart=on-failure
RestartSec=5s
WatchdogSec=60
EnvironmentFile=-/etc/nv/env
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
```

## Verification

- `cargo build` succeeds for all 3 workspace members
- `cargo clippy` passes with no warnings
- `nv-cli --help` prints subcommand help text
- `nv-daemon` starts, prints "NV daemon starting", and shuts down on Ctrl+C
