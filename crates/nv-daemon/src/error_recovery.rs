//! Structured error classification, user-facing messages, and retry keyboard
//! for worker-level Claude API failures.
//!
//! This module provides the `NovaError` enum and its associated helpers used by
//! `worker.rs` to classify raw errors, generate human-readable Telegram messages,
//! and build the inline "Retry" keyboard shown on final failure.

use nv_core::types::{InlineButton, InlineKeyboard};

// ── Error Classification ─────────────────────────────────────────────

/// Structured error variants covering all known Claude API failure modes.
#[derive(Debug, Clone, PartialEq)]
pub enum NovaError {
    /// HTTP 429 or CLI rate-limit message.
    RateLimit { retry_after_secs: Option<u64> },
    /// HTTP 529 / "overloaded" from Anthropic.
    ApiOverloaded,
    /// Network timeout or process timeout.
    Timeout,
    /// Auth failure — credentials invalid/missing.
    AuthFailure,
    /// Process crash (Broken pipe, EOF, process died).
    ProcessCrash,
    /// Unclassified error.
    Unknown { message: String },
}

/// Classify an `anyhow::Error` into a `NovaError` variant.
///
/// Uses substring matching on the error string display, mirroring the
/// existing patterns in `worker.rs` and extending them with 429/529
/// status code detection.
pub fn classify_error(e: &anyhow::Error) -> NovaError {
    let s = e.to_string();

    // Auth failures — non-retryable, checked first to avoid false positives.
    if s.contains("Not logged in")
        || s.contains("not logged in")
        || s.contains("invalid x-api-key")
        || s.contains("authentication_error")
        || s.contains("AuthError")
    {
        return NovaError::AuthFailure;
    }

    // Rate limit — HTTP 429 or CLI message.
    if s.contains("429")
        || s.contains("hit your limit")
        || s.contains("rate limit")
        || s.contains("rate_limit")
        || s.contains("Rate limit")
        || s.contains("RateLimit")
    {
        return NovaError::RateLimit {
            retry_after_secs: None,
        };
    }

    // API overloaded — HTTP 529 or "overloaded" message.
    if s.contains("529")
        || s.contains("overloaded")
        || s.contains("Overloaded")
        || s.contains("api_overloaded")
    {
        return NovaError::ApiOverloaded;
    }

    // Timeout.
    if s.contains("Timeout")
        || s.contains("timeout")
        || s.contains("timed out")
        || s.contains("TimedOut")
    {
        return NovaError::Timeout;
    }

    // Process crash — broken pipe, EOF, process died.
    if s.contains("Broken pipe")
        || s.contains("broken pipe")
        || s.contains("EOF while parsing")
        || s.contains("EOF")
        || s.contains("closed stdout")
        || s.contains("process died")
        || s.contains("process exited")
    {
        return NovaError::ProcessCrash;
    }

    NovaError::Unknown {
        message: s.chars().take(200).collect(),
    }
}

/// Returns `true` when the error class is transient and should be retried.
pub fn is_retryable(error: &NovaError) -> bool {
    matches!(
        error,
        NovaError::RateLimit { .. }
            | NovaError::ApiOverloaded
            | NovaError::Timeout
            | NovaError::ProcessCrash
    )
}

// ── User Messages ────────────────────────────────────────────────────

/// Return a human-readable, emoji-free message for a `NovaError`.
///
/// `attempt` is the 1-based attempt number just completed.
/// `max_attempts` is the total number of attempts allowed (e.g. 3).
/// When `attempt < max_attempts` the message is a brief "retrying" notice;
/// when `attempt >= max_attempts` it is the final failure message.
pub fn user_message(error: &NovaError, attempt: u32, max_attempts: u32) -> String {
    let is_final = attempt >= max_attempts;

    match error {
        NovaError::RateLimit { .. } => {
            if is_final {
                "I've hit my usage limit. Try again shortly.".to_string()
            } else {
                "I've hit my usage limit — retrying in a moment.".to_string()
            }
        }
        NovaError::ApiOverloaded => {
            if is_final {
                "The API is still overloaded. Try again in a moment.".to_string()
            } else {
                "The API is overloaded — retrying.".to_string()
            }
        }
        NovaError::Timeout => {
            if is_final {
                "That took too long. Try a shorter request.".to_string()
            } else {
                "That's taking longer than expected — retrying.".to_string()
            }
        }
        NovaError::AuthFailure => {
            // Auth failures are never retried — message is always final.
            "Authentication issue — check Claude CLI credentials.".to_string()
        }
        NovaError::ProcessCrash => {
            if is_final {
                "Something went wrong. Please try again.".to_string()
            } else {
                "Something went wrong — retrying.".to_string()
            }
        }
        NovaError::Unknown { .. } => "Something went wrong. Please try again.".to_string(),
    }
}

// ── Retry Keyboard ───────────────────────────────────────────────────

/// Build a single-button "Retry" inline keyboard.
///
/// The button's `callback_data` is `retry:{task_slug}`, which the orchestrator
/// routes to a new `WorkerTask` at `Priority::High`.
pub fn retry_keyboard(task_slug: &str) -> InlineKeyboard {
    InlineKeyboard {
        rows: vec![vec![InlineButton {
            text: "Retry".to_string(),
            callback_data: format!("retry:{task_slug}"),
        }]],
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_err(msg: &str) -> anyhow::Error {
        anyhow::anyhow!("{}", msg)
    }

    // ── classify_error ───────────────────────────────────────────────

    #[test]
    fn classify_rate_limit_hit_your_limit() {
        let e = make_err("Claude error: hit your limit for this hour");
        assert!(matches!(classify_error(&e), NovaError::RateLimit { .. }));
    }

    #[test]
    fn classify_rate_limit_429() {
        let e = make_err("HTTP 429 Too Many Requests");
        assert!(matches!(classify_error(&e), NovaError::RateLimit { .. }));
    }

    #[test]
    fn classify_rate_limit_rate_limit_string() {
        let e = make_err("rate limit exceeded");
        assert!(matches!(classify_error(&e), NovaError::RateLimit { .. }));
    }

    #[test]
    fn classify_api_overloaded_529() {
        let e = make_err("HTTP 529 API overloaded");
        assert!(matches!(classify_error(&e), NovaError::ApiOverloaded));
    }

    #[test]
    fn classify_api_overloaded_string() {
        let e = make_err("Anthropic API is overloaded, please retry");
        assert!(matches!(classify_error(&e), NovaError::ApiOverloaded));
    }

    #[test]
    fn classify_timeout_uppercase() {
        let e = make_err("Timeout waiting for response");
        assert!(matches!(classify_error(&e), NovaError::Timeout));
    }

    #[test]
    fn classify_timeout_timed_out() {
        let e = make_err("request timed out after 300s");
        assert!(matches!(classify_error(&e), NovaError::Timeout));
    }

    #[test]
    fn classify_process_crash_broken_pipe() {
        let e = make_err("Broken pipe (os error 32)");
        assert!(matches!(classify_error(&e), NovaError::ProcessCrash));
    }

    #[test]
    fn classify_process_crash_eof() {
        let e = make_err("EOF while parsing response stream");
        assert!(matches!(classify_error(&e), NovaError::ProcessCrash));
    }

    #[test]
    fn classify_process_crash_process_died() {
        let e = make_err("CLI process died unexpectedly");
        assert!(matches!(classify_error(&e), NovaError::ProcessCrash));
    }

    #[test]
    fn classify_auth_not_logged_in() {
        let e = make_err("Not logged in — run `claude login`");
        assert!(matches!(classify_error(&e), NovaError::AuthFailure));
    }

    #[test]
    fn classify_auth_authentication_error() {
        let e = make_err("authentication_error: invalid key");
        assert!(matches!(classify_error(&e), NovaError::AuthFailure));
    }

    #[test]
    fn classify_unknown_generic() {
        let e = make_err("something completely unrecognised happened");
        assert!(matches!(classify_error(&e), NovaError::Unknown { .. }));
    }

    // ── user_message ─────────────────────────────────────────────────

    /// Returns `true` when a char falls in a known emoji Unicode block.
    fn is_emoji(ch: char) -> bool {
        let cp = ch as u32;
        // Emoticons, Misc Symbols & Pictographs, Transport & Map, Supplemental Symbols,
        // Dingbats, Enclosed Alphanumeric Supplement (regional indicators / number keycaps)
        matches!(cp,
            0x2600..=0x26FF    // Misc symbols (classic)
            | 0x2700..=0x27BF  // Dingbats
            | 0x1F300..=0x1F5FF // Misc symbols & pictographs
            | 0x1F600..=0x1F64F // Emoticons
            | 0x1F680..=0x1F6FF // Transport & map
            | 0x1F700..=0x1F77F // Alchemical symbols
            | 0x1F900..=0x1F9FF // Supplemental symbols & pictographs
            | 0x1FA00..=0x1FA6F // Chess symbols
            | 0x1FA70..=0x1FAFF // Symbols & pictographs extended-A
            | 0x231A..=0x231B   // Watch, hourglass
            | 0x23E9..=0x23F3   // Fast-forward / rewind / clocks
            | 0x23F8..=0x23FA   // Pause / stop / record
            | 0x25AA..=0x25AB   // Small squares
            | 0x25B6             // Play button
            | 0x25C0             // Reverse button
            | 0x25FB..=0x25FE   // Medium squares
            | 0x2614..=0x2615   // Umbrella / hot beverage
            | 0x2648..=0x2653   // Zodiac
            | 0x267F             // Wheelchair
            | 0x2693             // Anchor
            | 0x26A1             // Lightning
            | 0x26AA..=0x26AB   // Circles
            | 0x26BD..=0x26BE   // Ball & baseball
            | 0x26C4..=0x26C5   // Snowman & sun
            | 0x26CE..=0x26CF   // Ophiuchus & pickaxe
            | 0x26D4             // No entry
            | 0x26EA             // Church
            | 0x26F2..=0x26F3   // Fountain & golf
            | 0x26F5             // Sailboat
            | 0x26FA             // Tent
            | 0x26FD             // Fuel pump
            | 0x2702             // Scissors
            | 0x2705             // Check mark
            | 0x2708..=0x270D   // Plane / envelope / scissors / pen
            | 0x270F             // Pencil
            | 0x2712             // Black nib
            | 0x2714             // Check mark
            | 0x2716             // Cross mark
            | 0x271D             // Latin cross
            | 0x2721             // Star of David
            | 0x2728             // Sparkles
            | 0x2733..=0x2734   // Eight-pointed stars
            | 0x2744             // Snowflake
            | 0x2747             // Sparkle
            | 0x274C             // Cross mark (red X)
            | 0x274E             // Cross mark
            | 0x2753..=0x2755   // Question marks
            | 0x2757             // Exclamation
            | 0x2763..=0x2764   // Hearts
            | 0x2795..=0x2797   // Plus / minus / division
            | 0x27A1             // Arrow
            | 0x27B0             // Curly loop
            | 0x27BF             // Double curly loop
            | 0x2934..=0x2935   // Arrows
            | 0x2B05..=0x2B07   // Arrows
            | 0x2B1B..=0x2B1C   // Squares
            | 0x2B50             // Star
            | 0x2B55             // Circle
            | 0x3030             // Wavy dash
            | 0x303D             // Part alternation mark
            | 0x3297             // Circled ideograph congratulation
            | 0x3299             // Circled ideograph secret
            | 0x1F004            // Mahjong tile
            | 0x1F0CF            // Joker
            | 0x1F170..=0x1F171 // Blood type
            | 0x1F17E..=0x1F17F // Blood type
            | 0x1F18E            // Blood type
            | 0x1F191..=0x1F19A // Squared words
            | 0x1F1E0..=0x1F1FF // Regional indicator letters (flags)
            | 0x1F201..=0x1F202 // Squared CJK
            | 0x1F21A            // Squared CJK
            | 0x1F22F            // Squared CJK
            | 0x1F232..=0x1F23A // Squared CJK
            | 0x1F250..=0x1F251 // Circled ideograph
        )
    }

    #[test]
    fn user_message_all_variants_non_empty_no_emoji() {
        let variants = vec![
            NovaError::RateLimit { retry_after_secs: None },
            NovaError::ApiOverloaded,
            NovaError::Timeout,
            NovaError::AuthFailure,
            NovaError::ProcessCrash,
            NovaError::Unknown { message: "oops".to_string() },
        ];
        for error in &variants {
            let retry_msg = user_message(error, 1, 3);
            let final_msg = user_message(error, 3, 3);
            assert!(!retry_msg.is_empty(), "retry message empty for {error:?}");
            assert!(!final_msg.is_empty(), "final message empty for {error:?}");
            // No emojis in user-facing messages (project convention).
            // Non-emoji Unicode (e.g. em-dash) is permitted.
            for ch in retry_msg.chars() {
                assert!(!is_emoji(ch), "emoji char U+{:04X} in retry message for {error:?}", ch as u32);
            }
            for ch in final_msg.chars() {
                assert!(!is_emoji(ch), "emoji char U+{:04X} in final message for {error:?}", ch as u32);
            }
        }
    }

    #[test]
    fn user_message_retry_vs_final_differ_for_rate_limit() {
        let e = NovaError::RateLimit { retry_after_secs: None };
        assert_ne!(user_message(&e, 1, 3), user_message(&e, 3, 3));
    }

    #[test]
    fn user_message_auth_always_same() {
        // Auth failure message is the same regardless of attempt number.
        assert_eq!(
            user_message(&NovaError::AuthFailure, 1, 3),
            user_message(&NovaError::AuthFailure, 3, 3)
        );
    }

    // ── retry_keyboard ───────────────────────────────────────────────

    #[test]
    fn retry_keyboard_single_row_single_button() {
        let kb = retry_keyboard("check-git-status");
        assert_eq!(kb.rows.len(), 1);
        assert_eq!(kb.rows[0].len(), 1);
    }

    #[test]
    fn retry_keyboard_callback_data_starts_with_retry_prefix() {
        let kb = retry_keyboard("my-task-slug");
        assert!(kb.rows[0][0].callback_data.starts_with("retry:"));
    }

    #[test]
    fn retry_keyboard_callback_data_contains_slug() {
        let slug = "summarise-pr-review";
        let kb = retry_keyboard(slug);
        assert_eq!(kb.rows[0][0].callback_data, format!("retry:{slug}"));
    }

    #[test]
    fn retry_keyboard_button_text_is_retry() {
        let kb = retry_keyboard("slug");
        assert_eq!(kb.rows[0][0].text, "Retry");
    }
}
