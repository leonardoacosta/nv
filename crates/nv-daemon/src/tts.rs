use std::time::Duration;

use anyhow::{bail, Context};
use reqwest::Client;
use tokio::process::Command;

/// ElevenLabs TTS client configuration.
#[derive(Clone)]
pub struct TtsClient {
    http: Client,
    api_key: String,
    voice_id: String,
    model_id: String,
}

impl TtsClient {
    /// Create a new TTS client.
    pub fn new(api_key: &str, voice_id: &str, model_id: &str) -> Self {
        Self {
            http: Client::new(),
            api_key: api_key.to_string(),
            voice_id: voice_id.to_string(),
            model_id: model_id.to_string(),
        }
    }

    /// Call the ElevenLabs TTS API to synthesize text into MP3 bytes.
    ///
    /// POST /v1/text-to-speech/{voice_id}
    /// Returns raw MP3 audio bytes.
    async fn synthesize_mp3(&self, text: &str) -> anyhow::Result<Vec<u8>> {
        let url = format!(
            "https://api.elevenlabs.io/v1/text-to-speech/{}",
            self.voice_id
        );

        let body = serde_json::json!({
            "text": text,
            "model_id": self.model_id,
            "voice_settings": {
                "stability": 0.5,
                "similarity_boost": 0.75
            }
        });

        let resp = self
            .http
            .post(&url)
            .header("xi-api-key", &self.api_key)
            .header("Content-Type", "application/json")
            .header("Accept", "audio/mpeg")
            .json(&body)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .context("ElevenLabs API request failed")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let error_body = resp.text().await.unwrap_or_default();
            bail!(
                "ElevenLabs API returned {}: {}",
                status,
                error_body
            );
        }

        let bytes = resp.bytes().await.context("failed to read ElevenLabs response body")?;
        Ok(bytes.to_vec())
    }
}

/// Transcode MP3 bytes to OGG/Opus via ffmpeg.
///
/// Spawns `ffmpeg -i pipe:0 -c:a libopus -f ogg pipe:1` and pipes
/// MP3 through stdin, collecting OGG from stdout.
async fn transcode_to_ogg(mp3_bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut child = Command::new("ffmpeg")
        .args([
            "-i", "pipe:0",
            "-c:a", "libopus",
            "-b:a", "64k",
            "-vn",
            "-f", "ogg",
            "pipe:1",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("failed to spawn ffmpeg — is it installed?")?;

    // Write MP3 to stdin
    if let Some(mut stdin) = child.stdin.take() {
        use tokio::io::AsyncWriteExt;
        stdin
            .write_all(mp3_bytes)
            .await
            .context("failed to write to ffmpeg stdin")?;
        // Drop stdin to close the pipe and let ffmpeg finish
    }

    let output = child
        .wait_with_output()
        .await
        .context("failed to wait for ffmpeg")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("ffmpeg transcode failed: {}", stderr);
    }

    Ok(output.stdout)
}

/// Synthesize text to OGG/Opus audio bytes.
///
/// Calls ElevenLabs TTS API (returns MP3), then transcodes to OGG/Opus
/// via ffmpeg. Returns the OGG bytes ready for Telegram's sendVoice.
pub async fn synthesize(client: &TtsClient, text: &str) -> anyhow::Result<Vec<u8>> {
    let mp3_bytes = client
        .synthesize_mp3(text)
        .await
        .context("TTS synthesis failed")?;

    tracing::debug!(mp3_size = mp3_bytes.len(), "ElevenLabs MP3 received");

    let ogg_bytes = transcode_to_ogg(&mp3_bytes)
        .await
        .context("OGG transcode failed")?;

    tracing::debug!(ogg_size = ogg_bytes.len(), "OGG/Opus transcode complete");

    Ok(ogg_bytes)
}

/// Check if ffmpeg is available in PATH.
///
/// Returns `true` if `ffmpeg -version` exits successfully.
pub async fn check_ffmpeg() -> bool {
    match Command::new("ffmpeg")
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
    {
        Ok(status) => status.success(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tts_client_creation() {
        let client = TtsClient::new("test-key", "voice-123", "eleven_multilingual_v2");
        assert_eq!(client.api_key, "test-key");
        assert_eq!(client.voice_id, "voice-123");
        assert_eq!(client.model_id, "eleven_multilingual_v2");
    }

    #[tokio::test]
    async fn check_ffmpeg_available() {
        // This test verifies the check_ffmpeg function runs without panicking.
        // The result depends on the environment (ffmpeg may or may not be installed).
        let _available = check_ffmpeg().await;
    }

    #[tokio::test]
    async fn transcode_rejects_invalid_input() {
        // Passing garbage bytes should cause ffmpeg to fail gracefully.
        let result = transcode_to_ogg(b"not-mp3-data").await;
        // ffmpeg may or may not be installed; if not installed, it fails at spawn.
        // If installed, it fails on invalid input. Either way, it should be an error.
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn synthesize_mp3_bad_api_key() {
        let client = TtsClient::new("invalid-key", "voice-123", "eleven_multilingual_v2");
        let result = client.synthesize_mp3("hello").await;
        // Should fail with an API error (401 or network error)
        assert!(result.is_err());
    }
}
