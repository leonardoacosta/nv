# Module Restructure

## MODIFIED Requirements

### Requirement: Daemon Source Organization

The daemon MUST organize tool source files under a `tools/` module directory and channel source files under a `channels/` module directory, replacing the current flat layout where 16+ `*_tools.rs` files share the `src/` root with core daemon files.

#### Scenario: Tool files relocated to tools/ module

**Given** the daemon has 12 `*_tools.rs` files in `crates/nv-daemon/src/`
**When** the restructure is applied
**Then** all tool files live under `crates/nv-daemon/src/tools/`:
  - `tools/mod.rs` — `register_tools()`, `execute_tool()`, `Checkable` trait, `ServiceRegistry<T>`
  - `tools/check.rs` — `CheckResult`, `check_all()`
  - `tools/stripe.rs` (was `stripe_tools.rs`)
  - `tools/vercel.rs` (was `vercel_tools.rs`)
  - `tools/sentry.rs` (was `sentry_tools.rs`)
  - `tools/neon.rs` (was `neon_tools.rs`)
  - `tools/posthog.rs` (was `posthog_tools.rs`)
  - `tools/upstash.rs` (was `upstash_tools.rs`)
  - `tools/resend.rs` (was `resend_tools.rs`)
  - `tools/ado.rs` (was `ado_tools.rs`)
  - `tools/ha.rs` (was `ha_tools.rs`)
  - `tools/docker.rs` (was `docker_tools.rs`)
  - `tools/plaid.rs` (was `plaid_tools.rs`)
  - `tools/github.rs` (was `github.rs`)
  - `tools/web.rs` (was `web_tools.rs`)
  - `tools/cloudflare.rs` (was `cloudflare_tools.rs`)
  - `tools/doppler.rs` (was `doppler_tools.rs`)
  - `tools/calendar.rs` (was `calendar_tools.rs`)
  - `tools/schedule.rs` (was `schedule_tools.rs`)
  - `tools/jira/` (already a module, moves into `tools/`)
**And** `main.rs` module declarations are updated from flat `mod xxx_tools;` to `mod tools;`
**And** all internal `use crate::xxx_tools` paths are updated to `use crate::tools::xxx`

#### Scenario: Channel files consolidated under channels/ module

**Given** channels are already in subdirectories (`telegram/`, `discord/`, `teams/`, `email/`, `imessage/`)
**When** the restructure is applied
**Then** a `channels/mod.rs` re-exports all channel types
**And** `main.rs` declares `mod channels;` instead of individual `mod telegram; mod discord;` etc.
**And** existing channel module internals are unchanged

#### Scenario: Core daemon files remain in src root

**Given** the restructure moves tools and channels
**When** complete
**Then** `src/` root retains only core daemon files: `main.rs`, `agent.rs`, `orchestrator.rs`, `worker.rs`, `callbacks.rs`, `health.rs`, `http.rs`, `memory.rs`, `messages.rs`, `conversation.rs`, `diary.rs`, `state.rs`, `bash.rs`, `claude.rs`, `tts.rs`, `voice_input.rs`, `speech_to_text.rs`, `account.rs`, `aggregation.rs`, `reminders.rs`, `scheduler.rs`, `shutdown.rs`, `tailscale.rs`
