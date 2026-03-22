# Capability: Voice Synthesis

## ADDED Requirements

### Requirement: ElevenLabs TTS Client
The daemon SHALL include an HTTP client that calls the ElevenLabs text-to-speech API (`/v1/text-to-speech/{voice_id}`) to convert text into audio bytes. The client MUST read `ELEVENLABS_API_KEY` from environment, and `voice_id` and `model_id` from config. Audio is received as MP3 and MUST be transcoded to OGG/Opus via a spawned `ffmpeg` process.

#### Scenario: Successful synthesis
Given voice is enabled and the response is within the character threshold
When the agent loop completes a text response
Then the TTS client POSTs the text to ElevenLabs, receives MP3 bytes, pipes them through `ffmpeg -i pipe:0 -c:a libopus -f ogg pipe:1`, and returns OGG/Opus bytes.

#### Scenario: ElevenLabs API failure
Given the ElevenLabs API returns an error or times out (10s)
When synthesis is attempted
Then the error is logged at warn level and voice is skipped for this response. The text reply is unaffected.

#### Scenario: ffmpeg not available
Given `ffmpeg` is not found in PATH at daemon startup
When config has `voice_enabled = true`
Then a warning is logged ("ffmpeg not found — voice replies disabled") and voice is force-disabled for the session.

#### Scenario: ffmpeg transcoding failure
Given ffmpeg exits with a non-zero code during transcoding
When synthesis produced valid MP3 bytes
Then the error is logged and voice is skipped for this response.
