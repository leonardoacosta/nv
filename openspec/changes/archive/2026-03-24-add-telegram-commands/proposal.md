# Proposal: Add Telegram Commands

## Change ID
`add-telegram-commands`

## Summary

Register BotFather commands (`/status`, `/digest`, `/health`, `/apply`, `/projects`) and handle
them in the orchestrator. Transform output for mobile readability: inline keyboards for choices,
status dots for health indicators, condensed table format. Remove raw CLI output from responses.

## Context
- Extends: `crates/nv-daemon/src/orchestrator.rs` (command routing), `crates/nv-daemon/src/telegram/client.rs` (output formatting), `crates/nv-daemon/src/telegram/types.rs` (command entity parsing)
- Related: PRD section 7.3, existing `classify_trigger()` in orchestrator, `TriggerClass::Command`, aggregation-layer tools (`project_health`, `homelab_status`)
- Depends on: `mature-nexus-integration` (spec 20) for `/apply`, `add-aggregation-layer` (spec 18) for `/status` and `/health`

## Motivation

Nova responds to natural language but has no command surface for quick, structured actions. Telegram
Bot Commands provide a discoverable menu in the bot UI. Formatting output for mobile (status dots,
inline keyboards, condensed format) replaces the raw pipe-delimited tables that are unreadable on
small screens.

## Requirements

### Req-1: BotFather Command Registration

Register these commands with BotFather (manual step, documented in spec):

| Command | Description |
|---------|-------------|
| `/status` | Project health dashboard |
| `/digest` | Trigger immediate digest |
| `/health` | Homelab status |
| `/apply` | Apply a spec to a project |
| `/projects` | List all projects |

### Req-2: Command Parsing

Detect `/command` messages in the orchestrator's trigger classification:

- Messages starting with `/` are classified as `TriggerClass::Command`
- Parse command and arguments: `/apply oo fix-chat-bugs` → command="apply", args=["oo", "fix-chat-bugs"]
- Unknown commands get a help response listing available commands

### Req-3: Command Handlers

| Command | Handler | Output |
|---------|---------|--------|
| `/status` | Call `project_health` for all projects | Status dots per project per dimension |
| `/digest` | Trigger immediate `CronEvent::Digest` | Standard digest output |
| `/health` | Call `homelab_status` | Container + network + home health with dots |
| `/apply <project> <spec>` | Dispatch to `start_session` (Nexus) | Confirmation keyboard → session status |
| `/projects` | List project registry | Project codes with latest status dot |

### Req-4: Mobile-Friendly Output Formatting

Transform tool output for Telegram mobile:

- **Status dots**: `🟢` healthy, `🟡` degraded, `🔴` down — replace textual status
- **Inline keyboards**: When output has actionable choices (e.g., list of projects), present as keyboard buttons
- **Condensed tables**: Convert markdown tables to key-value blocks or `<pre>` aligned text
- **No raw CLI output**: Strip ANSI codes, pipe separators, ASCII borders from any tool output before sending

Add `format_for_telegram(output, format_hints)` utility that applies these transformations.

### Req-5: Command-Only Routing (No Claude)

Commands are handled directly by the orchestrator — they do NOT go through a Claude worker.
This means:

- `/status` calls `project_health` tool directly (same code the worker uses)
- `/digest` injects a `CronEvent::Digest` into the trigger channel
- Response time is ~100ms (tool execution) instead of ~5s (Claude round-trip)
- Only `/apply` involves Claude (via Nexus StartSession)

## Scope
- **IN**: 5 BotFather commands, command parsing in orchestrator, direct tool execution for /status /digest /health /projects, mobile-friendly output formatting, help response for unknown commands
- **OUT**: Custom command registration API, per-user command permissions, command aliases, command history

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/orchestrator.rs` | Command parsing, direct handler dispatch, help response |
| `crates/nv-daemon/src/telegram/client.rs` | Add `format_for_telegram()` output transformer |
| `crates/nv-daemon/src/telegram/types.rs` | Parse bot command entities from Telegram updates |
| `crates/nv-daemon/src/worker.rs` | No change — commands bypass workers |

## Risks
| Risk | Mitigation |
|------|-----------|
| BotFather registration is manual | Document exact steps in tasks, one-time setup |
| Direct tool execution duplicates worker code | Extract shared tool execution into tools.rs functions callable from both worker and orchestrator |
| /apply without Nexus connection | Return clear error: "Cannot reach Nexus agent for OO. Is it running?" |
| Status dots on non-Unicode terminals | Telegram always supports Unicode — not an issue |
