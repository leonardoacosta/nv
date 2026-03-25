# Implementation Tasks

<!-- beads:epic:nv-yep0 -->

## Req-1: Config Schema

- [ ] [1.1] [P-1] Add `PersonaConfig` struct to `crates/nv-core/src/config.rs` — fields: `tone: Option<String>`, `verbosity: Option<String>`, `formality: Option<String>`, `language_hints: Vec<String>` (serde default empty vec) [owner:api-engineer]
- [ ] [1.2] [P-1] Add `personas: HashMap<String, PersonaConfig>` field to `Config` in `crates/nv-core/src/config.rs` with `#[serde(default)]` so absent key deserializes as empty map [owner:api-engineer]

## Req-2: Persona Module

- [ ] [2.1] [P-1] Create `crates/nv-daemon/src/persona.rs` with `render_persona_block(personas: &HashMap<String, PersonaConfig>, channel: &str) -> Option<String>` — case-insensitive lookup, returns formatted markdown block or `None` [owner:api-engineer]
- [ ] [2.2] [P-1] Register module in `crates/nv-daemon/src/lib.rs` with `pub mod persona;` [owner:api-engineer]
- [ ] [2.3] [P-2] Add unit tests in `persona.rs`: (a) returns `None` for unknown channel, (b) returns `Some` block containing tone/verbosity/formality for known channel, (c) case-insensitive match works (`"Telegram"` matches `"telegram"` key), (d) `language_hints` appear in output when provided [owner:api-engineer]

## Req-3: System Context Assembly

- [ ] [3.1] [P-1] Update `build_system_context` signature in `crates/nv-daemon/src/agent.rs` to `build_system_context(channel: Option<&str>) -> String` [owner:api-engineer]
- [ ] [3.2] [P-1] Inside `build_system_context`, after appending `soul.md`, load `Config` from `~/.nv/nv.toml`; call `persona::render_persona_block(&config.personas, ch)` when `channel` is `Some(ch)`; append the returned block to `context` [owner:api-engineer]
- [ ] [3.3] [P-2] Log a `tracing::warn!` if config load fails inside `build_system_context`; fall through silently (no persona injected) [owner:api-engineer]
- [ ] [3.4] [P-2] Update all existing `build_system_context()` call sites in tests to `build_system_context(None)` — behavior is identical [owner:api-engineer]

## Req-4: Worker Call-Site Wiring

- [ ] [4.1] [P-1] In `crates/nv-daemon/src/worker.rs` inside `Worker::run`, extract the channel from the first `Trigger::Message` in `task.triggers` — `let channel = task.triggers.iter().find_map(...)` [owner:api-engineer]
- [ ] [4.2] [P-1] Pass extracted channel (as `Option<&str>`) to `build_system_context(channel)` [owner:api-engineer]

## Req-5: Example Config

- [ ] [5.1] [P-3] Add commented-out example `[personas.telegram]`, `[personas.teams]`, and `[personas.discord]` blocks to `config/nv.toml` with inline comments explaining each field [owner:api-engineer]

## Verify

- [ ] [6.1] `cargo build` passes for all workspace members [owner:api-engineer]
- [ ] [6.2] `cargo clippy -- -D warnings` passes with no new warnings [owner:api-engineer]
- [ ] [6.3] `cargo test -p nv-daemon` passes — persona unit tests green, existing tests unaffected [owner:api-engineer]
- [ ] [6.4] `cargo test -p nv-core` passes — config deserialization test: toml with `[personas.telegram]` deserializes to `Config.personas["telegram"]` with correct field values [owner:api-engineer]
- [ ] [6.5] [user] Manual test: add `[personas.telegram]` with `tone = "casual"` and `verbosity = "brief"` to `~/.nv/nv.toml`; send a Telegram message; confirm the persona block appears in the system prompt (visible via debug log or `--verbose` run) [owner:api-engineer]
- [ ] [6.6] [user] Manual test: remove `[personas]` section entirely — daemon starts cleanly, behavior identical to today [owner:api-engineer]
