# Implementation Tasks

<!-- beads:epic:TBD -->

## Reply Threading

- [ ] [1.1] [P-1] Update worker.rs — thread telegram_message_id through to all send_message calls in response path, pass as reply_to [owner:api-engineer]
- [ ] [1.2] [P-2] Verify digest/cron responses send without reply_to (no regression) [owner:api-engineer]

## Typing Indicator

- [ ] [2.1] [P-1] Add send_chat_action(chat_id, action) method to TelegramClient — POST to sendChatAction endpoint, fire-and-forget [owner:api-engineer]
- [ ] [2.2] [P-1] Call send_chat_action("typing") as first step in worker process_task(), before prompt building [owner:api-engineer]

## Long-Task Confirmation

- [ ] [3.1] [P-2] Add is_long_task(triggers) heuristic to orchestrator — returns (bool, estimated_secs, description) based on trigger classification [owner:api-engineer]
- [ ] [3.2] [P-2] Send confirmation message via send_message with reply_to when is_long_task returns true, before dispatching to worker pool [owner:api-engineer]

## Quiet Hours

- [ ] [4.1] [P-1] Add quiet_start and quiet_end (Option<String>) to DaemonConfig in nv-core/src/config.rs, parse as NaiveTime [owner:api-engineer]
- [ ] [4.2] [P-2] Add is_quiet_hours(config) helper in orchestrator.rs — checks current local time against configured window [owner:api-engineer]
- [ ] [4.3] [P-2] Gate non-High-priority task dispatch in orchestrator — hold in queue during quiet window, release when window ends [owner:api-engineer]
- [ ] [4.4] [P-2] Add quiet hours check to outbound message path — suppress non-P0 digest/nexus-event messages during quiet window [owner:api-engineer]

## Verify

- [ ] [5.1] cargo build passes [owner:api-engineer]
- [ ] [5.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [ ] [5.3] cargo test — existing tests pass, new tests for is_quiet_hours, is_long_task heuristic [owner:api-engineer]
