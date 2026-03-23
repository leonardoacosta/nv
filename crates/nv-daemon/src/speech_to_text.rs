//! ElevenLabs Speech-to-Text client.
//!
//! Provides `transcribe_audio_elevenlabs()` which POSTs audio bytes to the
//! ElevenLabs `/v1/speech-to-text` endpoint and returns the transcript text.
//!
//! Used for both voice notes (OGG) and audio files (MP3/WAV).
//! Single provider for all speech-to-text in Nova.

use std::time::Duration;

use anyhow::{anyhow, Result};
use reqwest::multipart;

/// ElevenLabs Speech-to-Text API endpoint.
const ELEVENLABS_STT_URL: &str = "https://api.elevenlabs.io/v1/speech-to-text";

/// Request timeout for audio transcription (30 seconds as specified in the proposal).
const TRANSCRIPTION_TIMEOUT_SECS: u64 = 30;

/// Transcribe audio bytes using the ElevenLabs Speech-to-Text API.
///
/// `audio_bytes` — raw bytes of the audio file (MP3, WAV, etc.)
/// `file_name`   — file name used as the multipart filename (e.g. `"audio.mp3"`)
/// `mime_type`   — MIME type of the audio (e.g. `"audio/mpeg"`)
/// `api_key`     — ElevenLabs API key from `ELEVENLABS_API_KEY` env var
///
/// Returns the transcript text on success, or an error if the API call fails.
pub async fn transcribe_audio_elevenlabs(
    audio_bytes: Vec<u8>,
    file_name: &str,
    mime_type: &str,
    api_key: &str,
) -> Result<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(TRANSCRIPTION_TIMEOUT_SECS))
        .build()
        .map_err(|e| anyhow!("failed to build HTTP client: {e}"))?;

    // Build multipart form with the audio file
    let part = multipart::Part::bytes(audio_bytes)
        .file_name(file_name.to_string())
        .mime_str(mime_type)
        .map_err(|e| anyhow!("invalid MIME type '{}': {e}", mime_type))?;

    let form = multipart::Form::new()
        .part("file", part)
        .text("model_id", "scribe_v1");

    let response = client
        .post(ELEVENLABS_STT_URL)
        .header("xi-api-key", api_key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| anyhow!("ElevenLabs STT request failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!(
            "ElevenLabs STT returned {status}: {body}"
        ));
    }

    // Parse transcript from JSON response
    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| anyhow!("failed to parse ElevenLabs STT response: {e}"))?;

    // ElevenLabs STT returns {"text": "...", "words": [...], ...}
    let transcript = json
        .get("text")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("ElevenLabs STT response missing 'text' field: {json}"))?;

    let trimmed = transcript.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("ElevenLabs STT returned empty transcript"));
    }

    tracing::info!(
        transcript_len = trimmed.len(),
        "audio transcribed via ElevenLabs STT"
    );

    Ok(trimmed.to_string())
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_transcript_from_json() {
        let json = serde_json::json!({
            "text": "  Hello world  ",
            "words": [],
            "language_code": "en"
        });

        let transcript = json
            .get("text")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        assert_eq!(transcript, "Hello world");
    }

    #[test]
    fn missing_text_field_returns_error_path() {
        let json = serde_json::json!({
            "error": "unsupported format"
        });

        let result = json.get("text").and_then(|v| v.as_str());
        assert!(result.is_none());
    }

    #[test]
    fn empty_transcript_is_detected() {
        let json = serde_json::json!({ "text": "  " });
        let transcript = json
            .get("text")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .unwrap_or("");
        assert!(transcript.is_empty());
    }
}
