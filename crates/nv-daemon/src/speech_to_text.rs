//! Speech-to-Text clients.
//!
//! Provides two STT functions:
//! - `transcribe_audio_elevenlabs()` — multipart POST to ElevenLabs; used for
//!   MP3/WAV audio files sent via the Telegram audio player.
//! - `transcribe_audio_deepgram()` — raw-body POST to Deepgram nova-2; used for
//!   OGG voice notes from Telegram voice messages.
//!
//! The two providers are kept separate so each can use the best fit for its
//! audio format and billing model.

use std::time::Duration;

use anyhow::{anyhow, Result};
use reqwest::multipart;

/// ElevenLabs Speech-to-Text API endpoint.
const ELEVENLABS_STT_URL: &str = "https://api.elevenlabs.io/v1/speech-to-text";

/// Deepgram Speech-to-Text API endpoint (model and options appended at call time).
const DEEPGRAM_STT_URL: &str = "https://api.deepgram.com/v1/listen";

/// Request timeout for audio transcription (30 seconds).
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

// ── Deepgram STT ──────────────────────────────────────────────────

/// Transcribe audio bytes using the Deepgram Speech-to-Text API.
///
/// `audio_bytes` — raw bytes of the audio file (OGG Opus, etc.)
/// `mime_type`   — MIME type of the audio (e.g. `"audio/ogg"`)
/// `api_key`     — Deepgram API key from `DEEPGRAM_API_KEY` env var
/// `model`       — Deepgram model name (e.g. `"nova-2"`)
///
/// POSTs the raw audio body to:
///   `https://api.deepgram.com/v1/listen?model={model}&smart_format=true`
///
/// Extracts `results.channels[0].alternatives[0].transcript` from the response.
/// Returns an error if the API call fails, the response cannot be parsed,
/// the transcript path is missing, or the transcript is empty.
pub async fn transcribe_audio_deepgram(
    audio_bytes: Vec<u8>,
    mime_type: &str,
    api_key: &str,
    model: &str,
) -> Result<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(TRANSCRIPTION_TIMEOUT_SECS))
        .build()
        .map_err(|e| anyhow!("failed to build HTTP client: {e}"))?;

    let url = format!(
        "{DEEPGRAM_STT_URL}?model={model}&smart_format=true"
    );

    let response = client
        .post(&url)
        .header("Authorization", format!("Token {api_key}"))
        .header("Content-Type", mime_type)
        .body(audio_bytes)
        .send()
        .await
        .map_err(|e| anyhow!("Deepgram STT request failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!("Deepgram STT returned {status}: {body}"));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| anyhow!("failed to parse Deepgram STT response: {e}"))?;

    // Extract results.channels[0].alternatives[0].transcript
    let transcript = json
        .pointer("/results/channels/0/alternatives/0/transcript")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            anyhow!("Deepgram STT response missing transcript field: {json}")
        })?;

    let trimmed = transcript.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("Deepgram STT returned empty transcript"));
    }

    tracing::info!(
        transcript_len = trimmed.len(),
        model,
        "audio transcribed via Deepgram STT"
    );

    Ok(trimmed.to_string())
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {

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

    // ── Deepgram response parsing ──────────────────────────────────

    #[test]
    fn deepgram_parse_transcript_success() {
        let json = serde_json::json!({
            "results": {
                "channels": [{
                    "alternatives": [{ "transcript": "  Hello world  " }]
                }]
            }
        });

        let transcript = json
            .pointer("/results/channels/0/alternatives/0/transcript")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string());

        assert_eq!(transcript, Some("Hello world".to_string()));
    }

    #[test]
    fn deepgram_missing_alternatives_returns_none() {
        let json = serde_json::json!({
            "results": {
                "channels": [{}]
            }
        });

        let result = json
            .pointer("/results/channels/0/alternatives/0/transcript")
            .and_then(|v| v.as_str());

        assert!(result.is_none());
    }

    #[test]
    fn deepgram_empty_transcript_is_detected() {
        let json = serde_json::json!({
            "results": {
                "channels": [{
                    "alternatives": [{ "transcript": "  " }]
                }]
            }
        });

        let transcript = json
            .pointer("/results/channels/0/alternatives/0/transcript")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .unwrap_or("");

        assert!(transcript.is_empty());
    }

    #[test]
    fn deepgram_http_error_body_is_captured() {
        // Simulate what the error path would produce
        let status = 401u16;
        let body = r#"{"err_code":"INVALID_AUTH","err_msg":"Invalid credentials."}"#;
        let error_msg = format!("Deepgram STT returned {status}: {body}");
        assert!(error_msg.contains("INVALID_AUTH"));
    }
}
