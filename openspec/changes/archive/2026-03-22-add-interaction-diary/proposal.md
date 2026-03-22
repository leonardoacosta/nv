# Proposal: Add Interaction Diary

## Change ID
`add-interaction-diary`

## Summary

Rust-written daily interaction log at `~/.nv/diary/YYYY-MM-DD.md`. Every trigger processed
by the agent loop gets a timestamped entry recording what was checked, what was found, and
what action was taken. Zero token cost — written by Rust post-processing, not by Claude.

## Context
- Extends: `crates/nv-daemon/src/agent.rs` (post-processing after each trigger batch)
- Related: Existing memory system at `~/.nv/memory/`, tracing logs at `~/.nv/logs/`

## Motivation

Nova processes triggers (Telegram messages, cron digests, Nexus events) but has no persistent
record of what it did beyond tracing logs. Tracing logs are verbose, unstructured, and rotate
away. A diary provides:

1. **Accountability** — what did Nova do while Leo was away?
2. **Debugging** — why did Nova send that digest? What tools did it call?
3. **Context for Claude** — diary entries can be loaded as memory context for continuity
4. **Audit trail** — which Jira issues were created/transitioned, by whose confirmation?

## Requirements

### Req-1: Daily Rolling Diary Files

One markdown file per day at `~/.nv/diary/YYYY-MM-DD.md`. Created on first entry of the day.
Old files accumulate (no auto-deletion — they're small and useful for grep).

### Req-2: Entry Format

Each trigger batch produces one diary entry:

```markdown
## HH:MM — {trigger_type} ({source})

**Triggers:** {count} ({types})
**Tools called:** {tool_names or "none"}
**Sources checked:** {jira: N issues, nexus: online/offline, memory: topics}
**Result:** {sent reply / suppressed digest / created pending action / error}
**Cost:** {input_tokens + output_tokens from Claude response}
```

### Req-3: Written by Rust, Not Claude

The diary is written by Rust code in the agent loop's post-processing step. It reads the
tool call log and response metadata that already exist in the agent loop. Zero additional
Claude API calls. Zero additional token cost.

### Req-4: Diary Initialization

Create `~/.nv/diary/` directory on daemon startup (alongside memory/ and state/ init).

## Scope
- **IN**: Diary module, daily rolling files, post-processing hook in agent loop, directory init
- **OUT**: Claude-written entries, diary search tool, diary-to-memory summarization, retention policy

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/diary.rs` | New module: DiaryWriter with write_entry() |
| `crates/nv-daemon/src/agent.rs` | Post-processing: collect tool calls + response metadata, call diary |
| `crates/nv-daemon/src/main.rs` | Init diary directory on startup |

## Risks
| Risk | Mitigation |
|------|-----------|
| Diary writes slow down agent loop | Async write via spawn_blocking or sync (files are tiny, <1ms) |
| Disk usage over months | ~1KB/day typical, ~365KB/year. Not a concern. |
