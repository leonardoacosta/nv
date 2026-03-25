# Proposal: Agent Persona Switching

## Change ID
`agent-persona-switching`

## Summary

Per-channel persona profiles for Nova. Adds channel-specific personality overrides via
`[personas.{channel}]` in `nv.toml`. The system prompt is composed by merging `soul.md`
with the active channel's persona override. Work channels (Teams) get professional/concise
tone; personal channels (Telegram) get casual/friendly; Discord gets brief/technical. When no
override is configured, Nova's default persona from `soul.md` applies unchanged.

## Context
- Phase: 5 — Polish | Wave: 11
- Feature area: Persona management
- Idea source: nv-n2l
- Files: `crates/nv-daemon/src/agent.rs`, `crates/nv-core/src/config.rs`,
  new `crates/nv-daemon/src/persona.rs`
- Depends on: none

## Motivation

Nova currently has a single global personality configured in `~/.nv/soul.md`. As Nova is
used across multiple channels with different social contexts — work Teams threads, personal
Telegram, technical Discord — a single tone is a poor fit for all of them.

The mismatch is concrete:
- In Teams, Leo's colleagues see Nova responses. A casual "lmk" or emoji-heavy reply looks
  unprofessional in a work thread.
- In Telegram, an overly formal and verbose response to "what's the weather?" creates
  unnecessary friction in casual conversation.
- In Discord, technical peers expect terse, code-first answers — not executive summaries.

The fix is to make tone, verbosity, and formality configurable per channel while keeping
the core identity (`soul.md`) unchanged. The channel name already flows through every
`InboundMessage.channel` field, giving us a free routing key.

## Design

### Configuration Schema

New top-level section in `nv.toml`:

```toml
[personas.telegram]
tone = "casual"
verbosity = "normal"
formality = "casual"
language_hints = ["use contractions", "light emoji ok"]

[personas.teams]
tone = "professional"
verbosity = "brief"
formality = "professional"
language_hints = ["no emoji", "use full sentences"]

[personas.discord]
tone = "technical"
verbosity = "brief"
formality = "casual"
language_hints = ["code-first answers", "skip pleasantries"]
```

Field definitions:

| Field | Type | Values | Default |
|-------|------|--------|---------|
| `tone` | `String` | `"casual"` \| `"professional"` \| `"technical"` | inherited from soul.md |
| `verbosity` | `String` | `"brief"` \| `"normal"` \| `"verbose"` | `"normal"` |
| `formality` | `String` | `"casual"` \| `"professional"` | `"casual"` |
| `language_hints` | `Vec<String>` | arbitrary instruction strings | `[]` |

The channel key matches the channel name used in `InboundMessage.channel` (e.g., `"telegram"`,
`"teams"`, `"discord"`, `"imessage"`, `"email"`). The match is case-insensitive.

### Config Struct (`nv-core/src/config.rs`)

```rust
#[derive(Debug, Clone, Deserialize, Default)]
pub struct PersonaConfig {
    /// Tone of voice: "casual", "professional", "technical".
    pub tone: Option<String>,
    /// Response length: "brief", "normal", "verbose".
    pub verbosity: Option<String>,
    /// Formality level: "casual", "professional".
    pub formality: Option<String>,
    /// Free-form instruction strings appended to the persona block.
    #[serde(default)]
    pub language_hints: Vec<String>,
}
```

Added to `Config`:

```rust
/// Per-channel persona overrides. Key is channel name (case-insensitive).
#[serde(default)]
pub personas: HashMap<String, PersonaConfig>,
```

### Persona Module (`nv-daemon/src/persona.rs`)

New module with a single public function:

```rust
/// Render a persona override block for injection into the system context.
///
/// Returns `None` if no override is configured for the given channel,
/// meaning the caller should use the base soul.md unchanged.
pub fn render_persona_block(
    personas: &HashMap<String, PersonaConfig>,
    channel: &str,
) -> Option<String>
```

The function looks up `channel` (case-insensitive) in `personas`. If found, it builds a
structured markdown block:

```
## Active Persona Override (channel: telegram)

**Tone:** casual
**Verbosity:** normal
**Formality:** casual
**Language hints:**
- use contractions
- light emoji ok

These settings override your default tone for this conversation. Stay true to your core
identity (soul.md) — only adapt delivery style.
```

If the lookup misses, returns `None`.

### System Context Assembly (`nv-daemon/src/agent.rs`)

`build_system_context` currently concatenates system prompt + identity + soul + user files
unconditionally. Persona injection is added as a new step.

The channel must be passed in. `build_system_context` is extended to accept an optional
channel parameter:

```rust
pub fn build_system_context(channel: Option<&str>) -> String
```

After appending `soul.md`, the persona block is injected if a channel is provided and a
matching override exists:

```rust
if let Some(ch) = channel {
    if let Some(block) = persona::render_persona_block(&config.personas, ch) {
        context.push_str("\n\n");
        context.push_str(&block);
    }
}
```

Config is loaded inside `build_system_context` from the standard config path (`~/.nv/nv.toml`
via `Config::load()`). If config load fails or the `personas` map is empty, the call is a
no-op (no persona injected).

### Call Sites

`build_system_context` is called in `worker.rs` inside `Worker::run`. The `WorkerTask`
already carries `triggers: Vec<Trigger>`. The channel is extracted from the first
`Trigger::Message` in the batch:

```rust
let channel = task.triggers.iter().find_map(|t| {
    if let Trigger::Message(msg) = t {
        Some(msg.channel.as_str())
    } else {
        None
    }
});
let system_context = build_system_context(channel);
```

Cron and CLI triggers carry no channel — `channel` is `None` and the default persona
(soul.md only) is used.

### Default Behavior

When `[personas]` is absent from `nv.toml`, `Config.personas` deserializes as an empty
`HashMap` (via `#[serde(default)]`). `render_persona_block` returns `None`. The behavior
is identical to today — zero regression risk.

### Channel Name Stability

Channel names are string literals set at registration time in each channel's `mod.rs`:
`"telegram"`, `"teams"`, `"discord"`, `"imessage"`, `"email"`. These are stable. The
persona key match is case-insensitive (`to_lowercase` on both sides) to be lenient.

## Scope

- **IN**: `PersonaConfig` struct, `personas` map in `Config`, `persona.rs` module,
  `build_system_context` channel parameter, call-site wiring in `worker.rs`,
  example config block in `config/nv.toml`
- **OUT**: Runtime persona switching via chat command (separate spec), per-user persona
  overrides (channel-level is sufficient for now), persona A/B testing, persisting which
  persona was active per conversation turn

## Impact

| File | Change |
|------|--------|
| `crates/nv-core/src/config.rs` | Add `PersonaConfig` struct; add `personas: HashMap<String, PersonaConfig>` field to `Config` |
| `crates/nv-daemon/src/persona.rs` | New module: `render_persona_block` |
| `crates/nv-daemon/src/lib.rs` | `pub mod persona;` |
| `crates/nv-daemon/src/agent.rs` | `build_system_context(channel: Option<&str>)` — add channel param, persona injection |
| `crates/nv-daemon/src/worker.rs` | Extract channel from first `Trigger::Message`; pass to `build_system_context` |
| `config/nv.toml` | Add example `[personas.telegram]`, `[personas.teams]`, `[personas.discord]` blocks (commented out) |

## Risks

| Risk | Mitigation |
|------|-----------|
| Persona block inflates context token count | Block is ~10-15 lines; negligible vs. full context (~2-3K tokens). No mitigation needed. |
| Channel name mismatch (e.g., future channel uses different slug) | Case-insensitive match + documented convention in `channel/mod.rs` comments |
| Config load failure inside `build_system_context` silently drops persona | Log warning on load failure; fall back to no-persona gracefully |
| `build_system_context` signature change breaks tests | All existing test call sites pass `None` — same behavior as before |
