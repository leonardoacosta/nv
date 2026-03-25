# Implementation Tasks

<!-- beads:epic:TBD -->

## API Batch

- [ ] [2.1] [P-1] In `poll_messages()` in `crates/nv-daemon/src/channels/telegram/mod.rs`, replace `answer_callback_query(&cb.id, None)` with `answer_callback_query(&cb.id, Some(label))` where `label` is derived from `cb.data` prefix: `approve:` → `"Working on it..."`, `edit:` → `"Editing..."`, `cancel:` → `"Cancelled."`, anything else → `"Got it."` [owner:api-engineer]

## Verify

- [ ] [3.1] `cargo build -p nv-daemon` passes [owner:api-engineer]
- [ ] [3.2] Unit test: callback label helper returns correct text for each known prefix and the fallback [owner:api-engineer]
