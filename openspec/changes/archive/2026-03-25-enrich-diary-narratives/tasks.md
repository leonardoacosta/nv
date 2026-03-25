# Implementation Tasks

<!-- beads:epic:TBD -->

## System Prompt

- [x] [1.1] [P-1] Add summary tag instruction to Nova's system prompt — append one line instructing Claude to end every response with `[SUMMARY: <past-tense action, ≤120 chars>]` [owner:api-engineer]

## Rust Implementation

- [x] [2.1] [P-1] In worker.rs — add `extract_summary(response_text: &str) -> (String, String)` helper that returns `(summary, cleaned_response)`: search for last `[SUMMARY:` … `]`, extract inner text (cap at 120 chars), strip the tag line from response; if not found, fall back to first sentence (split on `.`/`!`/`?`, trim, cap at 120 chars); if response is empty return `("empty response", "")` [owner:api-engineer]
- [x] [2.2] [P-1] In worker.rs — replace the existing 80-char truncation block (lines ~852–861) with a call to `extract_summary(&response_text)`; use returned `summary` as `result_summary` and `cleaned_response` as the text delivered to the channel [owner:api-engineer]
- [x] [2.3] [P-2] In diary.rs — update `result_summary` field doc comment from "truncated snippet" language to "narrative summary extracted from Claude's [SUMMARY:] tag or first sentence" [owner:api-engineer]

## Verify

- [x] [3.1] cargo build passes [owner:api-engineer]
- [x] [3.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [x] [3.3] cargo test — unit tests for extract_summary: tag present, tag absent (first sentence fallback), empty response, tag with >120 chars, tag mid-response vs end-of-response [owner:api-engineer]
