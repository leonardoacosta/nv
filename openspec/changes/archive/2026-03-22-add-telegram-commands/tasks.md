# Implementation Tasks

<!-- beads:epic:TBD -->

## Command Parsing

- [x] [1.1] [P-1] Add BotCommand struct (command, args) and parse_bot_command(text) function to orchestrator.rs — extracts /command and arguments from message text [owner:api-engineer]
- [x] [1.2] [P-1] Update classify_trigger() — detect messages starting with "/" as TriggerClass::BotCommand, store parsed command in trigger metadata [owner:api-engineer]

## Output Formatting

- [x] [2.1] [P-1] Add format_for_telegram(output, format_type) utility to orchestrator.rs — converts raw tool output to mobile-friendly format [owner:api-engineer]
- [x] [2.2] [P-2] Implement status dot conversion — replace textual health statuses with emoji indicators [owner:api-engineer]
- [x] [2.3] [P-2] Implement condensed table format — convert markdown tables to key-value blocks or pre-aligned text [owner:api-engineer]
- [x] [2.4] [P-2] Strip ANSI codes and ASCII table borders from all tool output before Telegram send [owner:api-engineer]

## Command Handlers

- [x] [3.1] [P-1] Add handle_command(command, args, telegram, deps) dispatcher in orchestrator.rs — routes to per-command handlers [owner:api-engineer]
- [x] [3.2] [P-2] Implement /status handler — call project_health for all projects, format with status dots, send as reply [owner:api-engineer]
- [x] [3.3] [P-2] Implement /digest handler — inject CronEvent::Digest into trigger channel, confirm "Digest triggered" [owner:api-engineer]
- [x] [3.4] [P-2] Implement /health handler — call homelab_status, format with status dots, send as reply [owner:api-engineer]
- [x] [3.5] [P-2] Implement /apply handler — parse project + spec args, dispatch to start_session (Nexus), return confirmation keyboard [owner:api-engineer]
- [x] [3.6] [P-2] Implement /projects handler — list project registry with latest status dot per project, format as inline keyboard [owner:api-engineer]
- [x] [3.7] [P-2] Implement unknown command handler — list available commands with descriptions [owner:api-engineer]

## Integration

- [x] [4.1] [P-2] Wire command dispatch in orchestrator main loop — route TriggerClass::BotCommand to handle_command instead of worker pool [owner:api-engineer]
- [x] [4.2] [P-2] Extract shared tool execution functions from worker.rs into tools.rs — project_health, homelab_status callable from orchestrator [owner:api-engineer]

## Documentation

- [ ] [5.1] [deferred] Register commands in BotFather — /status, /digest, /health, /apply, /projects with descriptions [owner:user]

## Verify

- [x] [6.1] cargo build passes [owner:api-engineer]
- [x] [6.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [x] [6.3] cargo test — existing tests pass, new tests for parse_bot_command, format_for_telegram, handle_command routing [owner:api-engineer]
