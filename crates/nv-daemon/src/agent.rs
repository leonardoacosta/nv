use std::collections::HashMap;
use std::sync::Arc;

use nv_core::channel::Channel;
use nv_core::types::Trigger;
use nv_core::Config;

use crate::contact_store::inject_contact_profiles;

// ── Constants ────────────────────────────────────────────────────────

const DEFAULT_SYSTEM_PROMPT: &str = r#"You are Nova, an operations daemon. Your identity, personality, and operator details are loaded from separate files. This file contains operational rules only.

## Dispatch Test
Before every response, classify internally:
- Command ("create", "assign", "move") → Draft action, present for confirmation
- Query ("what's", "status of", "how many") → Gather tools, synthesize answer
- Digest (cron trigger) → Gather, gate, format or suppress
- Chat ("thanks", "ok") → Reply in ≤10 words

## Tool Use
Use tools proactively. Don't ask permission for reads. Don't describe tools to the operator.
- Reads (immediate): read_memory, search_memory, get_recent_messages, jira_search, jira_get, query_nexus, query_session, vercel_deployments, vercel_logs, list_channels
- Writes (confirm first): jira_create, jira_transition, jira_assign, jira_comment, send_to_channel
- Memory writes (autonomous): write_memory
- Bootstrap (one-time): complete_bootstrap
- Soul (rare): update_soul — always notify operator

## Response Rules
1. Lead with the answer. No filler.
2. Cite sources: [Jira: OO-142], [Memory: decisions], [Nexus: homelab]
3. Errors are one line.
4. Omit empty sections.
5. Suggest 1-3 next actions.

## NEVER
- Start with "Great", "Certainly", "Sure", "I'd be happy to", "Of course"
- Explain your architecture or internal state
- Apologize for tool errors or service outages
- Send a digest with nothing actionable
- Mention tool names to the operator

## Summary Tag
End every response with: [SUMMARY: <past-tense action, ≤120 chars>]"#;

/// Load the system prompt — override from file, or fall back to default.
pub fn load_system_prompt() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let override_path = std::path::Path::new(&home).join(".nv").join("system-prompt.md");
    if let Ok(contents) = std::fs::read_to_string(&override_path) {
        tracing::info!(path = %override_path.display(), "loaded custom system prompt");
        contents
    } else {
        tracing::debug!("using default system prompt");
        DEFAULT_SYSTEM_PROMPT.to_string()
    }
}

/// Load an optional file from `~/.nv/<name>`.
///
/// Returns `None` if the file does not exist or cannot be read.
fn load_file_optional(name: &str) -> Option<String> {
    let home = std::env::var("HOME").unwrap_or_default();
    let path = std::path::Path::new(&home).join(".nv").join(name);
    match std::fs::read_to_string(&path) {
        Ok(contents) => {
            tracing::debug!(file = name, "loaded optional config file");
            Some(contents)
        }
        Err(_) => None,
    }
}

/// Check whether the bootstrap has been completed.
///
/// Returns `true` if `~/.nv/bootstrap-state.json` exists.
pub fn check_bootstrap_state() -> bool {
    let home = std::env::var("HOME").unwrap_or_default();
    let path = std::path::Path::new(&home)
        .join(".nv")
        .join("bootstrap-state.json");
    path.exists()
}

/// Build the full system context by concatenating the system prompt
/// with identity/soul/user files (normal mode) or bootstrap instructions
/// (first-run mode).
///
/// Also injects a listing of available memory files so Nova always knows
/// what context files exist before calling `read_memory`.
///
/// When `channel` is `Some`, a per-channel persona override block is appended
/// after `soul.md` if a matching entry exists in `[personas]` of `nv.toml`.
/// Cron and CLI triggers pass `None` and receive the default soul.md persona.
pub fn build_system_context(channel: Option<&str>) -> String {
    let mut context = load_system_prompt();

    if check_bootstrap_state() {
        // Normal mode — load identity + soul + user
        if let Some(identity) = load_file_optional("identity.md") {
            context.push_str("\n\n");
            context.push_str(&identity);
        }
        if let Some(soul) = load_file_optional("soul.md") {
            context.push_str("\n\n");
            context.push_str(&soul);
        }

        // Inject per-channel persona override block after soul.md (when channel is provided).
        if let Some(ch) = channel {
            match nv_core::Config::load() {
                Ok(config) => {
                    if let Some(ref block) = crate::persona::render_persona_block(&config.personas, ch) {
                        context.push_str("\n\n");
                        context.push_str(&block);
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        channel = ch,
                        error = %e,
                        "failed to load config for persona injection; using default persona"
                    );
                }
            }
        }

        if let Some(user) = load_file_optional("user.md") {
            context.push_str("\n\n");
            context.push_str(&user);
        }

        // Inject available memory file listing for reliable memory reads
        let memory_listing = list_memory_files();
        if !memory_listing.is_empty() {
            context.push_str("\n\n## Available Memory Files\n\n");
            context.push_str("The following files are available in `~/.nv/memory/`. ");
            context.push_str("Use `read_memory` or `search_memory` to access them:\n\n");
            context.push_str(&memory_listing);
        }

        // Inject contact profiles from config/contact/*.md (skips example-* files).
        // Respects the [contacts] section in nv.toml; defaults to profile_dir="config/contact"
        // and inject_in_context=true when the section is absent.
        let contact_injection = build_contact_context();
        if let Some(contacts_section) = contact_injection {
            context.push_str("\n\n");
            context.push_str(&contacts_section);
        }
    } else {
        // Bootstrap mode — load bootstrap instructions instead
        if let Some(bootstrap) = load_file_optional("bootstrap.md") {
            context.push_str("\n\n");
            context.push_str(&bootstrap);
        }
    }

    context
}

/// List available memory files in `~/.nv/memory/`, formatted as a bullet list.
///
/// Returns a markdown bullet list of filenames (`.md` files only), or an
/// empty string if the directory is missing or contains no markdown files.
fn list_memory_files() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let memory_dir = std::path::Path::new(&home).join(".nv").join("memory");

    let entries = match std::fs::read_dir(&memory_dir) {
        Ok(e) => e,
        Err(_) => return String::new(),
    };

    let mut files: Vec<String> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().into_string().ok()?;
            if name.ends_with(".md") {
                Some(name)
            } else {
                None
            }
        })
        .collect();

    if files.is_empty() {
        return String::new();
    }

    files.sort();

    files
        .iter()
        .map(|f| format!("- `{f}`"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Build the contact profiles section for system context injection.
///
/// Reads the `[contacts]` section of `nv.toml` to determine the profile
/// directory and whether injection is enabled. Falls back to
/// `config/contact` + `inject_in_context = true` when the section is absent.
///
/// The `profile_dir` is resolved relative to the current working directory
/// (i.e. the project root from which `nv-daemon` is run).
fn build_contact_context() -> Option<String> {
    // Load config to check [contacts] settings (best-effort; silently skip on error).
    let (profile_dir, inject) = match Config::load() {
        Ok(cfg) => {
            let inject = cfg
                .contacts
                .as_ref()
                .map(|c| c.inject_in_context)
                .unwrap_or(true);
            let dir = cfg
                .contacts
                .as_ref()
                .map(|c| c.profile_dir.clone())
                .unwrap_or_else(|| "config/contact".to_string());
            (dir, inject)
        }
        Err(_) => ("config/contact".to_string(), true),
    };

    // Resolve relative to the current working directory (project root).
    let resolved = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join(&profile_dir);
    inject_contact_profiles(&resolved, inject)
}

// ── Channel Registry ────────────────────────────────────────────────

/// Maps channel names to their implementations for outbound routing.
pub type ChannelRegistry = HashMap<String, Arc<dyn Channel>>;

// ── Trigger Formatting ──────────────────────────────────────────────

/// Format a batch of triggers into a structured text message for Claude.
pub fn format_trigger_batch(triggers: &[Trigger]) -> String {
    let mut parts = Vec::new();
    for trigger in triggers {
        match trigger {
            Trigger::Message(msg) => {
                parts.push(format!(
                    "[{}] {} from @{}: {}",
                    msg.channel,
                    msg.timestamp.format("%H:%M"),
                    msg.sender,
                    msg.content
                ));
            }
            Trigger::Cron(event) => {
                parts.push(format!("[cron] {event:?} triggered"));
            }
            Trigger::NexusEvent(event) => {
                parts.push(format!(
                    "[nexus] {} session {} — {:?}{}",
                    event.agent_name,
                    event.session_id,
                    event.event_type,
                    event
                        .details
                        .as_ref()
                        .map(|d| format!(": {d}"))
                        .unwrap_or_default()
                ));
            }
            Trigger::CliCommand(req) => {
                parts.push(format!("[cli] {:?}", req.command));
            }
            Trigger::ObligationResearch(rt) => {
                parts.push(format!(
                    "[research] obligation {} — {}",
                    rt.obligation_id, rt.detected_action
                ));
            }
        }
    }
    parts.join("\n")
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use nv_core::types::{
        CliCommand, CliRequest, CronEvent, InboundMessage, SessionEvent, SessionEventType,
    };
    use crate::claude::Message;
    use crate::conversation::{MAX_HISTORY_CHARS, MAX_HISTORY_TURNS};

    #[test]
    fn format_trigger_batch_message() {
        let triggers = vec![Trigger::Message(InboundMessage {
            id: "1".into(),
            channel: "telegram".into(),
            sender: "leo".into(),
            content: "hello NV".into(),
            timestamp: Utc::now(),
            thread_id: None,
            metadata: serde_json::json!({}),
        })];

        let text = format_trigger_batch(&triggers);
        assert!(text.contains("[telegram]"));
        assert!(text.contains("@leo"));
        assert!(text.contains("hello NV"));
    }

    #[test]
    fn format_trigger_batch_cron() {
        let triggers = vec![Trigger::Cron(CronEvent::Digest)];
        let text = format_trigger_batch(&triggers);
        assert!(text.contains("[cron]"));
        assert!(text.contains("Digest"));
    }

    #[test]
    fn format_trigger_batch_nexus() {
        let triggers = vec![Trigger::NexusEvent(SessionEvent {
            agent_name: "builder".into(),
            session_id: "s-1".into(),
            event_type: SessionEventType::Completed,
            details: Some("all tests passed".into()),
        })];
        let text = format_trigger_batch(&triggers);
        assert!(text.contains("[nexus]"));
        assert!(text.contains("builder"));
        assert!(text.contains("Completed"));
        assert!(text.contains("all tests passed"));
    }

    #[test]
    fn format_trigger_batch_cli() {
        let triggers = vec![Trigger::CliCommand(CliRequest {
            command: CliCommand::Status,
            response_tx: None,
        })];
        let text = format_trigger_batch(&triggers);
        assert!(text.contains("[cli]"));
        assert!(text.contains("Status"));
    }

    #[test]
    fn format_trigger_batch_multiple() {
        let triggers = vec![
            Trigger::Message(InboundMessage {
                id: "1".into(),
                channel: "telegram".into(),
                sender: "leo".into(),
                content: "first".into(),
                timestamp: Utc::now(),
                thread_id: None,
                metadata: serde_json::json!({}),
            }),
            Trigger::Message(InboundMessage {
                id: "2".into(),
                channel: "telegram".into(),
                sender: "leo".into(),
                content: "second".into(),
                timestamp: Utc::now(),
                thread_id: None,
                metadata: serde_json::json!({}),
            }),
            Trigger::Cron(CronEvent::MemoryCleanup),
        ];

        let text = format_trigger_batch(&triggers);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].contains("first"));
        assert!(lines[1].contains("second"));
        assert!(lines[2].contains("MemoryCleanup"));
    }

    #[test]
    fn truncate_history_under_limit() {
        let mut history = vec![
            Message::user("hello"),
            Message::user("world"),
        ];
        crate::conversation::truncate_history(&mut history);
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn truncate_history_over_turn_limit() {
        let mut history: Vec<Message> = (0..30)
            .map(|i| Message::user(format!("message {i}")))
            .collect();
        crate::conversation::truncate_history(&mut history);
        assert_eq!(history.len(), MAX_HISTORY_TURNS);
        // Should keep the newest messages
        match &history.last().unwrap().content {
            crate::claude::MessageContent::Text(t) => assert_eq!(t, "message 29"),
            _ => panic!("expected text"),
        }
    }

    #[test]
    fn truncate_history_over_char_limit() {
        // Create messages that exceed MAX_HISTORY_CHARS
        let big_msg = "x".repeat(20_000);
        let mut history = vec![
            Message::user(big_msg.clone()),
            Message::user(big_msg.clone()),
            Message::user(big_msg.clone()),
            Message::user("recent message"),
        ];
        crate::conversation::truncate_history(&mut history);
        // Should have dropped old messages but kept at least 2
        assert!(history.len() >= 2);
        let total: usize = history.iter().map(|m| m.content_len()).sum();
        assert!(total <= MAX_HISTORY_CHARS || history.len() == 2);
    }

    #[test]
    fn truncate_history_keeps_minimum_two() {
        let big_msg = "x".repeat(40_000);
        let mut history = vec![
            Message::user(big_msg.clone()),
            Message::user(big_msg),
        ];
        crate::conversation::truncate_history(&mut history);
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn load_system_prompt_returns_default() {
        // In a test environment without ~/.nv/system-prompt.md, should return default
        let prompt = load_system_prompt();
        assert!(prompt.contains("Nova"));
        assert!(prompt.contains("Dispatch Test"));
        assert!(prompt.contains("Tool Use"));
    }

    #[test]
    fn build_system_context_includes_system_prompt() {
        let context = build_system_context(None);
        // Should always contain the system prompt content
        assert!(context.contains("Nova"));
        assert!(context.contains("Dispatch Test"));
    }

    #[test]
    fn check_bootstrap_state_returns_false_when_missing() {
        // In a test environment, bootstrap-state.json shouldn't exist
        // (unless running on the dev machine with ~/.nv/ set up)
        // This test just verifies the function doesn't panic
        let _ = check_bootstrap_state();
    }

    #[test]
    fn load_file_optional_returns_none_for_missing() {
        let result = load_file_optional("nonexistent-file-abc123.md");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn drain_triggers_single() {
        use tokio::sync::mpsc;
        let (tx, rx) = mpsc::unbounded_channel::<Trigger>();
        let mut agent_rx = rx;

        tx.send(Trigger::Cron(CronEvent::Digest)).unwrap();
        drop(tx);

        // Manually drain
        let first = agent_rx.recv().await.unwrap();
        let mut batch = vec![first];
        while let Ok(trigger) = agent_rx.try_recv() {
            batch.push(trigger);
        }
        assert_eq!(batch.len(), 1);
    }

    #[tokio::test]
    async fn drain_triggers_batch() {
        use tokio::sync::mpsc;
        let (tx, rx) = mpsc::unbounded_channel::<Trigger>();
        let mut agent_rx = rx;

        // Send 5 triggers before draining
        for _ in 0..5 {
            tx.send(Trigger::Cron(CronEvent::Digest)).unwrap();
        }
        drop(tx);

        let first = agent_rx.recv().await.unwrap();
        let mut batch = vec![first];
        while let Ok(trigger) = agent_rx.try_recv() {
            batch.push(trigger);
        }
        assert_eq!(batch.len(), 5);
    }

    #[tokio::test]
    async fn drain_triggers_channel_closed() {
        use tokio::sync::mpsc;
        let (tx, rx) = mpsc::unbounded_channel::<Trigger>();
        let mut agent_rx = rx;

        drop(tx); // Close immediately

        let result = agent_rx.recv().await;
        assert!(result.is_none());
    }
}
