# Proposal: Enrich Diary Narratives

## Change ID
`enrich-diary-narratives`

## Summary

Replace the truncated 80-char snippet in `DiaryEntry.result_summary` with a one-line narrative
summary extracted from the first sentence of Claude's response. No extra API calls. Zero token
cost beyond what the worker already spends.

## Context
- Modifies: `crates/nv-daemon/src/worker.rs` (summary extraction)
- Modifies: `crates/nv-daemon/src/diary.rs` (field rename/doc update, no struct shape change)
- Related: `add-interaction-diary` (archived) — established the diary system

## Motivation

The current `result_summary` field is built by taking the first 80 chars of `response_text` and
appending `...`. This produces entries like:

```
**Result:** Sure, I've looked at the Jira board. OO-142 is a priority mismatch in the sprint...
```

That's a truncated fragment, not a summary. The diary is most useful when entries read as
human-legible records: "Resolved OO-142 priority mismatch and sent Jira close request." A one-line
narrative summary makes the diary scannable and useful as memory context.

## Approach

Use a `[SUMMARY: ...]` tag in the Claude system prompt. The worker already has access to the raw
response text before building the DiaryEntry. It can extract the tag with a simple regex/string
parse. If the tag is absent (e.g. very short responses, chat acks), fall back to the first sentence
of the response text (up to 120 chars), stripped of the tag syntax.

**Why a tag over "first sentence"?**
- First sentence of a Claude response is often a preamble ("I've checked...") not a summary.
- A tag lets Claude write the summary in the right register: past tense, action-oriented, specific.
- Parsing is trivial and 100% local — no extra round-trip.

**Tag format:**

```
[SUMMARY: Resolved OO-142 priority mismatch and sent Jira close request.]
```

Claude appends this at the end of every response. The worker strips it from the displayed response
text and uses it as `result_summary`.

## Requirements

### Req-1: System Prompt Instruction

Add a one-line instruction to the system prompt that tells Claude to append a `[SUMMARY: ...]`
tag as the final line of every response. Tag must be ≤120 chars, past tense, action-oriented.

### Req-2: Worker Summary Extraction

In `worker.rs`, after the tool use loop completes and `response_text` is assembled, extract the
`[SUMMARY: ...]` tag before writing the diary entry:

1. Search `response_text` for `[SUMMARY: ...]` (last occurrence wins if multiple).
2. If found: use the inner text as `result_summary`; strip the tag line from `response_text`
   so it is not delivered to the channel.
3. If not found: fall back to the first sentence of `response_text` (up to 120 chars). This
   preserves behavior for chat acks and very short responses where Claude may omit the tag.

### Req-3: Fallback Sentence Extraction

The fallback must extract the first sentence (split on `.`, `!`, or `?`), trim whitespace, and
cap at 120 chars. If the response is empty, use `"empty response"` (existing behavior).

### Req-4: No Struct Changes Required

`DiaryEntry.result_summary: String` stays as-is. Only the value assigned to it changes.
Update the doc comment to reflect that it now holds a narrative summary, not a truncated snippet.

## Scope
- **IN**: System prompt instruction, tag extraction in worker, fallback sentence extraction, doc
  comment update on `result_summary`
- **OUT**: Diary file format changes, DiaryEntry struct changes, new API calls, response length
  increases beyond one line

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/worker.rs` | Extract `[SUMMARY: ...]` tag from response; strip from delivered text; fallback to first sentence |
| `crates/nv-daemon/src/diary.rs` | Update `result_summary` doc comment |
| System prompt (soul or config) | Add one instruction line for the summary tag |

## Risks
| Risk | Mitigation |
|------|-----------|
| Claude omits the tag on short responses | Fallback to first sentence (Req-3) handles this |
| Tag appears mid-response and breaks formatting | Always strip before delivery; search for last occurrence |
| Summary exceeds 120 chars | Truncate at 120 with no ellipsis in the tag path |
| Regex complexity | Plain string search: find `[SUMMARY:`, extract to next `]` — no regex needed |
