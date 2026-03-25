# Proposal: Add Session Slug Names

## Change ID
`add-session-slug-names`

## Summary

Give each worker session a human-readable slug name derived from the first message content (e.g.
"jira-sprint-review", "telegram-photo-analysis"). Store the slug on `WorkerTask`, propagate it into
`DiaryEntry`, and append a dashboard link to Telegram responses. The dashboard base URL is
configurable in `nv.toml`.

## Context
- Extends: `crates/nv-daemon/src/worker.rs` (WorkerTask, Worker::run), `crates/nv-daemon/src/diary.rs` (DiaryEntry, format_entry), `crates/nv-core/src/config.rs` (DaemonConfig)
- Related: beads nv-wqd (session-slug-names-with-dashboard-links), Wave 2a independent feature
- Depends on: none — works on the current daemon, no dashboard service required

## Motivation

Sessions are currently identified only by UUID (`task_id`). When reading diary logs or receiving
Telegram responses, there is no way to tell at a glance what a session was about. A slug derived
from the message content ("jira-sprint-review") is instantly readable. Pairing it with a dashboard
link in the Telegram reply gives a one-tap path to inspect the full session detail.

The dashboard link is dead until the dashboard exists — that is intentional. The URL is
configurable so it can be pointed at a real deployment in Wave 2b without re-deploying the daemon.
If `dashboard_url` is not set, the link is omitted entirely.

## Requirements

### Req-1: Slug Generation

Add a pure function `generate_slug(content: &str) -> String` in `worker.rs`:

- Normalise input: lowercase, strip punctuation, collapse whitespace.
- Extract the first 2–3 meaningful words (skip stopwords: "the", "a", "an", "is", "are", "was",
  "were", "can", "could", "would", "please", "hey", "hi", "hello", "i", "me", "my", "what",
  "how", "when", "where", "why", "who").
- Join with hyphens.
- Clamp to 40 characters max (truncate at last complete word within limit).
- Fallback to `"session"` when no meaningful words remain (e.g. pure emoji or very short ack).

Examples:

| Input | Slug |
|-------|------|
| `"Can you check the Jira sprint review?"` | `"check-jira-sprint"` |
| `"Telegram photo analysis needed"` | `"telegram-photo-analysis"` |
| `"what is the status of OO?"` | `"status-oo"` |
| `"hey nova"` | `"session"` |
| `"[cron] digest"` | `"digest"` |

For cron triggers (`Trigger::Cron`), use the event name as the slug directly (e.g. `"digest"`,
`"morning-briefing"`).

For CLI triggers (`Trigger::CliCommand`), prefix with `"cli-"` and apply the same word extraction.

### Req-2: WorkerTask Slug Field

Add `pub slug: String` to `WorkerTask` in `worker.rs`.

The slug is set in `Orchestrator::process_trigger_batch` immediately before building the task,
using `generate_slug` applied to the first trigger's content.

```rust
// In process_trigger_batch, before constructing WorkerTask:
let slug = generate_slug_for_triggers(&triggers);

let task = WorkerTask {
    id: Uuid::new_v4(),
    slug,
    triggers: std::mem::take(triggers),
    // ... rest unchanged
};
```

Add a helper `generate_slug_for_triggers(triggers: &[Trigger]) -> String` that dispatches to
`generate_slug` or the cron/CLI variants based on the first trigger type.

### Req-3: Diary Entry Slug Field

Add `pub slug: String` to `DiaryEntry` in `diary.rs`.

Update `format_entry` to include the slug in the diary heading:

```
## 14:32 — message (telegram) · jira-sprint-review
```

Format: existing `## {time} — {trigger_type} ({trigger_source})` gets ` · {slug}` appended.

The diary `write_entry` path and all test fixtures must supply the new field.

### Req-4: Dashboard Link in Telegram Responses

In `Worker::run`, after `response_text` is finalised and before the channel send, optionally append
a dashboard link:

```
[jira-sprint-review](https://dashboard.example.com/sessions/550e8400-e29b-41d4-a716-446655440000)
```

Rules:
- Only appended when `deps.dashboard_url` is `Some(url)` AND `response_text` is non-empty.
- The UUID is `task_id` (the `WorkerTask.id`).
- One blank line separates the response body from the link line.
- The link is Telegram Markdown-compatible (`[text](url)` inline link).
- Not appended for CLI responses (those go to `task.cli_response_txs`, which already receive the
  undecorated `response_text`).

### Req-5: Dashboard URL Config

Add `dashboard_url: Option<String>` to `DaemonConfig` in `crates/nv-core/src/config.rs`.

Propagate into `SharedDeps` in `worker.rs`:
```rust
pub dashboard_url: Option<String>,
```

Populate from config in `main.rs` when building `SharedDeps`.

Example `nv.toml` usage:
```toml
[daemon]
dashboard_url = "https://dashboard.nv.local"
```

When absent, the link is silently omitted. No default value.

## Scope
- **IN**: slug generation, `WorkerTask.slug`, `DiaryEntry.slug`, diary heading format, Telegram
  response link, `DaemonConfig.dashboard_url`, `SharedDeps.dashboard_url`
- **OUT**: dashboard service itself, slug persistence to DB, slug search, slug uniqueness
  enforcement, link preview rendering

## Impact

| Area | Change |
|------|--------|
| `crates/nv-core/src/config.rs` | Add `dashboard_url: Option<String>` to `DaemonConfig` |
| `crates/nv-daemon/src/worker.rs` | Add `generate_slug`, `WorkerTask.slug`, `SharedDeps.dashboard_url`, link append in `Worker::run` |
| `crates/nv-daemon/src/orchestrator.rs` | Call `generate_slug_for_triggers`, set `task.slug` |
| `crates/nv-daemon/src/diary.rs` | Add `DiaryEntry.slug`, update `format_entry` heading |
| `crates/nv-daemon/src/main.rs` | Read `dashboard_url` from config, pass to `SharedDeps` |

## Risks

| Risk | Mitigation |
|------|-----------|
| Slug collides across concurrent sessions | Slugs are display-only — no uniqueness requirement |
| Stopword list is incomplete for non-English input | Fallback to `"session"` makes this safe; list can be extended later |
| Dashboard URL not yet deployed | Link is omitted when `dashboard_url` is absent; dead links are acceptable per spec |
| Telegram Markdown parse mode required for inline links | Telegram `sendMessage` already uses `parse_mode = "Markdown"` (or HTML) — confirm in `client.rs` before choosing format |
