//! Per-channel persona override rendering.
//!
//! Provides [`render_persona_block`], which looks up a channel name in the
//! personas map (case-insensitive) and returns a formatted markdown block for
//! injection into the system context. Returns `None` when no override is
//! configured, leaving `soul.md` behaviour unchanged.

use std::collections::HashMap;

use nv_core::config::PersonaConfig;

/// Render a persona override block for injection into the system context.
///
/// Returns `None` if no override is configured for the given channel,
/// meaning the caller should use the base soul.md unchanged.
pub fn render_persona_block(
    personas: &HashMap<String, PersonaConfig>,
    channel: &str,
) -> Option<String> {
    let key = channel.to_lowercase();
    let persona = personas.get(&key)?;

    let mut block = format!(
        "## Active Persona Override (channel: {channel})\n"
    );

    if let Some(ref tone) = persona.tone {
        block.push_str(&format!("\n**Tone:** {tone}"));
    }
    if let Some(ref verbosity) = persona.verbosity {
        block.push_str(&format!("\n**Verbosity:** {verbosity}"));
    }
    if let Some(ref formality) = persona.formality {
        block.push_str(&format!("\n**Formality:** {formality}"));
    }
    if !persona.language_hints.is_empty() {
        block.push_str("\n**Language hints:**");
        for hint in &persona.language_hints {
            block.push_str(&format!("\n- {hint}"));
        }
    }

    block.push_str(
        "\n\nThese settings override your default tone for this conversation. Stay true to your \
         core identity (soul.md) — only adapt delivery style.",
    );

    Some(block)
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_persona(
        tone: Option<&str>,
        verbosity: Option<&str>,
        formality: Option<&str>,
        hints: Vec<&str>,
    ) -> PersonaConfig {
        PersonaConfig {
            tone: tone.map(|s| s.to_string()),
            verbosity: verbosity.map(|s| s.to_string()),
            formality: formality.map(|s| s.to_string()),
            language_hints: hints.into_iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn returns_none_for_unknown_channel() {
        let personas = HashMap::new();
        assert!(render_persona_block(&personas, "unknown").is_none());
    }

    #[test]
    fn returns_some_block_with_tone_verbosity_formality() {
        let mut personas = HashMap::new();
        personas.insert(
            "telegram".to_string(),
            make_persona(Some("casual"), Some("normal"), Some("casual"), vec![]),
        );
        let block = render_persona_block(&personas, "telegram").expect("expected Some");
        assert!(block.contains("casual"), "block should contain tone");
        assert!(block.contains("normal"), "block should contain verbosity");
    }

    #[test]
    fn case_insensitive_match() {
        let mut personas = HashMap::new();
        personas.insert(
            "telegram".to_string(),
            make_persona(Some("casual"), None, None, vec![]),
        );
        // "Telegram" with uppercase T should match the lowercase key
        let result = render_persona_block(&personas, "Telegram");
        assert!(result.is_some(), "expected case-insensitive match");
    }

    #[test]
    fn language_hints_appear_in_output() {
        let mut personas = HashMap::new();
        personas.insert(
            "discord".to_string(),
            make_persona(
                Some("technical"),
                Some("brief"),
                Some("casual"),
                vec!["code-first answers", "skip pleasantries"],
            ),
        );
        let block = render_persona_block(&personas, "discord").expect("expected Some");
        assert!(block.contains("code-first answers"));
        assert!(block.contains("skip pleasantries"));
    }
}
