# Nova (nv)

A proactive, always-on Rust daemon that monitors communication channels, manages tasks, and orchestrates tools across 18+ service integrations — all controllable via Telegram.

Nova runs Claude as a persistent subprocess, giving it access to 92 tools spanning DevOps, project management, infrastructure, finance, and smart home control. It receives messages, processes them through Claude, confirms write operations via inline keyboards, and delivers results back to the user.

## Install

```bash
# Build from source (requires Rust toolchain)
git clone https://github.com/leonardoacosta/nv.git
cd nv
cargo build --release

# Deploy (builds, installs binaries, sets up systemd)
bash deploy/install.sh
```

**Binaries installed:**
- `nv-daemon` — the always-on service
- `nv` — CLI client for status, queries, and diagnostics

## Usage

```bash
nv status                        # Daemon health + active sessions
nv check                         # Verify all service credentials
nv check --read-only             # Skip write probes
nv check --service stripe        # Check single service
nv stats                         # Message counts + Claude usage
nv digest --now                  # Trigger immediate digest
nv ask "What PRs are open on OO?" --json
```

The primary interface is **Telegram** — send messages, voice notes, or photos to your Nova bot and it responds with tool-assisted answers, inline action confirmations, and proactive digests.

## Architecture

```
crates/
  nv-core/     Core types: Channel trait, Trigger, Message, PendingAction
  nv-daemon/   Daemon: orchestrator, worker pool, Claude subprocess, all tools
  nv-cli/      CLI: status, check, ask, stats, digest
config/
  nv.toml      Runtime config (channels, services, agent settings)
  soul.md      Personality and behavior guidelines
  system-prompt.md  Operational dispatch rules
  identity.md  Name, nature, emoji
  user.md      Operator profile and preferences
deploy/
  install.sh   Build + install + systemd setup
  nv.service   Systemd unit (Type=notify, MemoryMax=2G)
```

**Data flow:** Telegram message -> Orchestrator -> Worker Pool (max 3) -> Claude subprocess -> Tool calls -> Confirmation keyboard -> Execute -> Reply

## Tools (92)

| Category | Tools | Auth |
|----------|-------|------|
| **Jira** | search, get, create, transition, assign, comment | API token (multi-instance) |
| **GitHub** | PRs, issues, runs, releases, diffs, compare | `gh` CLI |
| **Azure DevOps** | projects, pipelines, builds | PAT |
| **Vercel** | deployments, logs | API token |
| **Sentry** | issues, issue detail | Auth token |
| **PostHog** | trends, feature flags | API key |
| **Neon** | SQL query, projects, branches, compute | Connection string + API key |
| **Stripe** | customers, invoices | Secret key |
| **Resend** | emails, bounces | API key |
| **Doppler** | secrets (names only), compare, activity | API token |
| **Cloudflare** | zones, DNS records, domain status | API token |
| **Upstash** | Redis info, keys | REST URL + token |
| **Home Assistant** | states, entity, service calls | Long-lived token |
| **Docker** | container status, logs | Local socket |
| **Tailscale** | network status, node info | Local CLI |
| **Plaid** | balances, bills | DB connection |
| **Teams** | channels, messages, send, presence | MS Graph OAuth2 |
| **Calendar** | today, upcoming, next event | Google service account |
| **Web** | fetch URL, check URL, search | None / SearXNG |
| **Memory** | read, write, search + message history (FTS5) | Local SQLite |
| **Reminders** | set, list, cancel | Local SQLite |
| **Schedules** | list, add, remove (cron expressions) | Local SQLite |
| **Nexus** | sessions, commands, project proposals | gRPC |
| **Aggregation** | project health, homelab status, financial summary | Composite |
| **Bash** | git status/log/branch/diff, ls, cat, beads | Scoped allowlist |
| **Cross-channel** | list channels, send to any channel | Internal |
| **Diagnostics** | check all services (read + write probes) | All |

All write operations require **Telegram confirmation** via inline keyboard before execution.

## Channels

| Channel | Direction | Notes |
|---------|-----------|-------|
| **Telegram** | In + Out | Primary. Voice notes, photos, inline keyboards. |
| **Discord** | In + Out | Bot token, configurable channel IDs |
| **Teams** | In + Out | Webhook relay (inbound) + MS Graph (outbound) |
| **Email** | Out | Via Resend API |
| **iMessage** | In + Out | macOS native |

## Configuration

All config lives in `~/.nv/`:

| File | Purpose |
|------|---------|
| `nv.toml` | Channels, services, agent model, digest interval |
| `env` | Service credentials (auto-loaded by daemon + CLI) |
| `memory/` | Persistent markdown memory files |
| `messages.db` | SQLite: messages, tool usage, reminders, schedules |

### Multi-Instance Services

Any service supports named instances for personal vs organization credentials:

```toml
[jira.instances.personal]
instance = "you.atlassian.net"
default_project = "OO"

[jira.instances.llc]
instance = "company.atlassian.net"
default_project = "CT"

[jira.project_map]
OO = "personal"
CT = "llc"
```

Env vars follow the pattern `SERVICE_VAR_INSTANCENAME` with fallback to unqualified `SERVICE_VAR`.

### Service Health

```bash
$ nv check --read-only

 Services (read)
  ✓ ado           projects endpoint reachable           126ms
  ✓ cloudflare    token verified                        177ms
  ✓ docker        docker daemon reachable (v29.1.3)      31ms
  ✓ github        leonardoacosta authenticated           203ms
  ✓ ha            API reachable (http://localhost:8123)    6ms
  ✓ posthog       projects endpoint reachable            122ms
  ✓ resend        domains endpoint reachable             217ms
  ✓ sentry        org: leonardo-acosta                   455ms
  ✓ stripe        balance endpoint reachable             263ms
  ✓ upstash       INFO command succeeded                 185ms
  ...

 Summary: 14/16 healthy, 2 unhealthy
```

## Voice & Media

- **Voice input:** Send a Telegram voice note -> ElevenLabs STT transcription -> processed as text
- **Voice output:** `/voice` toggle, responses synthesized via ElevenLabs TTS
- **Photos:** Send a photo -> downloaded -> passed to Claude vision via `--attachment`
- **Audio files:** MP3/WAV -> ElevenLabs STT transcription

## Deployment

```bash
bash deploy/install.sh
```

This script:
1. Stops running services
2. Builds release binaries (`cargo build --release`)
3. Installs to `~/.local/bin/`
4. Links config from repo to `~/.nv/`
5. Sets up Discord relay + Teams webhook relay
6. Installs and enables systemd user services
7. Verifies health endpoint

**Systemd services:**
- `nv.service` — main daemon (MemoryMax=2G, WatchdogSec=60)
- `nv-discord-relay.service` — Discord bot relay
- `nv-teams-relay.service` — Teams webhook relay

**Logs:** `journalctl --user -u nv -f`

## Gotchas

- Claude CLI subprocess uses OAuth — the daemon needs `~/.claude/.credentials.json` accessible
- `nv check` auto-loads `~/.nv/env` so it works outside systemd context
- Jira project KEYs must be 2-10 uppercase alphanumeric (`OO`, `CT` — not full project names)
- Neon tools need per-project `POSTGRES_URL_{CODE}` env vars (direct connection, not API)
- Doppler tools never return secret **values** — names only, by design
- Worker timeout is 5 minutes (configurable via `daemon.worker_timeout_secs` in nv.toml)
- Persistent subprocess stays alive across tool cycles within a worker session; cold-start fallback if it dies
