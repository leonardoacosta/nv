//! Voice-to-text transcription via Deepgram API.
//!
//! Downloads Telegram voice messages, sends them to Deepgram for
//! transcription, and returns the text for injection as `Trigger::Message`.
#![allow(dead_code)]

use anyhow::{bail, Result};
use reqwest::Client;
use serde::Deserialize;

/// Deepgram transcription response (simplified).
#[derive(Debug, Deserialize)]
struct DeepgramResponse {
    results: Option<DeepgramResults>,
}

#[derive(Debug, Deserialize)]
struct DeepgramResults {
    channels: Vec<DeepgramChannel>,
}

#[derive(Debug, Deserialize)]
struct DeepgramChannel {
    alternatives: Vec<DeepgramAlternative>,
}

#[derive(Debug, Deserialize)]
struct DeepgramAlternative {
    transcript: String,
}

/// Transcribe audio bytes via the Deepgram API.
///
/// Posts raw audio to `api.deepgram.com/v1/listen` and extracts the
/// transcript string from the response.
///
/// Returns:
/// - `Ok(transcript)` on success (may be empty if no speech detected)
/// - `Err(...)` on API or network error
pub async fn transcribe_voice(
    audio_bytes: &[u8],
    mime_type: &str,
    api_key: &str,
    model: &str,
) -> Result<String> {
    let client = Client::new();
    let url = format!(
        "https://api.deepgram.com/v1/listen?model={model}&smart_format=true"
    );

    let resp = client
        .post(&url)
        .header("Authorization", format!("Token {api_key}"))
        .header("Content-Type", mime_type)
        .body(audio_bytes.to_vec())
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        bail!("Deepgram API error ({status}): {body}");
    }

    let dg_resp: DeepgramResponse = resp.json().await?;

    let transcript = dg_resp
        .results
        .and_then(|r| r.channels.into_iter().next())
        .and_then(|c| c.alternatives.into_iter().next())
        .map(|a| a.transcript)
        .unwrap_or_default();

    Ok(transcript)
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deepgram_response_parsing() {
        let json = r#"{
            "results": {
                "channels": [{
                    "alternatives": [{
                        "transcript": "Check the status of tribal cities"
                    }]
                }]
            }
        }"#;

        let resp: DeepgramResponse = serde_json::from_str(json).unwrap();
        let transcript = resp
            .results
            .unwrap()
            .channels
            .into_iter()
            .next()
            .unwrap()
            .alternatives
            .into_iter()
            .next()
            .unwrap()
            .transcript;

        assert_eq!(transcript, "Check the status of tribal cities");
    }

    #[test]
    fn deepgram_response_empty_transcript() {
        let json = r#"{
            "results": {
                "channels": [{
                    "alternatives": [{
                        "transcript": ""
                    }]
                }]
            }
        }"#;

        let resp: DeepgramResponse = serde_json::from_str(json).unwrap();
        let transcript = resp
            .results
            .unwrap()
            .channels
            .into_iter()
            .next()
            .unwrap()
            .alternatives
            .into_iter()
            .next()
            .unwrap()
            .transcript;

        assert!(transcript.is_empty());
    }

    #[test]
    fn deepgram_response_no_results() {
        let json = r#"{ "results": null }"#;
        let resp: DeepgramResponse = serde_json::from_str(json).unwrap();
        assert!(resp.results.is_none());
    }
}
