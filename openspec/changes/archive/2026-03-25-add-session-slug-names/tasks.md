# Implementation Tasks

<!-- beads:epic:nv-wqd -->

## Config

- [x] [1.1] [P-1] Add `dashboard_url: Option<String>` field to `DaemonConfig` in `crates/nv-core/src/config.rs` — no default, omitted = link suppressed [owner:api-engineer]

## Slug Generation

- [x] [2.1] [P-1] Add `generate_slug(content: &str) -> String` to `crates/nv-daemon/src/worker.rs` — lowercase + strip punctuation + remove stopwords + take first 2–3 words joined by hyphens, max 40 chars, fallback `"session"` [owner:api-engineer]
- [x] [2.2] [P-1] Add `generate_slug_for_triggers(triggers: &[Trigger]) -> String` to `worker.rs` — dispatches to `generate_slug` on message content, uses cron event name for `Trigger::Cron`, prefixes `"cli-"` for `Trigger::CliCommand` [owner:api-engineer]

## WorkerTask

- [x] [3.1] [P-1] Add `pub slug: String` field to `WorkerTask` struct in `worker.rs` [owner:api-engineer]

## Orchestrator

- [x] [4.1] [P-1] Call `generate_slug_for_triggers(&triggers)` in `Orchestrator::process_trigger_batch` in `orchestrator.rs` and assign result to `task.slug` when constructing `WorkerTask` [owner:api-engineer]

## SharedDeps

- [x] [5.1] [P-1] Add `pub dashboard_url: Option<String>` to `SharedDeps` struct in `worker.rs` [owner:api-engineer]
- [x] [5.2] [P-1] Populate `SharedDeps.dashboard_url` from `config.daemon.as_ref().and_then(|d| d.dashboard_url.clone())` in `main.rs` when building `SharedDeps` [owner:api-engineer]

## Diary

- [x] [6.1] [P-1] Add `pub slug: String` field to `DiaryEntry` struct in `diary.rs` [owner:api-engineer]
- [x] [6.2] [P-1] Update `format_entry` in `diary.rs` to append ` · {slug}` to the heading line: `## {time} — {trigger_type} ({trigger_source}) · {slug}` [owner:api-engineer]
- [x] [6.3] [P-2] Update all `DiaryEntry` construction sites (in `worker.rs` and in diary tests) to supply `slug: task.slug.clone()` or a test value [owner:api-engineer]

## Telegram Response Link

- [x] [7.1] [P-1] In `Worker::run` in `worker.rs`, after `response_text` is finalised and before the channel `send_message` call, conditionally build and append the dashboard link — only when `deps.dashboard_url` is `Some` and `response_text` is non-empty [owner:api-engineer]
- [x] [7.2] [P-1] Format the link as HTML `<a href="{url}/sessions/{task_id}">{slug}</a>` on its own line, separated from the response body by one blank line — matches the HTML parse mode used by `TelegramClient::send_message` (see `client.rs` line 343: `parse_mode: "HTML"`) [owner:api-engineer]
- [x] [7.3] [P-2] Ensure CLI responses (`task.cli_response_txs`) receive the original undecorated `response_text`, not the link-appended version [owner:api-engineer]

## Tests

- [x] [8.1] [P-1] Unit tests for `generate_slug`: cover normal message, stopword-only input (fallback), cron slug, CLI slug, long input truncation at 40 chars, empty input [owner:api-engineer]
- [x] [8.2] [P-2] Unit test for diary `format_entry` with slug field — assert heading contains ` · ` separator and slug value [owner:api-engineer]

## Verify

- [x] [9.1] `cargo build -p nv-daemon -p nv-core` passes [owner:api-engineer]
- [x] [9.2] `cargo clippy -p nv-daemon -p nv-core -- -D warnings` passes [owner:api-engineer]
- [x] [9.3] `cargo test -p nv-daemon -p nv-core` passes — existing tests plus new slug + diary tests [owner:api-engineer]
