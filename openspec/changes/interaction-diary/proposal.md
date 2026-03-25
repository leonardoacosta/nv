# Proposal: Interaction Diary (v7 Extension)

## Change ID
`interaction-diary`

## Summary

Extend the existing `diary.rs` implementation (shipped in v7 Wave 1) with five missing
capabilities: `channel_source` field, `response_latency_ms` field, daily rollover at midnight,
`/diary` Telegram bot command, and a dashboard Diary timeline page. Zero new token cost —
all diary writes remain pure Rust post-processing.

## Context
- Extends: `crates/nv-daemon/src/diary.rs` (DiaryEntry, DiaryWriter — already implemented)
- Extends: `crates/nv-daemon/src/worker.rs` (two DiaryEntry construction sites)
- Extends: `crates/nv-daemon/src/orchestrator.rs` (handle_bot_commands, command dispatch table)
- Extends: `crates/nv-daemon/src/http.rs` (add GET /api/diary endpoint)
- Extends: `dashboard/src/` (new DiaryPage, sidebar nav entry, App.tsx route)
- Related: `crates/nv-daemon/src/worker.rs` already imports DiaryEntry and DiaryWriter; slug field already propagated from orchestrator

## Motivation

The base diary (slug, tools called, token cost, result summary) shipped in Wave 1. Three gaps
remain before the diary is operationally useful:

1. **No channel source**: entries show `trigger_source = "telegram"` but not which Telegram
   chat or other channel variant actually sent the message. Debugging multi-channel setups
   requires this.

2. **No response latency**: the worker already computes `response_time_ms` and logs it to
   `message_store`. Not carrying it into the diary means per-interaction performance is invisible
   without grepping structured logs.

3. **No query interface**: diary files sit at `~/.nv/diary/` but there is no way to ask Nova
   "what did you do today?" from Telegram or inspect entries from the dashboard. The `/diary`
   command and dashboard page close this gap at zero token cost.

## Requirements

### Req-1: Add `channel_source` Field to DiaryEntry

Add `channel_source: String` to `DiaryEntry`. This is the raw channel name from the trigger
(e.g., `"telegram-personal"`, `"teams-work"`, `"cli"`). It is distinct from `trigger_source`
(which classifies the trigger kind: `"message"`, `"cron"`, `"nexus"`).

The format_entry output gains a **Channel:** line:

```markdown
## HH:MM — {trigger_type} ({trigger_source}) · {slug}

**Channel:** {channel_source}
**Triggers:** {count} ({trigger_type})
**Tools called:** {tool_names or "none"}
**Sources checked:** {sources_checked}
**Result:** {result_summary}
**Latency:** {response_latency_ms}ms
**Cost:** {tokens_in} in + {tokens_out} out tokens
```

### Req-2: Add `response_latency_ms` Field to DiaryEntry

Add `response_latency_ms: u64` to `DiaryEntry`. Populated from the `response_time_ms`
variable that already exists at both worker DiaryEntry construction sites in `worker.rs`.

For the dashboard-path entry (cold-start bypass), latency is computed as
`task_start.elapsed().as_millis() as u64` — same value already passed to `log_outbound`.

### Req-3: Daily Rollover (New File at Midnight)

`DiaryWriter::write_entry` already uses `entry.timestamp.date_naive()` to compute the daily
file path, so a midnight trigger naturally produces a new file. No additional logic is needed
in `diary.rs`.

The rollover requirement is already satisfied by the current implementation. This requirement
documents that fact and adds a targeted test: create an entry with timestamp `23:59`, then
create one with timestamp `00:00` the next day, and assert two separate files exist.

### Req-4: `/diary` Telegram Bot Command

Add a `/diary` handler to `orchestrator.rs` that reads recent diary entries and returns a
formatted summary to the requesting channel.

Signature: `/diary [N]` where N is the number of recent entries to show (default 5, max 20).

Behavior:
- Read the current day's diary file from `~/.nv/diary/YYYY-MM-DD.md`
- If fewer than N entries exist in today's file, also read yesterday's file to fill up to N
- Parse entries by splitting on `## ` headings
- Return the most recent N entries formatted as plain text (no markdown tables — Telegram
  renders them poorly)
- If no diary files exist, return `"No diary entries found."`

Example response:
```
Diary — last 5 entries

14:32 — message (telegram-personal) · check-jira-sprint
  Tools: jira_search, read_memory
  Result: sent reply (42ms, 340 in + 89 out)

13:05 — cron (digest) · morning-digest
  Tools: jira_search, nexus_status
  Result: sent digest (1,203ms, 1,820 in + 410 out)
```

The handler reads files directly (no Claude API call). Access to `DiaryWriter.base_path` is
needed — expose it via a `base_path()` accessor method.

### Req-5: HTTP Endpoint GET /api/diary

Add `GET /api/diary` to `http.rs`. Query parameters:

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `date` | `String` (YYYY-MM-DD) | today | Which day's file to read |
| `limit` | `usize` | 50 | Max entries to return |

Response shape:

```json
{
  "date": "2026-03-25",
  "entries": [
    {
      "time": "14:32",
      "trigger_type": "message",
      "trigger_source": "telegram",
      "channel_source": "telegram-personal",
      "slug": "check-jira-sprint",
      "tools_called": ["jira_search", "read_memory"],
      "result_summary": "sent reply",
      "response_latency_ms": 42,
      "tokens_in": 340,
      "tokens_out": 89
    }
  ],
  "total": 1
}
```

Parsing strategy: read the daily markdown file and parse each `## HH:MM` section back into
structured fields. This is straightforward because `format_entry` emits a fixed schema.

The endpoint needs `DiaryWriter` accessible from `HttpState` — add
`diary: Arc<Mutex<DiaryWriter>>` to `HttpState` (it is already on `SharedDeps`; thread it
through in `main.rs`).

### Req-6: Dashboard Diary Timeline Page

Add a `/diary` route to the dashboard showing a reverse-chronological timeline of diary
entries for the selected day.

Components:
- `DiaryPage.tsx` — page with a date picker, entry count badge, and scrollable timeline
- `DiaryEntry.tsx` — single entry card: heading line, tools pills, result text, latency chip,
  token cost badge

UI layout (matches existing cosmic theme):
- Header: "Interaction Diary" + date selector (prev/next day arrows + date display)
- Summary bar: total entries today, total tokens today, avg latency today
- Timeline: reverse-chronological list of `DiaryEntry` cards

Add to `Sidebar.tsx` NAV_ITEMS: `{ to: "/diary", label: "Diary", icon: BookOpen }` (Lucide).
Add to `App.tsx` route: `<Route path="/diary" element={<DiaryPage />} />`.
Add `DiaryGetResponse` and `DiaryEntryItem` to `dashboard/src/types/api.ts`.

## Scope

- **IN**: `channel_source` + `response_latency_ms` fields on DiaryEntry, rollover test,
  `/diary` Telegram command (read-only, no Claude), GET /api/diary endpoint, dashboard Diary
  page + sidebar nav
- **OUT**: diary search/grep, diary-to-memory summarization, diary retention policy,
  diary write from orchestrator bot commands (those don't go through the worker path),
  editing diary entries from dashboard

## Impact

| Area | Change |
|------|--------|
| `crates/nv-daemon/src/diary.rs` | Add `channel_source`, `response_latency_ms` to DiaryEntry; add `base_path()` accessor; update `format_entry`; add rollover test |
| `crates/nv-daemon/src/worker.rs` | Populate new fields at both DiaryEntry construction sites |
| `crates/nv-daemon/src/orchestrator.rs` | Add `"diary"` branch to command dispatch; add `cmd_diary()` handler |
| `crates/nv-daemon/src/http.rs` | Add `GET /api/diary` handler + `DiaryWriter` on `HttpState` |
| `crates/nv-daemon/src/main.rs` | Thread `diary` into `HttpState` |
| `dashboard/src/pages/DiaryPage.tsx` | New page |
| `dashboard/src/components/DiaryEntry.tsx` | New component |
| `dashboard/src/types/api.ts` | Add `DiaryGetResponse`, `DiaryEntryItem` |
| `dashboard/src/components/Sidebar.tsx` | Add Diary nav item |
| `dashboard/src/App.tsx` | Add `/diary` route |

## Risks

| Risk | Mitigation |
|------|-----------|
| Markdown parsing is fragile | Parser is trivial — split on `## ` and extract fixed-position lines; the schema is under our control |
| DiaryEntry construction sites miss new fields | Both sites are in `worker.rs` — compiler enforces struct completeness |
| HttpState doesn't have DiaryWriter | Add field in main.rs where HttpState is constructed; single change point |
| Telegram /diary response is too long for one message | Clamp at 20 entries max; format_entry is ~120 chars per entry; 20 entries ≈ 2,400 chars (well under 4,096 Telegram limit) |
